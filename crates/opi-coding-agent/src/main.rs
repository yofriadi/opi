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

    // Handle session CLI commands first -- they don't need config or a provider.
    let resumed_messages = match opi_coding_agent::session_cli::handle_session_cli(
        cli.list_sessions,
        cli.resume.as_deref(),
        cli.delete_session.as_deref(),
    ) {
        Ok((true, Some(session))) => {
            Some(opi_coding_agent::session_cli::reconstruct_context(
                &session.entries,
            ))
        }
        Ok((true, None)) => return,         // list/delete handled
        Ok((_, None | Some(_))) => None,    // no session command or unreachable
        Err(code) => std::process::exit(code),
    };

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

    if cli.non_interactive || cli.json || !prompt_text.is_empty() {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("opi: runtime error: {e}");
                std::process::exit(1);
            }
        };

        let exit_code = rt.block_on(async {
            run_non_interactive(&cli, &config, &prompt_text, resumed_messages).await
        });
        std::process::exit(exit_code);
    } else {
        // Interactive mode -- use TUI
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("opi: runtime error: {e}");
                std::process::exit(1);
            }
        };
        rt.block_on(async { run_interactive(&cli, &config, resumed_messages).await });
    }
}

async fn run_non_interactive(
    cli: &Cli,
    config: &opi_coding_agent::config::OpiConfig,
    prompt_text: &str,
    resumed_messages: Option<Vec<opi_agent::message::AgentMessage>>,
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
        resumed_messages.unwrap_or_default(),
    );

    let result = if cli.json {
        runner.run_json(prompt_text).await
    } else {
        runner.run(prompt_text).await
    };

    if !result.stdout.is_empty() {
        print!("{}", result.stdout);
    }
    if !result.stderr.is_empty() {
        eprintln!("{}", result.stderr);
    }

    result.exit_code
}

async fn run_interactive(
    cli: &Cli,
    config: &opi_coding_agent::config::OpiConfig,
    resumed_messages: Option<Vec<opi_agent::message::AgentMessage>>,
) {
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
    let initial_messages = resumed_messages.unwrap_or_default();
    let harness = CodingHarness::new_with_hooks(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        std::env::current_dir().unwrap_or_default(),
        hooks,
        user_system_prompt,
        initial_messages,
    );

    let model_display = config.defaults.model.clone();
    let theme_name = config.defaults.theme.clone();
    let keybindings = parse_keybindings(&config.keybindings);
    if let Err(e) =
        interactive::run_interactive_tui(harness, model_display, &theme_name, keybindings).await
    {
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
    use opi_ai::provider::Provider;

    let spec = &config.defaults.model;
    let (provider_id, _) = spec.split_once(':').ok_or_else(|| {
        ProviderBuildError::Config(format!(
            "invalid model spec: {spec:?} (expected provider:model)"
        ))
    })?;

    match provider_id {
        "anthropic" => {
            let env_name = &config.providers.anthropic.api_key_env;
            let api_key = std::env::var(env_name).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {env_name} environment variable"
                ))
            })?;
            let provider =
                opi_ai::anthropic::AnthropicProvider::new(api_key, config.providers.anthropic.base_url.clone());
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openai" => {
            let env_name =
                resolve_env_name(&config.providers.openai.api_key_env, "OPENAI_API_KEY");
            let api_key = std::env::var(&env_name).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {env_name} environment variable"
                ))
            })?;
            let provider = opi_ai::openai_chat::OpenAiChatProvider::new(
                api_key,
                config.providers.openai.base_url.clone(),
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openrouter" => {
            let env_name = resolve_env_name(
                &config.providers.openrouter.api_key_env,
                "OPENROUTER_API_KEY",
            );
            let api_key = std::env::var(&env_name).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {env_name} environment variable"
                ))
            })?;
            // If a custom referer is configured, build the provider directly with it.
            let provider = if let Some(ref referer) = config.providers.openrouter.referer {
                let base_url = config
                    .providers
                    .openrouter
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://openrouter.ai/api".into());
                let compat = opi_ai::openai_chat::CompatConfig::default();
                let extra_headers = vec![
                    ("HTTP-Referer".into(), referer.clone()),
                    ("X-Title".into(), "opi".into()),
                ];
                // Use the default model list from the openrouter module.
                let temp = opi_ai::openrouter::openrouter_provider(
                    String::new(),
                    config.providers.openrouter.base_url.clone(),
                );
                let models = temp.models().to_vec();
                opi_ai::openai_chat::OpenAiChatProvider::new_for_profile(
                    api_key,
                    base_url,
                    "openrouter".into(),
                    compat,
                    extra_headers,
                    models,
                )
            } else {
                opi_ai::openrouter::openrouter_provider(
                    api_key,
                    config.providers.openrouter.base_url.clone(),
                )
            };
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "mistral" => {
            let env_name =
                resolve_env_name(&config.providers.mistral.api_key_env, "MISTRAL_API_KEY");
            let api_key = std::env::var(&env_name).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {env_name} environment variable"
                ))
            })?;
            let provider =
                opi_ai::mistral::mistral_provider(api_key, config.providers.mistral.base_url.clone());
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openai-responses" => {
            let env_name = resolve_env_name(
                &config.providers.openai_responses.api_key_env,
                "OPENAI_API_KEY",
            );
            let api_key = std::env::var(&env_name).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {env_name} environment variable"
                ))
            })?;
            let provider = opi_ai::openai_responses::OpenAiResponsesProvider::new(
                api_key,
                config.providers.openai_responses.base_url.clone(),
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "gemini" => {
            let env_name = resolve_env_name(&config.providers.gemini.api_key_env, "GEMINI_API_KEY");
            let api_key = std::env::var(&env_name).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {env_name} environment variable"
                ))
            })?;
            let provider = opi_ai::gemini::GeminiProvider::new(
                api_key,
                config.providers.gemini.base_url.clone(),
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        other => Err(ProviderBuildError::Config(format!(
            "unknown provider: {other}"
        ))),
    }
}

fn resolve_env_name(configured: &str, default: &str) -> String {
    if configured.is_empty() {
        default.into()
    } else {
        configured.into()
    }
}

fn parse_keybindings(config: &opi_coding_agent::config::KeybindingsConfig) -> opi_tui::Keybindings {
    use std::collections::HashMap;

    let map = HashMap::from([
        ("submit".to_string(), config.submit.clone()),
        ("abort".to_string(), config.abort.clone()),
        ("new_line".to_string(), config.new_line.clone()),
    ]);
    match opi_tui::Keybindings::from_config_map(&map) {
        Ok(kb) => kb,
        Err(e) => {
            eprintln!("opi: warning: invalid keybindings config ({e}), using defaults");
            opi_tui::Keybindings::default()
        }
    }
}
