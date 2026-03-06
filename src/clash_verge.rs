use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for GroupsTemplate {
    fn default() -> Self {
        Self {
            prepend: Vec::new(),
            append: Vec::new(),
            delete: Vec::new(),
        }
    }
}

/// 将 RW_* 组模板同步到 Clash Verge 的 groups 增强文件中。
pub fn sync_rw_groups(verge_dir: &Path, sync_all: bool, dry_run: bool) -> Result<Vec<PathBuf>> {
    let target_files = resolve_target_files(verge_dir, sync_all)?;
    if target_files.is_empty() {
        bail!("未找到可写入的 groups 增强文件");
    }

    if dry_run {
        return Ok(target_files);
    }

    for file in &target_files {
        upsert_groups_file(file)?;
    }
    Ok(target_files)
}

fn resolve_target_files(verge_dir: &Path, sync_all: bool) -> Result<Vec<PathBuf>> {
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
            if let Some(groups_name) = item.option.and_then(|opt| opt.groups) {
                names.insert(groups_name);
            }
        }
    } else {
        let current = profiles.current.context("profiles.current 为空")?;
        let current_item = profiles
            .items
            .into_iter()
            .find(|item| item.uid == current)
            .with_context(|| format!("未找到当前 profile: {current}"))?;
        let groups_name = current_item
            .option
            .and_then(|opt| opt.groups)
            .context("当前 profile 未配置 option.groups")?;
        names.insert(groups_name);
    }

    let profiles_dir = verge_dir.join("profiles");
    Ok(names
        .into_iter()
        .map(|name| profiles_dir.join(format!("{name}.yaml")))
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

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建目录失败: {}", parent.display()))?;
    }

    let yaml = serde_yaml::to_string(&tpl).context("序列化 groups 模板失败")?;
    let output = format!("# Managed by route-warden sync-rw-groups\n\n{yaml}");
    fs::write(path, output).with_context(|| format!("写入 groups 文件失败: {}", path.display()))
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
