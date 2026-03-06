use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Client, Proxy};

use crate::cli::Cli;
use crate::config::{self, Config, TargetConfig};
use crate::controller::ControllerClient;
use crate::score::{NodeStats, ScoreWeights, score_nodes};
use crate::select::{Decision, DecisionInput, make_decision};
use crate::store::{GroupStateRecord, ProbeRecord, SqliteStore, SwitchEventRecord};

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
        return Ok(());
    }

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
        for (group_key, group_cfg) in &config.groups {
            let Some(targets) = config.targets.get(group_key) else {
                tracing::warn!("group {group_key} 未配置 targets，跳过");
                continue;
            };

            let strategy_group = &group_cfg.strategy_group;
            process_group(
                round_id,
                group_key,
                strategy_group,
                targets,
                config,
                controller,
                probe_client,
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

#[allow(clippy::too_many_arguments)]
async fn process_group(
    round_id: i64,
    group_key: &str,
    strategy_group: &str,
    targets: &[TargetConfig],
    config: &Config,
    controller: &ControllerClient,
    probe_client: &Client,
    store: &SqliteStore,
    memory: &mut RuntimeMemory,
    dry_run: bool,
) -> Result<()> {
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
        return Ok(());
    }

    let mut stats = Vec::with_capacity(candidates.len());
    for node in &candidates {
        controller
            .switch_group(strategy_group, node)
            .await
            .with_context(|| format!("切换到探测节点失败: group={strategy_group}, node={node}"))?;

        let mut success = 0_usize;
        let mut latencies_ms = Vec::with_capacity(targets.len());
        for target in targets {
            let result = crate::probe::probe_target(
                probe_client,
                &target.name,
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

            store.save_probe(&ProbeRecord {
                round_id,
                group_name: group_key.to_string(),
                node_name: node.clone(),
                target: target.name.clone(),
                status_code: result.status_code.map(i64::from),
                latency_ms: result.latency.as_secs_f64() * 1000.0,
                is_success: ok,
                failure_kind,
                created_at: now_ts(),
            })?;

            if ok {
                success = success.saturating_add(1);
            }
            latencies_ms.push(result.latency.as_millis() as f64);
        }

        let total = targets.len();
        let consecutive_failures = if success == 0 { 1 } else { 0 };
        stats.push(NodeStats {
            node: node.clone(),
            total,
            success,
            latencies_ms,
            consecutive_failures,
        });
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
    let ranked = score_nodes(&stats, weights);
    let Some(best) = ranked.first() else {
        tracing::warn!("group {group_key} 没有可评分节点，跳过");
        controller
            .switch_group(strategy_group, &current_node)
            .await?;
        return Ok(());
    };
    let candidate = best.node.clone();

    let now_ts = now_ts();
    let saved = store.load_group_state(group_key)?;
    let last_switch_ts = saved.as_ref().and_then(|s| s.last_switch_ts);

    let current_score = ranked
        .iter()
        .find(|x| x.node == current_node)
        .map(|x| x.score)
        .unwrap_or(0.0);
    let current_failures = stats
        .iter()
        .find(|x| x.node == current_node)
        .map(|x| x.consecutive_failures)
        .unwrap_or(0);
    let candidate_wins = memory.update_candidate_streak(group_key, &current_node, &candidate);

    let decision = make_decision(&DecisionInput {
        current_node: current_node.clone(),
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

    let mut final_node = current_node.clone();
    let mut new_last_switch_ts = last_switch_ts;

    match decision {
        Decision::Switch { from, to, reason } => {
            if dry_run {
                controller.switch_group(strategy_group, &from).await?;
                tracing::info!(
                    "dry-run switch skipped: group={group_key}, from={from}, to={to}, reason={reason}"
                );
            } else {
                controller.switch_group(strategy_group, &to).await?;
                store.save_switch_event(&SwitchEventRecord {
                    group_name: group_key.to_string(),
                    from_node: from,
                    to_node: to.clone(),
                    reason: reason.clone(),
                    created_at: now_ts,
                })?;
                final_node = to;
                new_last_switch_ts = Some(now_ts);
                tracing::info!(
                    "switched: group={group_key}, strategy_group={strategy_group}, node={final_node}, reason={reason}"
                );
            }
        }
        Decision::Keep { reason } => {
            controller
                .switch_group(strategy_group, &current_node)
                .await?;
            tracing::info!(
                "keep: group={group_key}, strategy_group={strategy_group}, node={current_node}, reason={reason}"
            );
        }
    }

    store.save_group_state(&GroupStateRecord {
        group_name: group_key.to_string(),
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

#[cfg(test)]
mod tests {
    use super::{RuntimeMemory, filter_candidates, match_target_success};
    use crate::config::TargetConfig;

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
}
