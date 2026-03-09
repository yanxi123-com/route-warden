use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use chrono::{Duration as ChronoDuration, Local, TimeZone};
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use reqwest::{Client, Proxy};
#[cfg(test)]
use std::future::Future;
#[cfg(test)]
use tokio::task::JoinSet;

use crate::cli::Cli;
use crate::config::{self, Config, TargetConfig};
use crate::controller::ControllerClient;
use crate::score::{NodeStats, ScoreWeights, score_nodes};
use crate::select::{Decision, DecisionInput, make_decision};
use crate::store::{
    GroupStateRecord, ProbeRecord, ProbeSummaryRecord, SqliteStore, SwitchEventRecord,
};

#[derive(Default)]
struct RuntimeMemory {
    candidate_wins: HashMap<String, (String, u32)>,
}

impl RuntimeMemory {
    fn update_candidate_streak(&mut self, group_key: &str, current: &str, candidate: &str) -> u32 {
        if candidate == current {
            self.candidate_wins.remove(group_key);
            return 0;
        }

        let entry = self
            .candidate_wins
            .entry(group_key.to_string())
            .or_insert_with(|| (candidate.to_string(), 0));

        if entry.0 == candidate {
            entry.1 = entry.1.saturating_add(1);
        } else {
            *entry = (candidate.to_string(), 1);
        }

        entry.1
    }
}

#[derive(Debug, Clone)]
struct GroupProbeJob {
    group_key: String,
    strategy_group: String,
    targets: Vec<TargetConfig>,
}

#[derive(Debug, Clone)]
struct GroupProbeSample {
    node_name: String,
    target: String,
    status_code: Option<i64>,
    latency_ms: f64,
    is_success: bool,
    failure_kind: Option<String>,
    created_at: i64,
}

#[derive(Debug, Clone)]
struct GroupProbeRound {
    group_key: String,
    strategy_group: String,
    current_node: String,
    stats: Vec<NodeStats>,
    probes: Vec<GroupProbeSample>,
}

pub async fn run(cli: Cli) -> Result<()> {
    let config = config::load_from_path(&cli.config)?;
    init_logging(&config);

    let controller_cfg = config.controller.as_ref().context("配置缺少 controller")?;
    let controller =
        ControllerClient::new(&controller_cfg.base_url, controller_cfg.secret.clone())?;
    let probe_client = build_probe_client(&config)?;

    let db_path = default_db_path(&cli.config);
    let store = SqliteStore::open(&db_path)
        .with_context(|| format!("打开状态库失败: {}", db_path.display()))?;

    let mut memory = RuntimeMemory::default();

    if cli.once {
        run_one_round(
            &config,
            &controller,
            &probe_client,
            &store,
            &mut memory,
            cli.dry_run,
        )
        .await?;
        print_minute_report(&config, &db_path)?;
        return Ok(());
    }

    spawn_minute_reporter(config.clone(), db_path.clone());

    loop {
        if let Err(err) = run_one_round(
            &config,
            &controller,
            &probe_client,
            &store,
            &mut memory,
            cli.dry_run,
        )
        .await
        {
            tracing::error!("round failed: {err:#}");
        }
        tokio::time::sleep(Duration::from_secs(config.interval_sec)).await;
    }
}

async fn run_one_round(
    config: &Config,
    controller: &ControllerClient,
    probe_client: &Client,
    store: &SqliteStore,
    memory: &mut RuntimeMemory,
    dry_run: bool,
) -> Result<()> {
    let started_at = now_ts();
    let round_id = store.start_round(started_at)?;

    let result: Result<()> = async {
        let probe_strategy_group = probe_strategy_group(config).to_string();
        let mut jobs = Vec::with_capacity(config.groups.len());
        for (group_key, group_cfg) in &config.groups {
            let Some(targets) = config.targets.get(group_key) else {
                tracing::warn!("group {group_key} 未配置 targets，跳过");
                continue;
            };
            jobs.push(GroupProbeJob {
                group_key: group_key.clone(),
                strategy_group: group_cfg.strategy_group.clone(),
                targets: targets.clone(),
            });
        }

        // All probe traffic goes through one probe strategy group (RW_PROBE by default),
        // so probing must be serialized to avoid concurrent group races.
        let mut rounds = Vec::with_capacity(jobs.len());
        for job in jobs {
            rounds.push(
                probe_group_candidates(job, controller, probe_client, &probe_strategy_group)
                    .await?,
            );
        }
        rounds.sort_by(|a, b| a.group_key.cmp(&b.group_key));

        for group_round in rounds {
            apply_group_round(
                round_id,
                group_round,
                config,
                controller.clone(),
                store,
                memory,
                dry_run,
            )
            .await?;
        }
        Ok(())
    }
    .await;

    let status = if result.is_ok() { "ok" } else { "failed" };
    store.finish_round(round_id, now_ts(), status)?;
    result
}

#[cfg(test)]
async fn run_jobs_in_parallel<J, T, F, Fut>(jobs: Vec<J>, runner: F) -> Result<Vec<T>>
where
    J: Send + 'static,
    T: Send + 'static,
    F: Fn(J) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    let mut set = JoinSet::new();
    for job in jobs {
        let runner = runner.clone();
        set.spawn(async move { runner(job).await });
    }

    let mut out = Vec::new();
    while let Some(joined) = set.join_next().await {
        let item = joined.context("并发任务执行失败")??;
        out.push(item);
    }
    Ok(out)
}

async fn probe_group_candidates(
    job: GroupProbeJob,
    controller: &ControllerClient,
    probe_client: &Client,
    probe_strategy_group: &str,
) -> Result<GroupProbeRound> {
    let group_key = &job.group_key;
    let strategy_group = &job.strategy_group;
    let current_node = controller
        .get_group_current(strategy_group)
        .await
        .with_context(|| format!("读取当前节点失败: group={strategy_group}"))?;

    let members = controller
        .get_group_members(strategy_group)
        .await
        .with_context(|| format!("读取组成员失败: group={strategy_group}"))?;
    let candidates = filter_candidates(&members);
    if candidates.is_empty() {
        tracing::warn!("group {group_key} 候选节点为空，跳过");
        return Ok(GroupProbeRound {
            group_key: job.group_key,
            strategy_group: job.strategy_group,
            current_node,
            stats: Vec::new(),
            probes: Vec::new(),
        });
    }

    let mut stats = Vec::with_capacity(candidates.len());
    let mut probes = Vec::with_capacity(candidates.len().saturating_mul(job.targets.len()));
    let probe_current_node = controller
        .get_group_current(probe_strategy_group)
        .await
        .with_context(|| {
            format!(
                "读取探测组当前节点失败: group={probe_strategy_group}（请确认已 sync-rw-profile 并重载配置）"
            )
        })?;
    let probe_result: Result<()> = async {
        for node in &candidates {
            controller
                .switch_group(probe_strategy_group, node)
                .await
                .with_context(|| {
                    format!("切换到探测节点失败: group={probe_strategy_group}, node={node}")
                })?;

            let mut success = 0_usize;
            let mut latencies_ms = Vec::with_capacity(job.targets.len());
            for target in &job.targets {
                let result = crate::probe::probe_target(
                    probe_client,
                    &target.name,
                    &target.method,
                    &target.url,
                    Duration::from_millis(target.timeout_ms),
                )
                .await
                .with_context(|| format!("探测失败: node={node}, target={}", target.name))?;

                let ok = match result.status_code {
                    Some(status) => match_target_success(target, status, result.is_success),
                    None => false,
                };
                let failure_kind = if ok {
                    None
                } else {
                    result.failure_kind.clone().or_else(|| {
                        result
                            .status_code
                            .map(|status| format!("http_status_{status}"))
                    })
                };

                probes.push(GroupProbeSample {
                    node_name: node.clone(),
                    target: target.name.clone(),
                    status_code: result.status_code.map(i64::from),
                    latency_ms: result.latency.as_secs_f64() * 1000.0,
                    is_success: ok,
                    failure_kind,
                    created_at: now_ts(),
                });

                if ok {
                    success = success.saturating_add(1);
                }
                latencies_ms.push(result.latency.as_millis() as f64);
            }

            let total = job.targets.len();
            let consecutive_failures = if success == 0 { 1 } else { 0 };
            stats.push(NodeStats {
                node: node.clone(),
                total,
                success,
                latencies_ms,
                consecutive_failures,
            });
        }
        Ok(())
    }
    .await;

    let restore_result = controller
        .switch_group(probe_strategy_group, &probe_current_node)
        .await
        .with_context(|| {
            format!("恢复探测组节点失败: group={probe_strategy_group}, node={probe_current_node}")
        });
    match (probe_result, restore_result) {
        (Err(probe_err), Err(restore_err)) => {
            return Err(anyhow!(
                "{probe_err:#}; 另恢复当前节点失败: {restore_err:#}"
            ));
        }
        (Err(probe_err), Ok(_)) => return Err(probe_err),
        (Ok(_), Err(restore_err)) => return Err(restore_err),
        (Ok(_), Ok(_)) => {}
    }

    Ok(GroupProbeRound {
        group_key: job.group_key,
        strategy_group: job.strategy_group,
        current_node,
        stats,
        probes,
    })
}

#[allow(clippy::too_many_arguments)]
async fn apply_group_round(
    round_id: i64,
    group_round: GroupProbeRound,
    config: &Config,
    controller: ControllerClient,
    store: &SqliteStore,
    memory: &mut RuntimeMemory,
    dry_run: bool,
) -> Result<()> {
    for probe in &group_round.probes {
        store.save_probe(&ProbeRecord {
            round_id,
            group_name: group_round.group_key.clone(),
            node_name: probe.node_name.clone(),
            target: probe.target.clone(),
            status_code: probe.status_code,
            latency_ms: probe.latency_ms,
            is_success: probe.is_success,
            failure_kind: probe.failure_kind.clone(),
            created_at: probe.created_at,
        })?;
    }
    if group_round.stats.is_empty() {
        return Ok(());
    }

    let weights = config
        .scoring
        .as_ref()
        .map(|v| ScoreWeights {
            availability_weight: v.availability_weight,
            p50_weight: v.p50_weight,
            p95_weight: v.p95_weight,
            jitter_weight: v.jitter_weight,
            ..ScoreWeights::default()
        })
        .unwrap_or_default();
    let ranked = score_nodes(&group_round.stats, weights);
    let Some(best) = ranked.first() else {
        tracing::warn!("group {} 没有可评分节点，跳过", group_round.group_key);
        return Ok(());
    };
    let candidate = best.node.clone();

    let now_ts = now_ts();
    let saved = store.load_group_state(&group_round.group_key)?;
    let last_switch_ts = saved.as_ref().and_then(|s| s.last_switch_ts);

    let current_score = ranked
        .iter()
        .find(|x| x.node == group_round.current_node)
        .map(|x| x.score)
        .unwrap_or(0.0);
    let current_failures = group_round
        .stats
        .iter()
        .find(|x| x.node == group_round.current_node)
        .map(|x| x.consecutive_failures)
        .unwrap_or(0);
    let candidate_wins = memory.update_candidate_streak(
        &group_round.group_key,
        &group_round.current_node,
        &candidate,
    );

    let decision = make_decision(&DecisionInput {
        current_node: group_round.current_node.clone(),
        candidate_node: candidate.clone(),
        current_score,
        candidate_score: best.score,
        consecutive_candidate_wins: candidate_wins,
        consecutive_current_failures: current_failures,
        min_wins: config.min_wins,
        min_improvement: config.min_improvement,
        cooldown_sec: config.cooldown_sec,
        last_switch_ts,
        now_ts,
        emergency_failures: 3,
    });

    let mut final_node = group_round.current_node.clone();
    let mut new_last_switch_ts = last_switch_ts;
    let score_gap = best.score - current_score;

    match decision {
        Decision::Switch { from, to, reason } => {
            if dry_run {
                tracing::info!(
                    "dry-run switch skipped: group={}, from={from}, to={to}, score_gap={score_gap:.4}, reason={reason}",
                    group_round.group_key
                );
            } else {
                controller
                    .switch_group(&group_round.strategy_group, &to)
                    .await?;
                store.save_switch_event(&SwitchEventRecord {
                    group_name: group_round.group_key.clone(),
                    from_node: from,
                    to_node: to.clone(),
                    score_gap,
                    reason: reason.clone(),
                    created_at: now_ts,
                })?;
                final_node = to;
                new_last_switch_ts = Some(now_ts);
                tracing::info!(
                    "switched: group={}, strategy_group={}, node={final_node}, score_gap={score_gap:.4}, reason={reason}",
                    group_round.group_key,
                    group_round.strategy_group
                );
            }
        }
        Decision::Keep { reason } => {
            tracing::info!(
                "keep: group={}, strategy_group={}, node={}, reason={reason}",
                group_round.group_key,
                group_round.strategy_group,
                group_round.current_node
            );
        }
    }

    store.save_group_state(&GroupStateRecord {
        group_name: group_round.group_key,
        current_node: final_node,
        last_switch_ts: new_last_switch_ts,
        cooldown_until_ts: new_last_switch_ts.map(|ts| ts + config.cooldown_sec as i64),
        updated_at: now_ts,
    })?;

    Ok(())
}

fn build_probe_client(config: &Config) -> Result<Client> {
    let proxy_url = config
        .probe
        .as_ref()
        .and_then(|v| v.proxy_url.as_deref())
        .unwrap_or("http://127.0.0.1:7890");

    Client::builder()
        .no_proxy()
        .proxy(Proxy::all(proxy_url).context("无效的探测代理地址")?)
        .build()
        .context("创建探测 HTTP 客户端失败")
}

fn probe_strategy_group(config: &Config) -> &str {
    config
        .probe
        .as_ref()
        .and_then(|v| v.strategy_group.as_deref())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("RW_PROBE")
}

fn filter_candidates(nodes: &[String]) -> Vec<String> {
    nodes
        .iter()
        .filter(|name| {
            !matches!(
                name.as_str(),
                "REJECT" | "REJECT-DROP" | "PASS" | "COMPATIBLE"
            )
        })
        .cloned()
        .collect()
}

fn match_target_success(target: &TargetConfig, status: u16, fallback: bool) -> bool {
    if target.success_status.is_empty() {
        return fallback;
    }
    target.success_status.contains(&status)
}

fn default_db_path(config_path: &Path) -> PathBuf {
    let mut path = config_path.to_path_buf();
    path.set_extension("sqlite3");
    path
}

fn init_logging(config: &Config) {
    let default_level = config
        .logging
        .as_ref()
        .map(|v| v.level.as_str())
        .unwrap_or("info");

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0)
}

fn spawn_minute_reporter(config: Config, db_path: PathBuf) {
    std::thread::spawn(move || {
        if let Err(err) = print_minute_report(&config, &db_path) {
            tracing::error!("status report failed: {err:#}");
        }

        loop {
            std::thread::sleep(Duration::from_secs(60));
            if let Err(err) = print_minute_report(&config, &db_path) {
                tracing::error!("status report failed: {err:#}");
            }
        }
    });
}

fn collect_minute_report_rows(
    config: &Config,
    store: &SqliteStore,
    since_ts: i64,
) -> Result<Vec<MinuteReportRow>> {
    let summaries = store.summarize_probes_since(since_ts)?;
    let mut summary_map: HashMap<(String, String), ProbeSummaryRecord> = HashMap::new();
    for item in summaries {
        summary_map.insert((item.group_name.clone(), item.node_name.clone()), item);
    }

    let mut rows = Vec::with_capacity(config.groups.len());
    for (group_key, group_cfg) in &config.groups {
        let strategy_group = group_cfg.strategy_group.clone();
        let current_node = store
            .load_group_state(group_key)?
            .map(|s| s.current_node)
            .unwrap_or_else(|| "n/a".to_string());
        let summary = if current_node == "n/a" {
            None
        } else {
            summary_map
                .get(&(group_key.clone(), current_node.clone()))
                .cloned()
        };

        rows.push(MinuteReportRow {
            group_key: group_key.clone(),
            strategy_group,
            current_node,
            summary,
        });
    }

    Ok(rows)
}

fn print_minute_report(config: &Config, db_path: &Path) -> Result<()> {
    let since_ts = now_ts() - 3600;
    let store = SqliteStore::open(db_path)
        .with_context(|| format!("打开状态库失败: {}", db_path.display()))?;
    let rows = collect_minute_report_rows(config, &store, since_ts)?;
    let table = build_status_report_table(&rows);
    tracing::info!("status-report (window=60m)\n{table}");
    Ok(())
}

fn build_status_report_table(rows: &[MinuteReportRow]) -> String {
    build_status_report_table_at(rows, now_ts())
}

fn build_status_report_table_at(rows: &[MinuteReportRow], now_ts: i64) -> String {
    let mut table = Table::new();
    table
        .set_header([
            "group",
            "strategy_group",
            "node",
            "success",
            "rate(%)",
            "avg_ms",
            "last_probe_ts",
        ])
        .set_content_arrangement(ContentArrangement::Dynamic)
        .load_preset(UTF8_FULL_CONDENSED);

    for row in rows {
        if let Some(summary) = &row.summary {
            table.add_row([
                row.group_key.as_str(),
                row.strategy_group.as_str(),
                summary.node_name.as_str(),
                &format!("{}/{}", summary.success, summary.total),
                &format!("{:.2}", summary.success_rate * 100.0),
                &format!("{:.1}", summary.avg_latency_ms),
                &format_relative_time(Some(summary.last_probe_at), now_ts),
            ]);
            continue;
        }

        table.add_row([
            row.group_key.as_str(),
            row.strategy_group.as_str(),
            row.current_node.as_str(),
            "n/a",
            "n/a",
            "n/a",
            "-",
        ]);
    }

    table.to_string()
}

fn format_relative_time(update_ts: Option<i64>, now_ts: i64) -> String {
    let Some(ts) = update_ts else {
        return "-".to_string();
    };
    if ts <= 0 || now_ts <= 0 {
        return "-".to_string();
    }

    let Some(now_dt) = Local.timestamp_opt(now_ts, 0).single() else {
        return "-".to_string();
    };
    let Some(update_dt) = Local.timestamp_opt(ts, 0).single() else {
        return "-".to_string();
    };
    let diff = now_dt - update_dt;

    if diff < ChronoDuration::seconds(1) {
        "刚刚".to_string()
    } else if diff < ChronoDuration::minutes(1) {
        format!("{}秒前", diff.num_seconds())
    } else if diff < ChronoDuration::hours(1) {
        format!("{}分钟前", diff.num_minutes())
    } else if diff < ChronoDuration::days(1) {
        format!("{}小时前", diff.num_hours())
    } else if diff < ChronoDuration::days(30) {
        let days = diff.num_days();
        let remaining_hours = diff.num_hours() % 24;
        if remaining_hours > 0 {
            format!("{}天{}小时前", days, remaining_hours)
        } else {
            format!("{}天前", days)
        }
    } else {
        update_dt.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

#[derive(Debug, Clone)]
struct MinuteReportRow {
    group_key: String,
    strategy_group: String,
    current_node: String,
    summary: Option<ProbeSummaryRecord>,
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{
        RuntimeMemory, collect_minute_report_rows, filter_candidates, match_target_success,
    };
    use crate::config::TargetConfig;
    use crate::controller::ControllerClient;
    use crate::store::{GroupStateRecord, ProbeRecord, SqliteStore};

    #[test]
    fn candidate_streak_increments_and_resets() {
        let mut memory = RuntimeMemory::default();
        assert_eq!(memory.update_candidate_streak("G", "A", "B"), 1);
        assert_eq!(memory.update_candidate_streak("G", "A", "B"), 2);
        assert_eq!(memory.update_candidate_streak("G", "A", "C"), 1);
        assert_eq!(memory.update_candidate_streak("G", "A", "A"), 0);
    }

    #[test]
    fn filter_builtin_reject_like_nodes() {
        let out = filter_candidates(&[
            "REJECT".to_string(),
            "PASS".to_string(),
            "NodeA".to_string(),
            "DIRECT".to_string(),
        ]);
        assert_eq!(out, vec!["NodeA".to_string(), "DIRECT".to_string()]);
    }

    #[test]
    fn target_success_prefers_configured_status() {
        let t = TargetConfig {
            name: "binance".to_string(),
            url: "https://api.binance.com".to_string(),
            method: "GET".to_string(),
            timeout_ms: 5_000,
            success_status: vec![200, 403],
        };
        assert!(match_target_success(&t, 403, false));
        assert!(!match_target_success(&t, 429, true));
    }

    #[test]
    fn minute_report_uses_group_state_and_probe_summary() {
        let config: crate::config::Config = serde_yaml::from_str(
            r#"
interval_sec: 180
cooldown_sec: 600
min_wins: 3
min_improvement: 0.15
groups:
  GLOBAL_BEST:
    strategy_group: "RW_GLOBAL"
targets:
  GLOBAL_BEST:
    - name: "google"
      url: "https://www.google.com/generate_204"
"#,
        )
        .expect("valid config");
        let store = SqliteStore::open_in_memory().expect("open sqlite");

        store
            .save_group_state(&GroupStateRecord {
                group_name: "GLOBAL_BEST".to_string(),
                current_node: "Node-A".to_string(),
                last_switch_ts: Some(1_000),
                cooldown_until_ts: Some(1_600),
                updated_at: 1_200,
            })
            .expect("save group state");
        let round_id = store.start_round(9_999).expect("start round");
        store
            .save_probe(&ProbeRecord {
                round_id,
                group_name: "GLOBAL_BEST".to_string(),
                node_name: "Node-A".to_string(),
                target: "google".to_string(),
                status_code: Some(204),
                latency_ms: 130.0,
                is_success: true,
                failure_kind: None,
                created_at: 10_000,
            })
            .expect("save probe");

        let rows = collect_minute_report_rows(&config, &store, 9_000).expect("collect rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].group_key, "GLOBAL_BEST");
        assert_eq!(rows[0].strategy_group, "RW_GLOBAL");
        assert_eq!(rows[0].current_node, "Node-A");
        assert_eq!(rows[0].summary.as_ref().map(|s| s.success), Some(1));
        assert_eq!(rows[0].summary.as_ref().map(|s| s.total), Some(1));
    }

    #[test]
    fn status_report_table_contains_headers_and_values() {
        let rows = vec![super::MinuteReportRow {
            group_key: "GLOBAL_BEST".to_string(),
            strategy_group: "RW_GLOBAL".to_string(),
            current_node: "Node-A".to_string(),
            summary: Some(crate::store::ProbeSummaryRecord {
                group_name: "GLOBAL_BEST".to_string(),
                node_name: "Node-A".to_string(),
                total: 10,
                success: 9,
                success_rate: 0.9,
                avg_latency_ms: 123.4,
                last_probe_at: 1_700_000_000,
            }),
        }];

        let table = super::build_status_report_table_at(&rows, 1_700_000_120);
        assert!(table.contains("group"));
        assert!(table.contains("strategy_group"));
        assert!(table.contains("GLOBAL_BEST"));
        assert!(table.contains("RW_GLOBAL"));
        assert!(table.contains("9/10"));
        assert!(table.contains("90.00"));
        assert!(table.contains("2分钟前"));
    }

    #[test]
    fn relative_time_format_matches_reference_style() {
        let now = 1_700_000_000_i64;
        assert_eq!(super::format_relative_time(None, now), "-");
        assert_eq!(super::format_relative_time(Some(0), now), "-");
        assert_eq!(super::format_relative_time(Some(now), now), "刚刚");
        assert_eq!(super::format_relative_time(Some(now - 5), now), "5秒前");
        assert_eq!(super::format_relative_time(Some(now - 120), now), "2分钟前");
        assert_eq!(
            super::format_relative_time(Some(now - 3 * 3600), now),
            "3小时前"
        );
        assert_eq!(
            super::format_relative_time(Some(now - (2 * 24 + 5) * 3600), now),
            "2天5小时前"
        );
        assert_eq!(
            super::format_relative_time(Some(now - 3 * 24 * 3600), now),
            "3天前"
        );
    }

    #[tokio::test]
    async fn run_jobs_in_parallel_is_not_serial() {
        let started = Instant::now();
        let out =
            super::run_jobs_in_parallel(vec![80_u64, 80_u64, 80_u64], |sleep_ms| async move {
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                Ok::<u64, anyhow::Error>(sleep_ms)
            })
            .await
            .expect("parallel run");

        assert_eq!(out.len(), 3);
        assert!(started.elapsed() < Duration::from_millis(170));
    }

    #[tokio::test]
    async fn probe_uses_probe_group_instead_of_strategy_group() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/proxies/RW_OPENAI"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name": "RW_OPENAI",
                "now": "NodeA",
                "all": ["NodeA", "NodeB"]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/proxies/RW_PROBE"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name": "RW_PROBE",
                "now": "ProbeStart",
                "all": ["NodeA", "NodeB"]
            })))
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/proxies/RW_OPENAI"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/proxies/RW_PROBE"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/probe-ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let controller = ControllerClient::new(&server.uri(), None).expect("controller");
        let probe_client = reqwest::Client::new();
        let job = super::GroupProbeJob {
            group_key: "OPENAI_GROUP".to_string(),
            strategy_group: "RW_OPENAI".to_string(),
            targets: vec![TargetConfig {
                name: "openai".to_string(),
                url: format!("{}/probe-ok", server.uri()),
                method: "GET".to_string(),
                timeout_ms: 5_000,
                success_status: vec![200],
            }],
        };

        let _ = super::probe_group_candidates(job, &controller, &probe_client, "RW_PROBE")
            .await
            .expect("probe should complete");

        let requests = server.received_requests().await.expect("requests");
        let openai_switches = requests
            .iter()
            .filter(|r| r.method.as_str() == "PUT" && r.url.path() == "/proxies/RW_OPENAI")
            .count();
        let probe_switches = requests
            .iter()
            .filter(|r| r.method.as_str() == "PUT" && r.url.path() == "/proxies/RW_PROBE")
            .count();

        assert_eq!(openai_switches, 0, "probe should not touch strategy group");
        assert!(probe_switches >= 1, "probe should switch probe group during tests");
    }

    #[test]
    fn probe_strategy_group_defaults_to_rw_probe() {
        let cfg: crate::config::Config =
            serde_yaml::from_str("{}").expect("default config should parse");
        assert_eq!(super::probe_strategy_group(&cfg), "RW_PROBE");
    }
}
