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
    assert!(cli.command.is_none());
}

#[test]
fn parse_sync_rw_groups_subcommand() {
    let cli = route_warden::cli::parse_from([
        "route-warden",
        "sync-rw-groups",
        "--all",
        "--verge-dir",
        "/tmp/verge",
        "--dry-run",
    ]);

    match cli.command {
        Some(route_warden::cli::Command::SyncRwGroups(args)) => {
            assert!(args.all);
            assert!(args.dry_run);
            assert_eq!(
                args.verge_dir
                    .expect("verge_dir 应被解析")
                    .to_string_lossy(),
                "/tmp/verge"
            );
        }
        _ => panic!("子命令解析失败"),
    }
}
