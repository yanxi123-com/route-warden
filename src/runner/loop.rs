use std::time::Duration;

use anyhow::Result;

pub trait RunnerHooks {
    fn fetch_groups(&self) -> Result<Vec<String>>;
    fn probe_group(&self, group: &str) -> Result<()>;
    fn maybe_switch_group(&self, group: &str) -> Result<()>;
    fn persist_round(&self) -> Result<()>;
}

pub struct Runner<H> {
    hooks: H,
    interval: Duration,
}

impl<H> Runner<H>
where
    H: RunnerHooks,
{
    pub fn new(hooks: H, interval: Duration) -> Self {
        Self { hooks, interval }
    }

    pub fn tick(&self) -> Result<()> {
        let groups = self.hooks.fetch_groups()?;
        for group in groups {
            self.hooks.probe_group(&group)?;
            self.hooks.maybe_switch_group(&group)?;
        }
        self.hooks.persist_round()?;
        Ok(())
    }

    pub async fn run_forever(&self) -> Result<()> {
        loop {
            if let Err(err) = self.tick() {
                tracing::error!("runner tick failed: {err:#}");
            }
            tokio::time::sleep(self.interval).await;
        }
    }
}
