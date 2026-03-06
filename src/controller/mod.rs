mod client;

pub use client::ControllerClient;

pub async fn switch_group(base_url: &str, group: &str, node: &str) -> anyhow::Result<()> {
    let client = ControllerClient::new(base_url, None)?;
    client.switch_group(group, node).await
}
