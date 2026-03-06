fn main() {
    let cli = route_warden::cli::parse();
    println!(
        "route-warden {} (once={}, dry_run={}, config={})",
        route_warden::app_version(),
        cli.once,
        cli.dry_run,
        cli.config.display()
    );
}
