use std::sync::Arc;

use clap::Parser;

use opi_coding_agent::cli::Cli;
use opi_coding_agent::config::{ConfigSource, resolve_config};
use opi_coding_agent::harness::ResumeInfo;
use opi_coding_agent::policy::{ToolFlags, ToolSelection, resolve_tool_selection};

fn main() {
    // Load .env if present (for local development/testing convenience).
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    // Handle shell completion generation early — no config/provider needed.
    if let Some(shell) = cli.generate_completion {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        let shell: clap_complete::Shell = shell.into();
        clap_complete::generate(shell, &mut cmd, "opi", &mut std::io::stdout());
        return;
    }

    if cli.verbose {
        eprintln!("opi {} - debug mode", env!("CARGO_PKG_VERSION"));
    }

    // Handle session CLI commands first -- they don't need config or a provider.
    let (resumed_messages, resume_info) = match opi_coding_agent::session_cli::handle_session_cli(
        cli.list_sessions,
        cli.resume.as_deref(),
        cli.delete_session.as_deref(),
    ) {
        Ok((true, Some(session))) => {
            let msgs = opi_coding_agent::session_cli::reconstruct_context(&session.entries);
            let original_cwd = std::path::PathBuf::from(&session.header.cwd);
            let info = ResumeInfo {
                path: session.path,
                session_id: session.header.id,
                entries: session.entries,
                original_cwd,
            };
            (Some(msgs), Some(info))
        }
        Ok((true, None)) => return,              // list/delete handled
        Ok((_, None | Some(_))) => (None, None), // no session command or unreachable
        Err(code) => std::process::exit(code),
    };

    let config = match resolve_config(ConfigSource {
        cli_model: cli.model.clone(),
        config_path: cli.config.clone(),
        env_model: std::env::var("OPI_MODEL").ok(),
        project_dir: resume_info
            .as_ref()
            .map(|info| info.original_cwd.clone())
            .or_else(|| std::env::current_dir().ok()),
        user_config_path: None,
    }) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("opi: config error: {e}");
            std::process::exit(2);
        }
    };

    let prompt_text = cli.prompt.join(" ");

    let tool_selection = resolve_tool_selection(ToolFlags {
        tools: cli.tools.clone(),
        no_tools: cli.no_tools,
        no_builtin_tools: cli.no_builtin_tools,
    });

    if cli.non_interactive || cli.json || !prompt_text.is_empty() {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("opi: runtime error: {e}");
                std::process::exit(1);
            }
        };

        let exit_code = rt.block_on(async {
            run_non_interactive(
                &cli,
                &config,
                &prompt_text,
                resumed_messages,
                resume_info,
                tool_selection,
            )
            .await
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
        rt.block_on(async {
            run_interactive(&cli, &config, resumed_messages, resume_info, tool_selection).await
        });
    }
}

async fn run_non_interactive(
    cli: &Cli,
    config: &opi_coding_agent::config::OpiConfig,
    prompt_text: &str,
    resumed_messages: Option<Vec<opi_agent::message::AgentMessage>>,
    resume_info: Option<ResumeInfo>,
    tool_selection: ToolSelection,
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

    let workspace_root = resume_info
        .as_ref()
        .map(|info| info.original_cwd.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let mut runner = NonInteractiveRunner::new_with_resume(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        workspace_root,
        allow_mutating,
        user_system_prompt,
        resumed_messages.unwrap_or_default(),
        resume_info,
        tool_selection,
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
    resume_info: Option<ResumeInfo>,
    tool_selection: ToolSelection,
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
    let workspace_root = resume_info
        .as_ref()
        .map(|info| info.original_cwd.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let harness = CodingHarness::new_with_hooks_and_resume(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        workspace_root,
        hooks,
        user_system_prompt,
        initial_messages,
        resume_info,
        tool_selection,
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
            let api_key = require_api_key(env_name)?;
            let provider = opi_ai::anthropic::AnthropicProvider::new(
                api_key,
                config.providers.anthropic.base_url.clone(),
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openai" => {
            let env_name = resolve_env_name(&config.providers.openai.api_key_env, "OPENAI_API_KEY");
            let api_key = require_api_key(&env_name)?;
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
            let api_key = require_api_key(&env_name)?;
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
            let api_key = require_api_key(&env_name)?;
            let provider = opi_ai::mistral::mistral_provider(
                api_key,
                config.providers.mistral.base_url.clone(),
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openai-responses" => {
            let env_name = resolve_env_name(
                &config.providers.openai_responses.api_key_env,
                "OPENAI_API_KEY",
            );
            let api_key = require_api_key(&env_name)?;
            let provider = opi_ai::openai_responses::OpenAiResponsesProvider::new(
                api_key,
                config.providers.openai_responses.base_url.clone(),
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "gemini" => {
            let env_name = resolve_env_name(&config.providers.gemini.api_key_env, "GEMINI_API_KEY");
            let api_key = require_api_key(&env_name)?;
            let provider = opi_ai::gemini::GeminiProvider::new(
                api_key,
                config.providers.gemini.base_url.clone(),
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "bedrock" => {
            let bedrock_config = &config.providers.bedrock;

            // Resolve credentials: config > env > profile
            let (akid, sak, token, env_region) = resolve_bedrock_env_credentials();
            let profile_name = bedrock_config.profile.as_deref();
            let credentials_file = default_aws_credentials_path();

            // Read secret key from configured env var
            let secret_key = bedrock_config
                .secret_access_key_env
                .as_deref()
                .and_then(|env_name| std::env::var(env_name).ok());

            // Read session token from configured env var
            let session_token = bedrock_config
                .session_token_env
                .as_deref()
                .and_then(|env_name| std::env::var(env_name).ok());

            let input = opi_ai::bedrock::credentials::CredentialResolutionInput {
                config_access_key_id: bedrock_config.access_key_id.as_deref(),
                config_secret_access_key: secret_key.as_deref(),
                config_session_token: session_token.as_deref(),
                config_region: bedrock_config.region.as_deref(),
                env_access_key_id: akid.as_deref(),
                env_secret_access_key: sak.as_deref(),
                env_session_token: token.as_deref(),
                env_region: env_region.as_deref(),
                profile_name,
                credentials_file_path: credentials_file.as_deref(),
            };

            let resolved = opi_ai::bedrock::credentials::resolve_credentials(&input);

            let (bedrock_creds, _source) = resolved.ok_or_else(|| {
                ProviderBuildError::Auth(
                    "no AWS credentials found: set AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY env vars, configure [providers.bedrock], or set up ~/.aws/credentials".into(),
                )
            })?;

            let provider = opi_ai::bedrock::BedrockProvider::from_credentials(
                bedrock_creds,
                bedrock_config.base_url.clone(),
                Arc::new(opi_ai::http::HttpClient::new()),
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "azure" => {
            let azure_config = &config.providers.azure;
            let env_name = resolve_env_name(&azure_config.api_key_env, "AZURE_OPENAI_API_KEY");
            let api_key = require_api_key(&env_name)?;

            // Extract deployment name from model spec (azure:deployment-name)
            let deployment = spec.split_once(':').map(|(_, id)| id).unwrap_or("");

            let provider = if azure_config.deployments.is_empty() {
                opi_ai::azure_openai::AzureOpenAIProvider::new(
                    api_key,
                    azure_config.endpoint.clone(),
                    deployment.to_string(),
                    azure_config.api_version.clone(),
                )
            } else {
                opi_ai::azure_openai::AzureOpenAIProvider::from_config(
                    api_key,
                    azure_config.endpoint.clone(),
                    azure_config.deployments.clone(),
                    azure_config.api_version.clone(),
                )
            }
            .with_client(Arc::new(opi_ai::http::HttpClient::new()));
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

fn require_api_key(env_name: &str) -> Result<String, ProviderBuildError> {
    let key = std::env::var(env_name).map_err(|_| {
        ProviderBuildError::Auth(format!(
            "missing API key: set {env_name} environment variable"
        ))
    })?;
    if key.trim().is_empty() {
        return Err(ProviderBuildError::Auth(format!(
            "empty API key: {env_name} is set but empty"
        )));
    }
    Ok(key)
}

/// Read AWS credentials from environment variables.
fn resolve_bedrock_env_credentials() -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let akid = std::env::var("AWS_ACCESS_KEY_ID").ok();
    let sak = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
    let token = std::env::var("AWS_SESSION_TOKEN").ok();
    let region = std::env::var("AWS_REGION")
        .ok()
        .or_else(|| std::env::var("AWS_DEFAULT_REGION").ok());
    (akid, sak, token, region)
}

/// Default path for ~/.aws/credentials.
fn default_aws_credentials_path() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".aws").join("credentials"))
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
