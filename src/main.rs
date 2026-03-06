#[tokio::main]
async fn main() {
    let cli = route_warden::cli::parse();

    if let Some(command) = cli.command.clone() {
        match command {
            route_warden::cli::Command::SyncRwProfile(args) => {
                let verge_dir = args.verge_dir.unwrap_or_else(default_clash_verge_dir);
                let config = match route_warden::config::load_from_path(&cli.config) {
                    Ok(v) => v,
                    Err(err) => {
                        eprintln!("load config failed: {err:#}");
                        std::process::exit(1);
                    }
                };
                match route_warden::clash_verge::sync_rw_profile(
                    &verge_dir,
                    &config,
                    args.all,
                    args.dry_run,
                ) {
                    Ok(files) => {
                        println!(
                            "sync-rw-profile done (dry_run={}, files={})",
                            args.dry_run,
                            files.len()
                        );
                        for file in files {
                            println!("- {}", file.display());
                        }
                    }
                    Err(err) => {
                        eprintln!("sync-rw-profile failed: {err:#}");
                        std::process::exit(1);
                    }
                }
                return;
            }
        }
    }

    if let Err(err) = route_warden::app::run(cli).await {
        eprintln!("route-warden failed: {err:#}");
        std::process::exit(1);
    }
}

fn default_clash_verge_dir() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let mut dir = std::path::PathBuf::from(home);
    dir.push("Library");
    dir.push("Application Support");
    dir.push("io.github.clash-verge-rev.clash-verge-rev");
    dir
}
