#[test]
fn parse_dry_run_and_once_flags() {
    let cli = route_warden::cli::parse_from([
        "route-warden",
        "--once",
        "--dry-run",
        "--config",
        "fixtures/config.valid.yaml",
    ]);

    assert!(cli.once);
    assert!(cli.dry_run);
    assert_eq!(
        cli.config.to_string_lossy(),
        "fixtures/config.valid.yaml".to_string()
    );
}
