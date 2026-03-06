mod http_probe;

use std::time::{Duration, Instant};

use anyhow::Result;
pub use http_probe::probe_url;
use reqwest::Client;

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub target: String,
    pub url: String,
    pub latency: Duration,
    pub status_code: Option<u16>,
    pub is_success: bool,
    pub failure_kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Classification {
    pub is_success: bool,
}

pub fn classify(target: &str, status_code: u16) -> Classification {
    let normalized = target.to_ascii_uppercase();
    let is_success = match normalized.as_str() {
        "BINANCE" => matches!(status_code, 200 | 403),
        "OPENAI" => matches!(status_code, 200 | 401 | 403),
        _ => (200..400).contains(&status_code),
    };
    Classification { is_success }
}

pub async fn probe_target(
    client: &Client,
    target: &str,
    url: &str,
    timeout: Duration,
) -> Result<ProbeResult> {
    let start = Instant::now();
    let response = client.get(url).timeout(timeout).send().await;
    let elapsed = start.elapsed();

    let result = match response {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let classification = classify(target, status);
            ProbeResult {
                target: target.to_string(),
                url: url.to_string(),
                latency: elapsed,
                status_code: Some(status),
                is_success: classification.is_success,
                failure_kind: if classification.is_success {
                    None
                } else {
                    Some(format!("http_status_{status}"))
                },
            }
        }
        Err(err) => ProbeResult {
            target: target.to_string(),
            url: url.to_string(),
            latency: elapsed,
            status_code: None,
            is_success: false,
            failure_kind: Some(classify_transport_error(&err)),
        },
    };
    Ok(result)
}

fn classify_transport_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        return "timeout".to_string();
    }
    if err.is_connect() {
        return "connect_error".to_string();
    }
    if err.is_request() {
        return "request_error".to_string();
    }
    "transport_error".to_string()
}
