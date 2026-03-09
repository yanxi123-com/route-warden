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

#[test]
fn reject_invalid_target_method() {
    use std::fs;
    use tempfile::tempdir;

    let temp = tempdir().expect("create tempdir");
    let config_path = temp.path().join("invalid-method.yaml");
    fs::write(
        &config_path,
        r#"
interval_sec: 180
cooldown_sec: 600
min_wins: 3
min_improvement: 0.15
groups:
  GLOBAL_BEST:
    strategy_group: "RW_GLOBAL"
targets:
  GLOBAL_BEST:
    - name: "google"
      url: "https://www.google.com/generate_204"
      method: "NOT A METHOD"
"#,
    )
    .expect("write config");

    let result = route_warden::config::load_from_path(&config_path);
    assert!(result.is_err());
}

#[test]
fn load_default_config_matches_example_profile() {
    use std::fs;
    use tempfile::tempdir;

    let temp = tempdir().expect("create tempdir");
    let config_path = temp.path().join("defaults.yaml");
    fs::write(&config_path, "{}\n").expect("write config");

    let cfg = route_warden::config::load_from_path(&config_path).expect("load defaults");
    assert_eq!(cfg.interval_sec, 180);
    assert_eq!(cfg.cooldown_sec, 600);
    assert_eq!(cfg.min_wins, 3);
    assert!((cfg.min_improvement - 0.15).abs() < f64::EPSILON);

    let controller = cfg.controller.expect("controller defaults");
    assert_eq!(controller.base_url, "unix:///tmp/verge/verge-mihomo.sock");
    assert_eq!(controller.secret, Some(String::new()));

    let probe = cfg.probe.expect("probe defaults");
    assert_eq!(probe.proxy_url, Some("http://127.0.0.1:7890".to_string()));
    assert_eq!(probe.strategy_group, Some("RW_PROBE".to_string()));

    assert_eq!(
        cfg.groups
            .get("GLOBAL_BEST")
            .expect("group GLOBAL_BEST")
            .strategy_group,
        "RW_GLOBAL"
    );
    assert_eq!(
        cfg.targets
            .get("OPENAI_GROUP")
            .expect("targets OPENAI_GROUP")
            .len(),
        2
    );

    let routing = cfg.routing.expect("routing defaults");
    assert_eq!(
        routing
            .domain_to_group
            .get("api.openai.com")
            .expect("routing api.openai.com"),
        "OPENAI_GROUP"
    );

    let logging = cfg.logging.expect("logging defaults");
    assert_eq!(logging.level, "info");
}
