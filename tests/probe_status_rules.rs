#[test]
fn classify_status_codes() {
    use route_warden::probe::classify;

    assert!(classify("BINANCE", 403).is_success);
    assert!(!classify("BINANCE", 429).is_success);

    assert!(classify("OPENAI", 401).is_success);
    assert!(!classify("OPENAI", 429).is_success);

    assert!(classify("GITHUB", 301).is_success);
    assert!(!classify("GITHUB", 503).is_success);
}

#[tokio::test]
async fn probe_target_uses_configured_http_method() {
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/healthz"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let result = route_warden::probe::probe_target(
        &client,
        "GOOGLE",
        "HEAD",
        &format!("{}/healthz", server.uri()),
        Duration::from_millis(500),
    )
    .await
    .expect("probe should complete");
    assert_eq!(result.status_code, Some(204));
    assert!(result.is_success);
}
