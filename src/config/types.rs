use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_controller")]
    pub controller: Option<ControllerConfig>,
    #[serde(default = "default_probe")]
    pub probe: Option<ProbeConfig>,
    #[serde(default = "default_interval_sec")]
    pub interval_sec: u64,
    #[serde(default = "default_cooldown_sec")]
    pub cooldown_sec: u64,
    #[serde(default = "default_min_wins")]
    pub min_wins: u32,
    #[serde(default = "default_min_improvement")]
    pub min_improvement: f64,
    #[serde(default = "default_groups")]
    pub groups: BTreeMap<String, GroupConfig>,
    #[serde(default = "default_targets")]
    pub targets: BTreeMap<String, Vec<TargetConfig>>,
    #[serde(default = "default_scoring")]
    pub scoring: Option<ScoringConfig>,
    #[serde(default = "default_routing")]
    pub routing: Option<RoutingConfig>,
    #[serde(default = "default_logging")]
    pub logging: Option<LoggingConfig>,
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.interval_sec == 0 {
            bail!("interval_sec 必须大于 0");
        }
        if self.cooldown_sec == 0 {
            bail!("cooldown_sec 必须大于 0");
        }
        if self.min_wins == 0 {
            bail!("min_wins 必须大于 0");
        }
        if !(0.0..=1.0).contains(&self.min_improvement) {
            bail!("min_improvement 必须在 [0, 1] 区间内");
        }
        if self.groups.is_empty() {
            bail!("groups 不能为空");
        }
        if !self.groups.contains_key("GLOBAL_BEST") {
            bail!("groups 必须包含 GLOBAL_BEST");
        }
        if let Some(probe) = &self.probe
            && let Some(proxy_url) = &probe.proxy_url
            && proxy_url.trim().is_empty()
        {
            bail!("probe.proxy_url 不能为空字符串");
        }

        for (name, items) in &self.targets {
            if items.is_empty() {
                bail!("targets.{name} 不能为空");
            }
            for (index, target) in items.iter().enumerate() {
                if target.url.trim().is_empty() {
                    bail!("targets.{name}[{index}].url 不能为空");
                }
                if target.timeout_ms == 0 {
                    bail!("targets.{name}[{index}].timeout_ms 必须大于 0");
                }
                if reqwest::Method::from_bytes(target.method.as_bytes()).is_err() {
                    bail!("targets.{name}[{index}].method 非法: {}", target.method);
                }
            }
        }
        if let Some(routing) = &self.routing {
            for (domain, group_ref) in &routing.domain_to_group {
                if domain.trim().is_empty() {
                    bail!("routing.domain_to_group 的 domain 不能为空");
                }
                let normalized = group_ref.trim();
                if normalized.is_empty() {
                    bail!("routing.domain_to_group.{domain} 的 group 不能为空");
                }
                if !self.groups.contains_key(normalized) && !normalized.starts_with("RW_") {
                    bail!("routing.domain_to_group.{domain} 引用了不存在的组: {normalized}");
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeConfig {
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControllerConfig {
    pub base_url: String,
    pub secret: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupConfig {
    pub strategy_group: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub success_status: Vec<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScoringConfig {
    pub availability_weight: f64,
    pub p50_weight: f64,
    pub p95_weight: f64,
    pub jitter_weight: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    pub domain_to_group: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

fn default_method() -> String {
    "GET".to_string()
}

fn default_timeout_ms() -> u64 {
    5_000
}

fn default_controller() -> Option<ControllerConfig> {
    Some(ControllerConfig {
        base_url: "unix:///tmp/verge/verge-mihomo.sock".to_string(),
        secret: Some(String::new()),
    })
}

fn default_probe() -> Option<ProbeConfig> {
    Some(ProbeConfig {
        proxy_url: Some("http://127.0.0.1:7890".to_string()),
    })
}

fn default_interval_sec() -> u64 {
    180
}

fn default_cooldown_sec() -> u64 {
    600
}

fn default_min_wins() -> u32 {
    3
}

fn default_min_improvement() -> f64 {
    0.15
}

fn default_groups() -> BTreeMap<String, GroupConfig> {
    BTreeMap::from([
        (
            "GOOGLE_GROUP".to_string(),
            GroupConfig {
                strategy_group: "RW_GOOGLE".to_string(),
            },
        ),
        (
            "BINANCE_GROUP".to_string(),
            GroupConfig {
                strategy_group: "RW_BINANCE".to_string(),
            },
        ),
        (
            "OPENAI_GROUP".to_string(),
            GroupConfig {
                strategy_group: "RW_OPENAI".to_string(),
            },
        ),
        (
            "GITHUB_GROUP".to_string(),
            GroupConfig {
                strategy_group: "RW_GITHUB".to_string(),
            },
        ),
        (
            "GLOBAL_BEST".to_string(),
            GroupConfig {
                strategy_group: "RW_GLOBAL".to_string(),
            },
        ),
    ])
}

fn default_targets() -> BTreeMap<String, Vec<TargetConfig>> {
    BTreeMap::from([
        (
            "GOOGLE_GROUP".to_string(),
            vec![TargetConfig {
                name: "google".to_string(),
                url: "https://www.google.com/generate_204".to_string(),
                method: default_method(),
                timeout_ms: default_timeout_ms(),
                success_status: vec![204],
            }],
        ),
        (
            "BINANCE_GROUP".to_string(),
            vec![TargetConfig {
                name: "binance".to_string(),
                url: "https://api.binance.com/api/v3/time".to_string(),
                method: default_method(),
                timeout_ms: default_timeout_ms(),
                success_status: vec![200, 403],
            }],
        ),
        (
            "OPENAI_GROUP".to_string(),
            vec![
                TargetConfig {
                    name: "chatgpt".to_string(),
                    url: "https://chatgpt.com/favicon.ico".to_string(),
                    method: default_method(),
                    timeout_ms: default_timeout_ms(),
                    success_status: vec![200, 301, 302],
                },
                TargetConfig {
                    name: "openai".to_string(),
                    url: "https://api.openai.com/v1/models".to_string(),
                    method: default_method(),
                    timeout_ms: default_timeout_ms(),
                    success_status: vec![200, 401, 403],
                },
            ],
        ),
        (
            "GITHUB_GROUP".to_string(),
            vec![
                TargetConfig {
                    name: "github".to_string(),
                    url: "https://github.com".to_string(),
                    method: default_method(),
                    timeout_ms: default_timeout_ms(),
                    success_status: vec![200, 301, 302],
                },
                TargetConfig {
                    name: "github-api".to_string(),
                    url: "https://api.github.com".to_string(),
                    method: default_method(),
                    timeout_ms: default_timeout_ms(),
                    success_status: vec![200],
                },
            ],
        ),
        (
            "GLOBAL_BEST".to_string(),
            vec![
                TargetConfig {
                    name: "google".to_string(),
                    url: "https://www.google.com/generate_204".to_string(),
                    method: default_method(),
                    timeout_ms: default_timeout_ms(),
                    success_status: vec![204],
                },
                TargetConfig {
                    name: "binance".to_string(),
                    url: "https://api.binance.com/api/v3/time".to_string(),
                    method: default_method(),
                    timeout_ms: default_timeout_ms(),
                    success_status: vec![200, 403],
                },
            ],
        ),
    ])
}

fn default_scoring() -> Option<ScoringConfig> {
    Some(ScoringConfig {
        availability_weight: 0.7,
        p50_weight: 0.15,
        p95_weight: 0.1,
        jitter_weight: 0.05,
    })
}

fn default_routing() -> Option<RoutingConfig> {
    Some(RoutingConfig {
        domain_to_group: BTreeMap::from([
            ("google.com".to_string(), "GOOGLE_GROUP".to_string()),
            ("api.binance.com".to_string(), "BINANCE_GROUP".to_string()),
            ("chatgpt.com".to_string(), "OPENAI_GROUP".to_string()),
            ("api.openai.com".to_string(), "OPENAI_GROUP".to_string()),
            ("github.com".to_string(), "GITHUB_GROUP".to_string()),
            ("api.github.com".to_string(), "GITHUB_GROUP".to_string()),
        ]),
    })
}

fn default_logging() -> Option<LoggingConfig> {
    Some(LoggingConfig {
        level: "info".to_string(),
    })
}
