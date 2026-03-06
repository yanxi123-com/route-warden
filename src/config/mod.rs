mod types;

use std::path::Path;

use anyhow::{Context, Result};

pub use types::{
    Config, ControllerConfig, GroupConfig, LoggingConfig, RoutingConfig, ScoringConfig,
    TargetConfig,
};

pub fn load_from_path(path: impl AsRef<Path>) -> Result<Config> {
    let path_ref = path.as_ref();
    let text = std::fs::read_to_string(path_ref)
        .with_context(|| format!("读取配置文件失败: {}", path_ref.display()))?;
    let config: Config = serde_yaml::from_str(&text)
        .with_context(|| format!("解析配置文件失败: {}", path_ref.display()))?;
    config.validate()?;
    Ok(config)
}
