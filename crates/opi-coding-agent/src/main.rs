use clap::Parser;

use opi_coding_agent::cli::Cli;
use opi_coding_agent::config::{ConfigSource, resolve_config};

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("opi {} - debug mode", env!("CARGO_PKG_VERSION"));
    }

    let config = match resolve_config(ConfigSource {
        cli_model: cli.model.clone(),
        config_path: cli.config.clone(),
        env_model: std::env::var("OPI_MODEL").ok(),
        project_dir: std::env::current_dir().ok(),
        user_config_path: None,
    }) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("opi: config error: {e}");
            std::process::exit(1);
        }
    };

    let prompt_text = cli.prompt.join(" ");

    if cli.non_interactive {
        eprintln!("opi: non-interactive mode not yet implemented");
        std::process::exit(1);
    } else if !prompt_text.is_empty() {
        eprintln!("opi: single-prompt mode not yet implemented");
        std::process::exit(1);
    } else {
        // Interactive mode (requires TTY — stub for now)
        println!("opi {} - AI coding agent", env!("CARGO_PKG_VERSION"));
        println!("(interactive mode not yet wired to TUI)");
    }

    let _ = config;
}
