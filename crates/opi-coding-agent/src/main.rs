fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version" | "-V") => {
            println!("opi {}", env!("CARGO_PKG_VERSION"));
        }
        Some("--help" | "-h") => {
            println!("opi {} - AI coding agent", env!("CARGO_PKG_VERSION"));
            println!();
            println!("Usage: opi [OPTIONS]");
            println!();
            println!("Options:");
            println!("  -V, --version    Print version information");
            println!("  -h, --help       Print help");
        }
        Some(arg) => {
            eprintln!("opi: unknown argument '{arg}'");
            eprintln!("Try 'opi --help' for more information.");
            std::process::exit(2);
        }
        None => {
            println!("opi {} - AI coding agent", env!("CARGO_PKG_VERSION"));
            println!("(scaffolding release - interactive mode not yet implemented)");
        }
    }
}
