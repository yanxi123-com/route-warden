#[test]
fn app_bootstrap_compiles_and_starts() {
    let version = route_warden::app_version();
    assert!(!version.is_empty());
}
