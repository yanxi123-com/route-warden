use std::fs;

use tempfile::tempdir;

fn test_config() -> route_warden::config::Config {
    serde_yaml::from_str(
        r#"
interval_sec: 180
cooldown_sec: 600
min_wins: 3
min_improvement: 0.15
groups:
  GOOGLE_GROUP:
    strategy_group: "RW_GOOGLE"
  BINANCE_GROUP:
    strategy_group: "RW_BINANCE"
  OPENAI_GROUP:
    strategy_group: "RW_OPENAI"
  GITHUB_GROUP:
    strategy_group: "RW_GITHUB"
  GLOBAL_BEST:
    strategy_group: "RW_GLOBAL"
targets:
  GLOBAL_BEST:
    - name: "google"
      url: "https://www.google.com/generate_204"
routing:
  domain_to_group:
    "google.com": "GOOGLE_GROUP"
    "api.binance.com": "BINANCE_GROUP"
    "chatgpt.com": "OPENAI_GROUP"
    "api.openai.com": "OPENAI_GROUP"
    "github.com": "GITHUB_GROUP"
    "api.github.com": "GITHUB_GROUP"
"#,
    )
    .expect("config should parse")
}

#[test]
fn sync_rw_profile_for_current_profile() {
    let temp = tempdir().unwrap();
    let verge_dir = temp.path();
    fs::create_dir_all(verge_dir.join("profiles")).unwrap();
    fs::write(
        verge_dir.join("profiles.yaml"),
        r#"
current: "abc"
items:
  - uid: "abc"
    type: "remote"
    option:
      groups: "g-current-groups"
      rules: "g-current-rules"
"#,
    )
    .unwrap();

    let config = test_config();
    let written = route_warden::clash_verge::sync_rw_profile(verge_dir, &config, false, false)
        .expect("写入失败");
    assert_eq!(written.len(), 2);
    assert!(written.iter().any(|f| f.ends_with("g-current-groups.yaml")));
    assert!(written.iter().any(|f| f.ends_with("g-current-rules.yaml")));

    let groups = fs::read_to_string(verge_dir.join("profiles").join("g-current-groups.yaml"))
        .expect("read groups");
    assert!(groups.contains("name: RW_GOOGLE"));
    assert!(groups.contains("name: RW_BINANCE"));
    assert!(groups.contains("name: RW_OPENAI"));
    assert!(groups.contains("name: RW_GITHUB"));
    assert!(groups.contains("name: RW_GLOBAL"));
    assert!(groups.contains("name: RW_CN_DIRECT"));
    assert!(groups.contains("name: RW_PROBE"));

    let rules = fs::read_to_string(verge_dir.join("profiles").join("g-current-rules.yaml"))
        .expect("read rules");
    assert!(rules.contains("DOMAIN,google.com,RW_GOOGLE"));
    assert!(rules.contains("DOMAIN,api.binance.com,RW_BINANCE"));
    assert!(rules.contains("GEOIP,CN,RW_CN_DIRECT,no-resolve"));
    assert!(rules.contains("MATCH,RW_GLOBAL"));

    let parsed: serde_yaml::Value = serde_yaml::from_str(&rules).expect("rules yaml parse");
    let prepend = parsed
        .get("prepend")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("prepend should exist");
    assert!(
        prepend
            .iter()
            .any(|v| v.as_str() == Some("DOMAIN,api.openai.com,RW_OPENAI")),
        "DOMAIN rule should be written into prepend to take priority over early MATCH rules"
    );
    assert!(
        prepend
            .iter()
            .any(|v| v.as_str() == Some("PROCESS-NAME,route-warden,RW_PROBE")),
        "route-warden process traffic should be routed to probe group"
    );
    let append = parsed
        .get("append")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("append should exist");
    assert!(
        append.is_empty(),
        "append should remain empty for managed route rules"
    );
}

#[test]
fn sync_rw_profile_for_all_profiles() {
    let temp = tempdir().unwrap();
    let verge_dir = temp.path();
    fs::create_dir_all(verge_dir.join("profiles")).unwrap();
    fs::write(
        verge_dir.join("profiles.yaml"),
        r#"
current: "abc"
items:
  - uid: "abc"
    type: "remote"
    option:
      groups: "g-current-groups"
      rules: "g-current-rules"
  - uid: "def"
    type: "remote"
    option:
      groups: "g-other-groups"
      rules: "g-other-rules"
"#,
    )
    .unwrap();

    let config = test_config();
    let written = route_warden::clash_verge::sync_rw_profile(verge_dir, &config, true, false)
        .expect("写入失败");
    assert_eq!(written.len(), 4);

    let current_groups =
        fs::read_to_string(verge_dir.join("profiles").join("g-current-groups.yaml")).unwrap();
    let other_groups =
        fs::read_to_string(verge_dir.join("profiles").join("g-other-groups.yaml")).unwrap();
    let current_rules =
        fs::read_to_string(verge_dir.join("profiles").join("g-current-rules.yaml")).unwrap();
    let other_rules =
        fs::read_to_string(verge_dir.join("profiles").join("g-other-rules.yaml")).unwrap();
    assert!(current_groups.contains("RW_GLOBAL"));
    assert!(other_groups.contains("RW_GLOBAL"));
    assert!(current_groups.contains("RW_PROBE"));
    assert!(other_groups.contains("RW_PROBE"));
    assert!(current_rules.contains("MATCH,RW_GLOBAL"));
    assert!(other_rules.contains("MATCH,RW_GLOBAL"));
    assert!(current_rules.contains("PROCESS-NAME,route-warden,RW_PROBE"));
    assert!(other_rules.contains("PROCESS-NAME,route-warden,RW_PROBE"));
}
