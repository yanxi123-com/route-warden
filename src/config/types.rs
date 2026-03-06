use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub controller: Option<ControllerConfig>,
    pub probe: Option<ProbeConfig>,
    pub interval_sec: u64,
    pub cooldown_sec: u64,
    pub min_wins: u32,
    pub min_improvement: f64,
    pub groups: BTreeMap<String, GroupConfig>,
    pub targets: BTreeMap<String, Vec<TargetConfig>>,
    pub scoring: Option<ScoringConfig>,
    pub routing: Option<RoutingConfig>,
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
