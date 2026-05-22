use clap::Parser;

use opi_coding_agent::cli::Cli;
use opi_coding_agent::config::{ConfigSource, resolve_config};

fn main() {
    // Load .env if present (for local development/testing convenience).
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("opi {} - debug mode", env!("CARGO_PKG_VERSION"));
    }

    // Handle session CLI commands first — they don't need config or a provider.
    match opi_coding_agent::session_cli::handle_session_cli(
        cli.list_sessions,
        cli.resume.as_deref(),
        cli.delete_session.as_deref(),
    ) {
        Ok(true) => return,
        Ok(false) => {}
        Err(code) => std::process::exit(code),
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
        // Interactive mode — use TUI
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("opi: runtime error: {e}");
                std::process::exit(1);
            }
        };
        rt.block_on(async { run_interactive(&cli, &config).await });
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
        Err(ProviderBuildError::Auth(msg)) => {
            eprintln!("opi: {msg}");
            return ExitCode::AuthFailure as i32;
        }
        Err(ProviderBuildError::Config(msg)) => {
            eprintln!("opi: {msg}");
            return ExitCode::ConfigError as i32;
        }
    };

    let allow_mutating = cli.allow_mutating || config.defaults.allow_mutating_tools;

    let user_system_prompt =
        cli.system
            .as_ref()
            .and_then(|path| match std::fs::read_to_string(path) {
                Ok(content) => Some(content),
                Err(e) => {
                    eprintln!(
                        "opi: warning: failed to read system prompt file {}: {e}",
                        path.display()
                    );
                    None
                }
            });

    let mut runner = NonInteractiveRunner::new(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        std::env::current_dir().unwrap_or_default(),
        allow_mutating,
        user_system_prompt,
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

async fn run_interactive(cli: &Cli, config: &opi_coding_agent::config::OpiConfig) {
    use opi_coding_agent::harness::{CodingHarness, InteractiveCodingHooks};
    use opi_coding_agent::interactive;

    let provider = match build_provider(config) {
        Ok(p) => p,
        Err(ProviderBuildError::Auth(msg)) => {
            eprintln!("opi: {msg}");
            std::process::exit(3);
        }
        Err(ProviderBuildError::Config(msg)) => {
            eprintln!("opi: {msg}");
            std::process::exit(2);
        }
    };

    let allow_mutating = cli.allow_mutating || config.defaults.allow_mutating_tools;
    let user_system_prompt = cli
        .system
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok());

    let hooks = Box::new(InteractiveCodingHooks::new(allow_mutating));
    let harness = CodingHarness::new_with_hooks(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        std::env::current_dir().unwrap_or_default(),
        hooks,
        user_system_prompt,
    );

    let model_display = config.defaults.model.clone();
    if let Err(e) = interactive::run_interactive_tui(harness, model_display).await {
        eprintln!("opi: TUI error: {e}");
        std::process::exit(1);
    }
}

enum ProviderBuildError {
    Auth(String),
    Config(String),
}

fn build_provider(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<Box<dyn opi_ai::provider::Provider>, ProviderBuildError> {
    use opi_ai::anthropic::AnthropicProvider;
    use opi_ai::provider::Provider;

    let spec = &config.defaults.model;
    let (provider_id, _) = spec.split_once(':').ok_or_else(|| {
        ProviderBuildError::Config(format!(
            "invalid model spec: {spec:?} (expected provider:model)"
        ))
    })?;

    match provider_id {
        "anthropic" => {
            let api_key_env = &config.providers.anthropic.api_key_env;
            let api_key = std::env::var(api_key_env).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {api_key_env} environment variable"
                ))
            })?;
            let base_url = config.providers.anthropic.base_url.clone();
            let provider = AnthropicProvider::new(api_key, base_url);
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        other => Err(ProviderBuildError::Config(format!(
            "unknown provider: {other}"
        ))),
    }
}
