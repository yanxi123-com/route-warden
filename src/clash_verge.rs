use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Deserialize)]
struct ProfilesConfig {
    current: Option<String>,
    #[serde(default)]
    items: Vec<ProfileItem>,
}

#[derive(Debug, Deserialize)]
struct ProfileItem {
    uid: String,
    #[serde(rename = "type")]
    item_type: Option<String>,
    option: Option<ProfileOption>,
}

#[derive(Debug, Deserialize)]
struct ProfileOption {
    groups: Option<String>,
    rules: Option<String>,
}

#[derive(Debug, Clone)]
struct EnhancementFiles {
    groups_path: PathBuf,
    rules_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GroupsTemplate {
    #[serde(default)]
    prepend: Vec<serde_yaml::Value>,
    #[serde(default)]
    append: Vec<GroupEntry>,
    #[serde(default)]
    delete: Vec<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupEntry {
    name: String,
    #[serde(rename = "type")]
    group_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxies: Option<Vec<String>>,
    #[serde(rename = "include-all", skip_serializing_if = "Option::is_none")]
    include_all: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RulesTemplate {
    #[serde(default)]
    prepend: Vec<serde_yaml::Value>,
    #[serde(default)]
    append: Vec<serde_yaml::Value>,
    #[serde(default)]
    delete: Vec<serde_yaml::Value>,
}

/// 将 RW_* 组与 route-warden 路由规则同步到 Clash Verge Profile Enhancement。
pub fn sync_rw_profile(
    verge_dir: &Path,
    config: &Config,
    sync_all: bool,
    dry_run: bool,
) -> Result<Vec<PathBuf>> {
    let targets = resolve_target_files(verge_dir, sync_all)?;
    if targets.is_empty() {
        bail!("未找到可写入的 enhancement 文件");
    }

    let mut files = BTreeSet::new();
    for target in &targets {
        files.insert(target.groups_path.clone());
        files.insert(target.rules_path.clone());
    }

    if dry_run {
        return Ok(files.into_iter().collect());
    }

    for target in &targets {
        upsert_groups_file(&target.groups_path)?;
        upsert_rules_file(&target.rules_path, config)?;
    }
    Ok(files.into_iter().collect())
}

fn resolve_target_files(verge_dir: &Path, sync_all: bool) -> Result<Vec<EnhancementFiles>> {
    let profiles_yaml = verge_dir.join("profiles.yaml");
    let content = fs::read_to_string(&profiles_yaml)
        .with_context(|| format!("读取 profiles 配置失败: {}", profiles_yaml.display()))?;
    let profiles: ProfilesConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("解析 profiles 配置失败: {}", profiles_yaml.display()))?;

    let mut names = BTreeSet::new();
    if sync_all {
        for item in profiles.items {
            if item.item_type.as_deref() != Some("remote") {
                continue;
            }
            let Some(option) = item.option else {
                continue;
            };
            let Some(groups_name) = option.groups else {
                continue;
            };
            let rules_name = option.rules.unwrap_or_else(|| groups_name.clone());
            names.insert((groups_name, rules_name));
        }
    } else {
        let current = profiles.current.context("profiles.current 为空")?;
        let current_item = profiles
            .items
            .into_iter()
            .find(|item| item.uid == current)
            .with_context(|| format!("未找到当前 profile: {current}"))?;
        let option = current_item.option.context("当前 profile 未配置 option")?;
        let groups_name = option.groups.context("当前 profile 未配置 option.groups")?;
        let rules_name = option.rules.unwrap_or_else(|| groups_name.clone());
        names.insert((groups_name, rules_name));
    }

    let profiles_dir = verge_dir.join("profiles");
    Ok(names
        .into_iter()
        .map(|(groups_name, rules_name)| EnhancementFiles {
            groups_path: profiles_dir.join(format!("{groups_name}.yaml")),
            rules_path: profiles_dir.join(format!("{rules_name}.yaml")),
        })
        .collect())
}

fn upsert_groups_file(path: &Path) -> Result<()> {
    let mut tpl = if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("读取 groups 文件失败: {}", path.display()))?;
        serde_yaml::from_str::<GroupsTemplate>(&content)
            .with_context(|| format!("解析 groups 文件失败: {}", path.display()))?
    } else {
        GroupsTemplate::default()
    };

    for desired in desired_groups() {
        if let Some(existing) = tpl.append.iter_mut().find(|item| item.name == desired.name) {
            *existing = desired;
        } else {
            tpl.append.push(desired);
        }
    }

    write_template_file(path, &tpl, "sync-rw-profile", "groups")
}

fn upsert_rules_file(path: &Path, config: &Config) -> Result<()> {
    let mut tpl = if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("读取 rules 文件失败: {}", path.display()))?;
        serde_yaml::from_str::<RulesTemplate>(&content)
            .with_context(|| format!("解析 rules 文件失败: {}", path.display()))?
    } else {
        RulesTemplate::default()
    };

    let desired = desired_rules(config)?
        .into_iter()
        .map(serde_yaml::Value::String)
        .collect();
    tpl.append = desired;

    write_template_file(path, &tpl, "sync-rw-profile", "rules")
}

fn write_template_file(
    path: &Path,
    template: &impl Serialize,
    command_name: &str,
    kind: &str,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建目录失败: {}", parent.display()))?;
    }

    let yaml =
        serde_yaml::to_string(template).with_context(|| format!("序列化 {kind} 模板失败"))?;
    let output = format!("# Managed by route-warden {command_name}\n\n{yaml}");
    fs::write(path, output).with_context(|| format!("写入 {kind} 文件失败: {}", path.display()))
}

fn desired_groups() -> Vec<GroupEntry> {
    vec![
        GroupEntry {
            name: "RW_CN_DIRECT".to_string(),
            group_type: "select".to_string(),
            proxies: Some(vec!["DIRECT".to_string()]),
            include_all: None,
        },
        GroupEntry {
            name: "RW_GOOGLE".to_string(),
            group_type: "select".to_string(),
            proxies: None,
            include_all: Some(true),
        },
        GroupEntry {
            name: "RW_BINANCE".to_string(),
            group_type: "select".to_string(),
            proxies: None,
            include_all: Some(true),
        },
        GroupEntry {
            name: "RW_OPENAI".to_string(),
            group_type: "select".to_string(),
            proxies: None,
            include_all: Some(true),
        },
        GroupEntry {
            name: "RW_GITHUB".to_string(),
            group_type: "select".to_string(),
            proxies: None,
            include_all: Some(true),
        },
        GroupEntry {
            name: "RW_GLOBAL".to_string(),
            group_type: "select".to_string(),
            proxies: None,
            include_all: Some(true),
        },
    ]
}

fn desired_rules(config: &Config) -> Result<Vec<String>> {
    let mut rules = Vec::new();
    if let Some(routing) = &config.routing {
        for (domain, group_ref) in &routing.domain_to_group {
            let normalized = domain.trim();
            if normalized.is_empty() {
                continue;
            }
            let strategy_group = resolve_strategy_group(config, group_ref)?;
            rules.push(format!("DOMAIN,{normalized},{strategy_group}"));
        }
    }

    rules.push("GEOSITE,CN,RW_CN_DIRECT".to_string());
    rules.push("GEOIP,CN,RW_CN_DIRECT,no-resolve".to_string());
    let global = config
        .groups
        .get("GLOBAL_BEST")
        .map(|group| group.strategy_group.clone())
        .unwrap_or_else(|| "RW_GLOBAL".to_string());
    rules.push(format!("MATCH,{global}"));
    Ok(rules)
}

fn resolve_strategy_group(config: &Config, group_ref: &str) -> Result<String> {
    let normalized = group_ref.trim();
    if normalized.is_empty() {
        bail!("routing.domain_to_group 的 group 不能为空");
    }
    if let Some(group) = config.groups.get(normalized) {
        return Ok(group.strategy_group.clone());
    }
    Ok(normalized.to_string())
}
