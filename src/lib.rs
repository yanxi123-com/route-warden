pub mod cli;
pub mod config;
pub mod controller;
pub mod probe;
pub mod runner;
pub mod score;
pub mod select;
pub mod store;

pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
