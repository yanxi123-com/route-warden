use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Clone, Parser)]
#[command(name = "route-warden")]
#[command(about = "Mihomo/Clash route and proxy selector daemon")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(long, help = "只执行一轮后退出")]
    pub once: bool,

    #[arg(long, help = "仅探测和评分，不执行切换")]
    pub dry_run: bool,

    #[arg(long, default_value_os_t = default_config_path(), help = "配置文件路径")]
    pub config: PathBuf,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    #[command(
        name = "sync-rw-groups",
        about = "写入 Clash Verge Profile Enhancement 的 RW_* 组模板"
    )]
    SyncRwGroups(SyncRwGroupsArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SyncRwGroupsArgs {
    #[arg(long, help = "Clash Verge 数据目录")]
    pub verge_dir: Option<PathBuf>,

    #[arg(long, help = "同步所有远程订阅对应的 groups 增强文件")]
    pub all: bool,

    #[arg(long, help = "仅显示将要写入的文件，不真正写入")]
    pub dry_run: bool,
}

pub fn parse() -> Cli {
    Cli::parse()
}

pub fn parse_from<I, T>(itr: I) -> Cli
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    Cli::parse_from(itr)
}

fn default_config_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let mut path = PathBuf::from(home);
    path.push(".route-warden");
    path.push("config.yaml");
    path
}
