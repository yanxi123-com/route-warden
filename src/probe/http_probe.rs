use anyhow::Result;

use super::{ProbeResult, probe_target};

pub async fn probe_url(
    client: &reqwest::Client,
    target: &str,
    method: &str,
    url: &str,
    timeout: std::time::Duration,
) -> Result<ProbeResult> {
    probe_target(client, target, method, url, timeout).await
}
