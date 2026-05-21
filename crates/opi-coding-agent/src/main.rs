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
            std::process::exit(2);
        }
    };

    let prompt_text = cli.prompt.join(" ");

    if cli.non_interactive || !prompt_text.is_empty() {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("opi: runtime error: {e}");
                std::process::exit(1);
            }
        };

        let exit_code =
            rt.block_on(async { run_non_interactive(&cli, &config, &prompt_text).await });
        std::process::exit(exit_code);
    } else {
        // Interactive mode (requires TTY — stub for now)
        println!("opi {} - AI coding agent", env!("CARGO_PKG_VERSION"));
        println!("(interactive mode not yet wired to TUI)");
    }
}

async fn run_non_interactive(
    cli: &Cli,
    config: &opi_coding_agent::config::OpiConfig,
    prompt_text: &str,
) -> i32 {
    use opi_coding_agent::runner::{ExitCode, NonInteractiveRunner};

    if prompt_text.is_empty() {
        eprintln!("opi: no prompt provided");
        return ExitCode::ConfigError as i32;
    }

    let provider = match build_provider(config) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("opi: {e}");
            return ExitCode::ConfigError as i32;
        }
    };

    let allow_mutating = cli.allow_mutating || config.defaults.allow_mutating_tools;

    let mut runner = NonInteractiveRunner::new(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        std::env::current_dir().unwrap_or_default(),
        allow_mutating,
    );

    let result = runner.run(prompt_text).await;

    if !result.stdout.is_empty() {
        print!("{}", result.stdout);
    }
    if !result.stderr.is_empty() {
        eprintln!("{}", result.stderr);
    }

    result.exit_code
}

fn build_provider(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<Box<dyn opi_ai::provider::Provider>, String> {
    use opi_ai::anthropic::AnthropicProvider;
    use opi_ai::provider::Provider;

    let spec = &config.defaults.model;
    let (provider_id, _) = spec
        .split_once(':')
        .ok_or_else(|| format!("invalid model spec: {spec:?} (expected provider:model)"))?;

    match provider_id {
        "anthropic" => {
            let api_key_env = &config.providers.anthropic.api_key_env;
            let api_key = std::env::var(api_key_env)
                .map_err(|_| format!("missing API key: set {api_key_env} environment variable"))?;
            let provider = AnthropicProvider::new(api_key, None);
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        other => Err(format!("unknown provider: {other}")),
    }
}
