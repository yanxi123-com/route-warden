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
