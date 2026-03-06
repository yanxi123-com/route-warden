mod http_probe;

use std::error::Error;
use std::io::ErrorKind;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
pub use http_probe::probe_url;
use reqwest::{Client, Method};

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
    method: &str,
    url: &str,
    timeout: Duration,
) -> Result<ProbeResult> {
    let method = Method::from_bytes(method.as_bytes())
        .with_context(|| format!("invalid HTTP method: {method}"))?;
    let start = Instant::now();
    let response = client.request(method, url).timeout(timeout).send().await;
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
    let io_kind = find_io_error_kind(err);
    let message = collect_error_messages(err);
    classify_transport_error_parts(err.is_timeout(), &message, io_kind)
}

fn classify_transport_error_parts(
    is_timeout: bool,
    message: &str,
    io_kind: Option<ErrorKind>,
) -> String {
    if is_timeout {
        return "timeout".to_string();
    }

    let lower = message.to_ascii_lowercase();
    if matches!(io_kind, Some(ErrorKind::ConnectionReset))
        || lower.contains("connection reset")
        || lower.contains("tcp reset")
    {
        return "tcp_reset".to_string();
    }

    if lower.contains("tls")
        || lower.contains("ssl")
        || lower.contains("certificate")
        || lower.contains("handshake")
        || lower.contains("x509")
    {
        return "tls_fail".to_string();
    }

    "transport_error".to_string()
}

fn find_io_error_kind(err: &reqwest::Error) -> Option<ErrorKind> {
    let mut source = err.source();
    while let Some(item) = source {
        if let Some(io_err) = item.downcast_ref::<std::io::Error>() {
            return Some(io_err.kind());
        }
        source = item.source();
    }
    None
}

fn collect_error_messages(err: &reqwest::Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(item) = source {
        text.push(' ');
        text.push_str(&item.to_string());
        source = item.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::classify_transport_error_parts;
    use std::io::ErrorKind;

    #[test]
    fn classify_timeout_first() {
        assert_eq!(
            classify_transport_error_parts(true, "tls handshake timeout", None),
            "timeout"
        );
    }

    #[test]
    fn classify_tcp_reset_by_io_kind_or_message() {
        assert_eq!(
            classify_transport_error_parts(
                false,
                "network error",
                Some(ErrorKind::ConnectionReset)
            ),
            "tcp_reset"
        );
        assert_eq!(
            classify_transport_error_parts(false, "connection reset by peer", None),
            "tcp_reset"
        );
    }

    #[test]
    fn classify_tls_failure_by_message() {
        assert_eq!(
            classify_transport_error_parts(false, "tls handshake eof", None),
            "tls_fail"
        );
        assert_eq!(
            classify_transport_error_parts(false, "invalid certificate chain", None),
            "tls_fail"
        );
    }

    #[test]
    fn classify_other_transport_failure() {
        assert_eq!(
            classify_transport_error_parts(false, "dns lookup failed", None),
            "transport_error"
        );
    }
}
