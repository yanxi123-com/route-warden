use std::fs;

use tempfile::tempdir;

#[test]
fn sync_rw_groups_for_current_profile() {
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
      groups: "g-current"
"#,
    )
    .unwrap();

    let written =
        route_warden::clash_verge::sync_rw_groups(verge_dir, false, false).expect("写入失败");
    assert_eq!(written.len(), 1);
    assert!(written[0].ends_with("g-current.yaml"));

    let content = fs::read_to_string(verge_dir.join("profiles").join("g-current.yaml")).unwrap();
    assert!(content.contains("name: RW_GOOGLE"));
    assert!(content.contains("name: RW_BINANCE"));
    assert!(content.contains("name: RW_OPENAI"));
    assert!(content.contains("name: RW_GITHUB"));
    assert!(content.contains("name: RW_GLOBAL"));
    assert!(content.contains("name: RW_CN_DIRECT"));
}

#[test]
fn sync_rw_groups_for_all_profiles() {
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
      groups: "g-current"
  - uid: "def"
    type: "remote"
    option:
      groups: "g-other"
"#,
    )
    .unwrap();

    let written =
        route_warden::clash_verge::sync_rw_groups(verge_dir, true, false).expect("写入失败");
    assert_eq!(written.len(), 2);

    let current = fs::read_to_string(verge_dir.join("profiles").join("g-current.yaml")).unwrap();
    let other = fs::read_to_string(verge_dir.join("profiles").join("g-other.yaml")).unwrap();
    assert!(current.contains("RW_GLOBAL"));
    assert!(other.contains("RW_GLOBAL"));
}
