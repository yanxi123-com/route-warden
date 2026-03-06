use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "route-warden")]
#[command(about = "Mihomo/Clash route and proxy selector daemon")]
pub struct Cli {
    #[arg(long, help = "只执行一轮后退出")]
    pub once: bool,

    #[arg(long, help = "仅探测和评分，不执行切换")]
    pub dry_run: bool,

    #[arg(long, default_value = "config.yaml", help = "配置文件路径")]
    pub config: PathBuf,
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
