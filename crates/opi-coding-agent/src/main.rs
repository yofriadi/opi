use std::sync::Arc;

use clap::Parser;
use opi_ai::provider::Provider;

use opi_coding_agent::cli::Cli;
use opi_coding_agent::config::{ConfigSource, resolve_config};
use opi_coding_agent::harness::ResumeInfo;
use opi_coding_agent::policy::{
    RunMode, ToolFlags, ToolRuntimeConfig, ToolSelection, resolve_tool_selection,
};

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
    if let Some(cmd) = &cli.command {
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

        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("opi: runtime error: {e}");
                std::process::exit(1);
            }
        };

        match cmd {
            opi_coding_agent::cli::CliCommand::Login(login_cmd) => {
                if let Some(opi_coding_agent::cli::LoginSubcommand::Status) = &login_cmd.subcommand
                {
                    if opi_coding_agent::auth::login::login_status().is_err() {
                        std::process::exit(1);
                    }
                    return;
                }

                let issuer = Some(config.providers.openai_codex.issuer.as_str());
                let client_id = Some(config.providers.openai_codex.client_id.as_str());

                let result = if login_cmd.device {
                    rt.block_on(opi_coding_agent::auth::login::login_device(
                        issuer,
                        client_id,
                        config.providers.openai_codex.proxy.as_ref(),
                    ))
                } else {
                    rt.block_on(opi_coding_agent::auth::login::login_browser(
                        issuer,
                        client_id,
                        config.providers.openai_codex.proxy.as_ref(),
                    ))
                };

                if let Err(e) = result {
                    eprintln!("Login failed: {}", e);
                    std::process::exit(1);
                }
                return;
            }
            opi_coding_agent::cli::CliCommand::Logout => {
                if let Err(e) = rt.block_on(opi_coding_agent::auth::login::logout(
                    config.providers.openai_codex.proxy.as_ref(),
                )) {
                    eprintln!("Logout failed: {}", e);
                    std::process::exit(1);
                }
                return;
            }
        }
    }

    if cli.verbose {
        eprintln!("opi {} - debug mode", env!("CARGO_PKG_VERSION"));
    }

    // Handle --list-models early -- needs config but not a full provider session.
    if cli.list_models {
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
        let exit_code = list_models(&config, cli.json);
        std::process::exit(exit_code);
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

    // RPC mode: bidirectional JSONL protocol over stdin/stdout.
    if cli.rpc {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("opi: runtime error: {e}");
                std::process::exit(1);
            }
        };
        let exit_code =
            rt.block_on(async { run_rpc(&cli, &config, resumed_messages, tool_selection).await });
        std::process::exit(exit_code);
    } else if cli.non_interactive || cli.json || !prompt_text.is_empty() {
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
        Err(ProviderBuildError::Provider(e)) => {
            eprintln!("opi: {e}");
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

    let mut runner = match NonInteractiveRunner::new_with_resume(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        workspace_root,
        allow_mutating,
        user_system_prompt,
        resumed_messages.unwrap_or_default(),
        resume_info,
        tool_selection,
    ) {
        Ok(runner) => runner,
        Err(e) => {
            eprintln!("opi: {e}");
            return ExitCode::ConfigError as i32;
        }
    };

    let result = if cli.image.is_empty() {
        // No images -- use the plain text path.
        if cli.json {
            runner.run_json(prompt_text).await
        } else {
            runner.run(prompt_text).await
        }
    } else {
        // Load images and combine with text prompt.
        let mut content: Vec<opi_ai::message::InputContent> = Vec::new();
        content.push(opi_ai::message::InputContent::Text {
            text: prompt_text.to_owned(),
        });
        for image_path in &cli.image {
            match opi_coding_agent::image::load_image_with_limit(
                image_path,
                config.defaults.max_image_bytes,
            ) {
                Ok(img) => content.push(img),
                Err(e) => {
                    eprintln!("opi: {e}");
                    return ExitCode::ConfigError as i32;
                }
            }
        }
        if cli.json {
            runner.run_json_with_content(content).await
        } else {
            runner.run_with_content(content).await
        }
    };

    if !result.stdout.is_empty() {
        print!("{}", result.stdout);
    }
    if !result.stderr.is_empty() {
        eprintln!("{}", result.stderr);
    }

    result.exit_code
}

async fn run_rpc(
    cli: &Cli,
    config: &opi_coding_agent::config::OpiConfig,
    resumed_messages: Option<Vec<opi_agent::message::AgentMessage>>,
    tool_selection: ToolSelection,
) -> i32 {
    use opi_coding_agent::rpc::RpcRunner;
    use opi_coding_agent::runner::ExitCode;

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
        Err(ProviderBuildError::Provider(e)) => {
            eprintln!("opi: {e}");
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

    let workspace_root = std::env::current_dir().unwrap_or_default();

    let mut runner = match RpcRunner::new(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        workspace_root,
        allow_mutating,
        tool_selection,
        user_system_prompt,
        resumed_messages.unwrap_or_default(),
    ) {
        Ok(runner) => runner,
        Err(e) => {
            eprintln!("opi: {e}");
            return ExitCode::ConfigError as i32;
        }
    };

    runner.run().await
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
        Err(ProviderBuildError::Provider(e)) => {
            eprintln!("opi: {e}");
            std::process::exit(2);
        }
    };

    let user_system_prompt = cli
        .system
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok());

    let hooks = Box::new(InteractiveCodingHooks::new(true));
    let initial_messages = resumed_messages.unwrap_or_default();
    let workspace_root = resume_info
        .as_ref()
        .map(|info| info.original_cwd.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let tool_config = ToolRuntimeConfig::resolve(RunMode::Interactive, true, tool_selection)
        .expect("interactive tool config should be valid");
    let harness = CodingHarness::new_with_hooks_and_resume_tool_config(
        provider,
        config.defaults.model.clone(),
        config.clone(),
        workspace_root,
        hooks,
        user_system_prompt,
        initial_messages,
        resume_info,
        tool_config,
    );

    let mut harness = harness;

    // Load --image files for the first interactive prompt.
    if !cli.image.is_empty() {
        let mut images = Vec::new();
        for image_path in &cli.image {
            match opi_coding_agent::image::load_image_with_limit(
                image_path,
                config.defaults.max_image_bytes,
            ) {
                Ok(img) => images.push(img),
                Err(e) => {
                    eprintln!("opi: {e}");
                    std::process::exit(2);
                }
            }
        }
        harness.queue_images(images);
    }

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
    Provider(opi_ai::provider::ProviderError),
}

/// Error from lightweight provider builders used by `--list-models`.
///
/// `MissingCredentials` — the provider has no API key / credentials
/// configured; skip silently and try the next provider.
///
/// `Config` — the config file contains a broken setting (e.g. invalid
/// proxy URL); report the error and exit.
enum ListModelsError {
    MissingCredentials,
    Config(String),
}

impl From<opi_ai::provider::ProviderError> for ProviderBuildError {
    fn from(e: opi_ai::provider::ProviderError) -> Self {
        ProviderBuildError::Provider(e)
    }
}

impl std::fmt::Display for ProviderBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderBuildError::Auth(msg) => write!(f, "{msg}"),
            ProviderBuildError::Config(msg) => write!(f, "{msg}"),
            ProviderBuildError::Provider(e) => write!(f, "{e}"),
        }
    }
}

fn build_http_client(
    proxy_config: Option<&opi_coding_agent::config::ProviderProxyConfig>,
) -> Result<Arc<opi_ai::http::HttpClient>, ProviderBuildError> {
    opi_coding_agent::config::build_http_client(proxy_config).map_err(|e| {
        ProviderBuildError::Config(format!(
            "failed to build HTTP client with proxy config: {e}"
        ))
    })
}

fn build_provider(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<Box<dyn opi_ai::provider::Provider>, ProviderBuildError> {
    let spec = &config.defaults.model;
    let (provider_id, _) = spec.split_once(':').ok_or_else(|| {
        ProviderBuildError::Config(format!(
            "invalid model spec: {spec:?} (expected provider:model)"
        ))
    })?;

    build_runtime_provider(config, provider_id)
}

fn build_runtime_provider(
    config: &opi_coding_agent::config::OpiConfig,
    provider_id: &str,
) -> Result<Box<dyn opi_ai::provider::Provider>, ProviderBuildError> {
    use opi_ai::provider::Provider;

    let spec = &config.defaults.model;
    match provider_id {
        "openai-codex" => {
            let (access_token, account_id) =
                match block_on_async(opi_coding_agent::auth::refresh::get_valid_token(
                    config.providers.openai_codex.proxy.as_ref(),
                )) {
                    Ok(creds) => creds,
                    Err(e) => return Err(ProviderBuildError::Auth(e.to_string())),
                };
            let client = build_http_client(config.providers.openai_codex.proxy.as_ref())?;
            let provider = opi_ai::openai_codex::OpenAiCodexProvider::new(
                access_token,
                account_id,
                config.providers.openai_codex.base_url.clone(),
                client,
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "anthropic" => {
            let env_name = &config.providers.anthropic.api_key_env;
            let api_key = require_api_key(env_name)?;
            let client = build_http_client(config.providers.anthropic.proxy.as_ref())?;
            let provider = opi_ai::anthropic::AnthropicProvider::with_client(
                api_key,
                config.providers.anthropic.base_url.clone(),
                client,
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openai" => {
            let env_name = resolve_env_name(&config.providers.openai.api_key_env, "OPENAI_API_KEY");
            let api_key = require_api_key(&env_name)?;
            let client = build_http_client(config.providers.openai.proxy.as_ref())?;
            let provider = opi_ai::openai_chat::OpenAiChatProvider::with_client(
                api_key,
                config.providers.openai.base_url.clone(),
                "openai".into(),
                vec![],
                client,
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openrouter" => {
            let env_name = resolve_env_name(
                &config.providers.openrouter.api_key_env,
                "OPENROUTER_API_KEY",
            );
            let api_key = require_api_key(&env_name)?;
            let client = build_http_client(config.providers.openrouter.proxy.as_ref())?;
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
                .with_shared_client(client)
            } else {
                opi_ai::openrouter::openrouter_provider(
                    api_key,
                    config.providers.openrouter.base_url.clone(),
                )
                .with_shared_client(client)
            };
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "mistral" => {
            let env_name =
                resolve_env_name(&config.providers.mistral.api_key_env, "MISTRAL_API_KEY");
            let api_key = require_api_key(&env_name)?;
            let client = build_http_client(config.providers.mistral.proxy.as_ref())?;
            let provider = opi_ai::mistral::mistral_provider(
                api_key,
                config.providers.mistral.base_url.clone(),
            )
            .with_shared_client(client);
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openai-responses" => {
            let env_name = resolve_env_name(
                &config.providers.openai_responses.api_key_env,
                "OPENAI_API_KEY",
            );
            let api_key = require_api_key(&env_name)?;
            let client = build_http_client(config.providers.openai_responses.proxy.as_ref())?;
            let provider = opi_ai::openai_responses::OpenAiResponsesProvider::with_client(
                api_key,
                config.providers.openai_responses.base_url.clone(),
                client,
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "gemini" => {
            let env_name = resolve_env_name(&config.providers.gemini.api_key_env, "GEMINI_API_KEY");
            let api_key = require_api_key(&env_name)?;
            let client = build_http_client(config.providers.gemini.proxy.as_ref())?;
            let provider = opi_ai::gemini::GeminiProvider::with_client(
                api_key,
                config.providers.gemini.base_url.clone(),
                client,
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "bedrock" => {
            let bedrock_config = &config.providers.bedrock;

            // Resolve credentials: config > env > profile
            let (akid, sak, token, env_region) = resolve_bedrock_env_credentials();
            let env_profile = std::env::var("AWS_PROFILE").ok();
            let profile_name = bedrock_config.profile.as_deref().or(env_profile.as_deref());
            let credentials_file = aws_credentials_path();
            let config_file = aws_config_path();

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
                config_file_path: config_file.as_deref(),
            };

            let resolved = opi_ai::bedrock::credentials::resolve_credentials(&input);

            let (bedrock_creds, _source) = resolved.ok_or_else(|| {
                ProviderBuildError::Auth(
                    "no AWS credentials found: set AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY env vars, configure [providers.bedrock], or set up AWS shared credentials/config profiles".into(),
                )
            })?;

            let client = build_http_client(bedrock_config.proxy.as_ref())?;
            let provider = opi_ai::bedrock::BedrockProvider::from_credentials(
                bedrock_creds,
                bedrock_config.base_url.clone(),
                client,
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
                )?
            } else {
                opi_ai::azure_openai::AzureOpenAIProvider::from_config(
                    api_key,
                    azure_config.endpoint.clone(),
                    azure_config.deployments.clone(),
                    azure_config.api_version.clone(),
                )?
            }
            .with_client(build_http_client(azure_config.proxy.as_ref())?);
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "vertex" => {
            let vertex_config = &config.providers.vertex;
            let env_name = resolve_env_name(&vertex_config.access_token_env, "VERTEX_ACCESS_TOKEN");
            let access_token = require_api_key(&env_name)?;

            let project = vertex_config.project.as_deref().ok_or_else(|| {
                ProviderBuildError::Config("vertex provider requires project".into())
            })?;
            let location = vertex_config.location.as_deref().ok_or_else(|| {
                ProviderBuildError::Config("vertex provider requires location".into())
            })?;

            let provider = if vertex_config.models.is_empty() {
                opi_ai::vertex::VertexProvider::new(
                    access_token,
                    project.into(),
                    location.into(),
                    vertex_config.base_url.clone(),
                )
            } else {
                opi_ai::vertex::VertexProvider::from_config(
                    access_token,
                    project.into(),
                    location.into(),
                    vertex_config.models.clone(),
                    vertex_config.base_url.clone(),
                )
            }
            .with_client(build_http_client(vertex_config.proxy.as_ref())?);
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

/// AWS shared credentials file path.
fn aws_credentials_path() -> Option<std::path::PathBuf> {
    std::env::var("AWS_SHARED_CREDENTIALS_FILE")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".aws").join("credentials")))
}

/// AWS shared config file path.
fn aws_config_path() -> Option<std::path::PathBuf> {
    std::env::var("AWS_CONFIG_FILE")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".aws").join("config")))
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(std::path::PathBuf::from)
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

/// List available models from all configured providers.
/// Returns exit code: 0 on success, 1 if no models found, 2 on config error.
fn list_models(config: &opi_coding_agent::config::OpiConfig, json_output: bool) -> i32 {
    let registry = match build_list_models_registry(config) {
        Ok(registry) => registry,
        Err(ListModelsError::MissingCredentials) => {
            eprintln!("opi: no models available (configure API keys to list models)");
            return 1;
        }
        Err(ListModelsError::Config(msg)) => {
            eprintln!("opi: config error: {msg}");
            return 2;
        }
    };
    let entries = opi_coding_agent::model_listing::model_entries_from_registry(&registry);

    if entries.is_empty() {
        eprintln!("opi: no models available (configure API keys to list models)");
        return 1;
    }

    if json_output {
        for entry in &entries {
            let json = serde_json::json!({
                "model": entry.model_id,
                "provider": entry.provider_id,
                "display_name": entry.display_name,
            });
            println!("{json}");
        }
    } else {
        // Compute column widths
        let max_id = entries.iter().map(|e| e.model_id.len()).max().unwrap_or(10);
        let max_name = entries
            .iter()
            .map(|e| e.display_name.len())
            .max()
            .unwrap_or(12);
        let max_prov = entries
            .iter()
            .map(|e| e.provider_id.len())
            .max()
            .unwrap_or(8);

        // Header
        println!(
            "{:<width_prov$}  {:<width_id$}  DISPLAY NAME",
            "PROVIDER",
            "MODEL ID",
            width_prov = max_prov,
            width_id = max_id,
        );
        println!(
            "{}  {}  {}",
            "-".repeat(max_prov),
            "-".repeat(max_id),
            "-".repeat(max_name),
        );

        for entry in &entries {
            println!(
                "{:<width_prov$}  {:<width_id$}  {}",
                entry.provider_id,
                entry.model_id,
                entry.display_name,
                width_prov = max_prov,
                width_id = max_id,
            );
        }
    }

    0
}

// Lightweight provider builders for --list-models.
// These try to construct providers but silently fail on missing auth.

const BUILT_IN_PROVIDER_IDS: &[&str] = &[
    "anthropic",
    "openai",
    "openrouter",
    "mistral",
    "openai-responses",
    "gemini",
    "bedrock",
    "azure",
    "vertex",
    "openai-codex",
];

fn build_list_models_registry(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::ProviderRegistry, ListModelsError> {
    let mut registry = opi_ai::ProviderRegistry::new();
    for provider_id in BUILT_IN_PROVIDER_IDS {
        match build_list_models_provider(config, provider_id) {
            Ok(provider) => registry
                .register_provider(provider)
                .map_err(|e| ListModelsError::Config(e.to_string()))?,
            Err(ListModelsError::MissingCredentials) => {}
            Err(e @ ListModelsError::Config(_)) => return Err(e),
        }
    }
    Ok(registry)
}

fn build_list_models_provider(
    config: &opi_coding_agent::config::OpiConfig,
    provider_id: &str,
) -> Result<Box<dyn Provider>, ListModelsError> {
    match provider_id {
        "anthropic" => Ok(Box::new(build_anthropic(config)?) as Box<dyn Provider>),
        "openai" => Ok(Box::new(build_openai(config)?) as Box<dyn Provider>),
        "openrouter" => Ok(Box::new(build_openrouter(config)?) as Box<dyn Provider>),
        "mistral" => Ok(Box::new(build_mistral(config)?) as Box<dyn Provider>),
        "openai-responses" => Ok(Box::new(build_openai_responses(config)?) as Box<dyn Provider>),
        "gemini" => Ok(Box::new(build_gemini(config)?) as Box<dyn Provider>),
        "bedrock" => Ok(Box::new(build_bedrock(config)?) as Box<dyn Provider>),
        "azure" => Ok(Box::new(build_azure(config)?) as Box<dyn Provider>),
        "vertex" => Ok(Box::new(build_vertex(config)?) as Box<dyn Provider>),
        "openai-codex" => match opi_coding_agent::auth::storage::load_auth() {
            Ok(None) => Err(ListModelsError::MissingCredentials),
            Err(e) => Err(ListModelsError::Config(format!(
                "OpenAI Codex credentials file is malformed: {e}"
            ))),
            Ok(Some(_)) => {
                let (access_token, account_id) =
                    match block_on_async(opi_coding_agent::auth::refresh::get_valid_token(
                        config.providers.openai_codex.proxy.as_ref(),
                    )) {
                        Ok(creds) => creds,
                        Err(e) => {
                            return Err(ListModelsError::Config(format!(
                                "OpenAI Codex auth refresh failed: {e}"
                            )));
                        }
                    };
                let client = build_http_client(config.providers.openai_codex.proxy.as_ref())
                    .map_err(|e| ListModelsError::Config(e.to_string()))?;
                let provider = opi_ai::openai_codex::OpenAiCodexProvider::new(
                    access_token,
                    account_id,
                    config.providers.openai_codex.base_url.clone(),
                    client,
                );
                Ok(Box::new(provider) as Box<dyn Provider>)
            }
        },
        other => Err(ListModelsError::Config(format!(
            "unknown provider in built-in list: {other}"
        ))),
    }
}

fn build_anthropic(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::anthropic::AnthropicProvider, ListModelsError> {
    let api_key = std::env::var(&config.providers.anthropic.api_key_env)
        .map_err(|_| ListModelsError::MissingCredentials)?;
    let client = build_http_client(config.providers.anthropic.proxy.as_ref())
        .map_err(|e| ListModelsError::Config(e.to_string()))?;
    Ok(opi_ai::anthropic::AnthropicProvider::with_client(
        api_key,
        config.providers.anthropic.base_url.clone(),
        client,
    ))
}

fn build_openai(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::openai_chat::OpenAiChatProvider, ListModelsError> {
    let env_name = resolve_env_name(&config.providers.openai.api_key_env, "OPENAI_API_KEY");
    let api_key = std::env::var(&env_name).map_err(|_| ListModelsError::MissingCredentials)?;
    let client = build_http_client(config.providers.openai.proxy.as_ref())
        .map_err(|e| ListModelsError::Config(e.to_string()))?;
    Ok(opi_ai::openai_chat::OpenAiChatProvider::with_client(
        api_key,
        config.providers.openai.base_url.clone(),
        "openai".into(),
        vec![],
        client,
    ))
}

fn build_openrouter(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::openai_chat::OpenAiChatProvider, ListModelsError> {
    let env_name = resolve_env_name(
        &config.providers.openrouter.api_key_env,
        "OPENROUTER_API_KEY",
    );
    let api_key = std::env::var(&env_name).map_err(|_| ListModelsError::MissingCredentials)?;
    let client = build_http_client(config.providers.openrouter.proxy.as_ref())
        .map_err(|e| ListModelsError::Config(e.to_string()))?;
    if let Some(ref referer) = config.providers.openrouter.referer {
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
        let temp = opi_ai::openrouter::openrouter_provider(
            String::new(),
            config.providers.openrouter.base_url.clone(),
        );
        let models = temp.models().to_vec();
        Ok(opi_ai::openai_chat::OpenAiChatProvider::new_for_profile(
            api_key,
            base_url,
            "openrouter".into(),
            compat,
            extra_headers,
            models,
        )
        .with_shared_client(client))
    } else {
        Ok(opi_ai::openrouter::openrouter_provider(
            api_key,
            config.providers.openrouter.base_url.clone(),
        )
        .with_shared_client(client))
    }
}

fn build_mistral(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::openai_chat::OpenAiChatProvider, ListModelsError> {
    let env_name = resolve_env_name(&config.providers.mistral.api_key_env, "MISTRAL_API_KEY");
    let api_key = std::env::var(&env_name).map_err(|_| ListModelsError::MissingCredentials)?;
    let client = build_http_client(config.providers.mistral.proxy.as_ref())
        .map_err(|e| ListModelsError::Config(e.to_string()))?;
    Ok(
        opi_ai::mistral::mistral_provider(api_key, config.providers.mistral.base_url.clone())
            .with_shared_client(client),
    )
}

fn build_openai_responses(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::openai_responses::OpenAiResponsesProvider, ListModelsError> {
    let env_name = resolve_env_name(
        &config.providers.openai_responses.api_key_env,
        "OPENAI_API_KEY",
    );
    let api_key = std::env::var(&env_name).map_err(|_| ListModelsError::MissingCredentials)?;
    let client = build_http_client(config.providers.openai_responses.proxy.as_ref())
        .map_err(|e| ListModelsError::Config(e.to_string()))?;
    Ok(
        opi_ai::openai_responses::OpenAiResponsesProvider::with_client(
            api_key,
            config.providers.openai_responses.base_url.clone(),
            client,
        ),
    )
}

fn build_gemini(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::gemini::GeminiProvider, ListModelsError> {
    let env_name = resolve_env_name(&config.providers.gemini.api_key_env, "GEMINI_API_KEY");
    let api_key = std::env::var(&env_name).map_err(|_| ListModelsError::MissingCredentials)?;
    let client = build_http_client(config.providers.gemini.proxy.as_ref())
        .map_err(|e| ListModelsError::Config(e.to_string()))?;
    Ok(opi_ai::gemini::GeminiProvider::with_client(
        api_key,
        config.providers.gemini.base_url.clone(),
        client,
    ))
}

fn build_bedrock(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::bedrock::BedrockProvider, ListModelsError> {
    let bedrock_config = &config.providers.bedrock;
    let (akid, sak, token, env_region) = resolve_bedrock_env_credentials();
    let env_profile = std::env::var("AWS_PROFILE").ok();
    let profile_name = bedrock_config.profile.as_deref().or(env_profile.as_deref());
    let credentials_file = aws_credentials_path();
    let config_file = aws_config_path();
    let secret_key = bedrock_config
        .secret_access_key_env
        .as_deref()
        .and_then(|env_name| std::env::var(env_name).ok());
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
        config_file_path: config_file.as_deref(),
    };
    let resolved = opi_ai::bedrock::credentials::resolve_credentials(&input);
    let (bedrock_creds, _) = resolved.ok_or(ListModelsError::MissingCredentials)?;
    let client = build_http_client(bedrock_config.proxy.as_ref())
        .map_err(|e| ListModelsError::Config(e.to_string()))?;
    Ok(opi_ai::bedrock::BedrockProvider::from_credentials(
        bedrock_creds,
        bedrock_config.base_url.clone(),
        client,
    ))
}

fn build_azure(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::azure_openai::AzureOpenAIProvider, ListModelsError> {
    let azure_config = &config.providers.azure;
    let env_name = resolve_env_name(&azure_config.api_key_env, "AZURE_OPENAI_API_KEY");
    let api_key = std::env::var(&env_name).map_err(|_| ListModelsError::MissingCredentials)?;
    if azure_config.deployments.is_empty() {
        return Err(ListModelsError::Config(
            "azure provider has no deployments configured".into(),
        ));
    }
    let provider = opi_ai::azure_openai::AzureOpenAIProvider::from_config(
        api_key,
        azure_config.endpoint.clone(),
        azure_config.deployments.clone(),
        azure_config.api_version.clone(),
    )
    .map_err(|e| ListModelsError::Config(e.to_string()))?;
    Ok(provider.with_client(
        build_http_client(azure_config.proxy.as_ref())
            .map_err(|e| ListModelsError::Config(e.to_string()))?,
    ))
}

fn build_vertex(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<opi_ai::vertex::VertexProvider, ListModelsError> {
    let vertex_config = &config.providers.vertex;
    let env_name = resolve_env_name(&vertex_config.access_token_env, "VERTEX_ACCESS_TOKEN");
    let access_token = std::env::var(&env_name).map_err(|_| ListModelsError::MissingCredentials)?;
    let project = vertex_config
        .project
        .as_deref()
        .ok_or_else(|| ListModelsError::Config("vertex provider requires project".into()))?;
    let location = vertex_config
        .location
        .as_deref()
        .ok_or_else(|| ListModelsError::Config("vertex provider requires location".into()))?;
    let provider = if vertex_config.models.is_empty() {
        opi_ai::vertex::VertexProvider::new(
            access_token,
            project.into(),
            location.into(),
            vertex_config.base_url.clone(),
        )
    } else {
        opi_ai::vertex::VertexProvider::from_config(
            access_token,
            project.into(),
            location.into(),
            vertex_config.models.clone(),
            vertex_config.base_url.clone(),
        )
    };
    Ok(provider.with_client(
        build_http_client(vertex_config.proxy.as_ref())
            .map_err(|e| ListModelsError::Config(e.to_string()))?,
    ))
}

fn block_on_async<F: std::future::Future>(f: F) -> F::Output {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(f))
    } else {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(f)
    }
}
