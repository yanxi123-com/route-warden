#[test]
fn load_valid_config() {
    let cfg = route_warden::config::load_from_path("fixtures/config.valid.yaml").unwrap();
    assert_eq!(cfg.interval_sec, 180);
    assert!(cfg.groups.contains_key("GLOBAL_BEST"));
}

#[test]
fn load_invalid_config() {
    let result = route_warden::config::load_from_path("fixtures/config.invalid.yaml");
    assert!(result.is_err());
}
