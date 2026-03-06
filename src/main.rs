fn main() {
    let cli = route_warden::cli::parse();

    if let Some(command) = cli.command.clone() {
        match command {
            route_warden::cli::Command::SyncRwGroups(args) => {
                let verge_dir = args.verge_dir.unwrap_or_else(default_clash_verge_dir);
                match route_warden::clash_verge::sync_rw_groups(&verge_dir, args.all, args.dry_run)
                {
                    Ok(files) => {
                        println!(
                            "sync-rw-groups done (dry_run={}, files={})",
                            args.dry_run,
                            files.len()
                        );
                        for file in files {
                            println!("- {}", file.display());
                        }
                    }
                    Err(err) => {
                        eprintln!("sync-rw-groups failed: {err:#}");
                        std::process::exit(1);
                    }
                }
                return;
            }
        }
    }

    println!(
        "route-warden {} (once={}, dry_run={}, config={})",
        route_warden::app_version(),
        cli.once,
        cli.dry_run,
        cli.config.display()
    );
}

fn default_clash_verge_dir() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let mut dir = std::path::PathBuf::from(home);
    dir.push("Library");
    dir.push("Application Support");
    dir.push("io.github.clash-verge-rev.clash-verge-rev");
    dir
}
