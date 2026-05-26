//! TOML config loading (S9.1/S9.1.1).
//!
//! Loads and resolves opi configuration with precedence:
//! CLI > env > project config > user config > built-in defaults.
//!
//! Phase 1 fields: model, max_iterations, tool_timeout_ms, theme,
//! thinking, providers.anthropic.api_key_env.
//!
//! Phase 2 fields: providers.{openai,openrouter,mistral,openai_responses,gemini}
//! config with api_key_env, base_url, and OpenRouter-specific referer.

use std::path::{Path, PathBuf};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Resolved config (public API — all fields present)
// ---------------------------------------------------------------------------

/// Top-level opi configuration (fully resolved).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OpiConfig {
    pub defaults: DefaultsConfig,
    pub thinking: ThinkingConfig,
    pub providers: ProvidersConfig,
    pub keybindings: KeybindingsConfig,
    pub retry: opi_ai::retry::RetryConfig,
    pub compaction: CompactionConfigSection,
}

/// `[defaults]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct DefaultsConfig {
    pub model: String,
    pub max_iterations: u32,
    pub tool_timeout_ms: u64,
    pub theme: String,
    pub allow_mutating_tools: bool,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            model: "anthropic:claude-sonnet-4".into(),
            max_iterations: 50,
            tool_timeout_ms: 30_000,
            theme: "default".into(),
            allow_mutating_tools: false,
        }
    }
}

/// `[thinking]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: u32,
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            budget_tokens: 10_000,
        }
    }
}

/// `[providers]` section.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProvidersConfig {
    pub anthropic: AnthropicProviderConfig,
    pub openai: GenericProviderConfig,
    pub openrouter: OpenRouterProviderConfig,
    pub mistral: GenericProviderConfig,
    pub openai_responses: GenericProviderConfig,
    pub gemini: GenericProviderConfig,
}

/// `[providers.anthropic]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct AnthropicProviderConfig {
    pub api_key_env: String,
    pub base_url: Option<String>,
    pub proxy: Option<ProviderProxyConfig>,
}

impl Default for AnthropicProviderConfig {
    fn default() -> Self {
        Self {
            api_key_env: "ANTHROPIC_API_KEY".into(),
            base_url: None,
            proxy: None,
        }
    }
}

/// Generic provider config (api_key_env + optional base_url + optional proxy).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GenericProviderConfig {
    pub api_key_env: String,
    pub base_url: Option<String>,
    pub proxy: Option<ProviderProxyConfig>,
}

/// OpenRouter-specific provider config.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OpenRouterProviderConfig {
    pub api_key_env: String,
    pub base_url: Option<String>,
    pub referer: Option<String>,
    pub proxy: Option<ProviderProxyConfig>,
}

/// Per-provider proxy configuration from `[providers.*.proxy]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderProxyConfig {
    pub url: String,
    pub no_proxy: Option<String>,
}

/// `[keybindings]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingsConfig {
    pub submit: String,
    pub abort: String,
    pub new_line: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            submit: "enter".into(),
            abort: "escape".into(),
            new_line: "alt+enter".into(),
        }
    }
}

/// `[compaction]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactionConfigSection {
    pub enabled: bool,
    pub threshold_tokens: u64,
}

impl Default for CompactionConfigSection {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_tokens: 100_000,
        }
    }
}

// ---------------------------------------------------------------------------
// TOML deserialization structs (Option fields detect presence)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlConfig {
    defaults: TomlDefaults,
    thinking: TomlThinking,
    providers: TomlProviders,
    keybindings: TomlKeybindings,
    retry: TomlRetry,
    compaction: TomlCompaction,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlDefaults {
    model: Option<String>,
    max_iterations: Option<u32>,
    tool_timeout_ms: Option<u64>,
    theme: Option<String>,
    allow_mutating_tools: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlThinking {
    enabled: Option<bool>,
    budget_tokens: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlProviders {
    anthropic: TomlAnthropic,
    openai: TomlGenericProvider,
    openrouter: TomlOpenRouterProvider,
    mistral: TomlGenericProvider,
    openai_responses: TomlGenericProvider,
    gemini: TomlGenericProvider,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlAnthropic {
    api_key_env: Option<String>,
    base_url: Option<String>,
    proxy: Option<TomlProxy>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlGenericProvider {
    api_key_env: Option<String>,
    base_url: Option<String>,
    proxy: Option<TomlProxy>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlOpenRouterProvider {
    api_key_env: Option<String>,
    base_url: Option<String>,
    referer: Option<String>,
    proxy: Option<TomlProxy>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlProxy {
    url: Option<String>,
    no_proxy: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlKeybindings {
    submit: Option<String>,
    abort: Option<String>,
    new_line: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlRetry {
    max_attempts: Option<u32>,
    initial_delay_ms: Option<u64>,
    max_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlCompaction {
    enabled: Option<bool>,
    threshold_tokens: Option<u64>,
}

impl TomlConfig {
    fn merge_into(self, config: &mut OpiConfig) {
        if let Some(v) = self.defaults.model {
            config.defaults.model = v;
        }
        if let Some(v) = self.defaults.max_iterations {
            config.defaults.max_iterations = v;
        }
        if let Some(v) = self.defaults.tool_timeout_ms {
            config.defaults.tool_timeout_ms = v;
        }
        if let Some(v) = self.defaults.theme {
            config.defaults.theme = v;
        }
        if let Some(v) = self.defaults.allow_mutating_tools {
            config.defaults.allow_mutating_tools = v;
        }
        if let Some(v) = self.thinking.enabled {
            config.thinking.enabled = v;
        }
        if let Some(v) = self.thinking.budget_tokens {
            config.thinking.budget_tokens = v;
        }
        if let Some(v) = self.providers.anthropic.api_key_env {
            config.providers.anthropic.api_key_env = v;
        }
        if let Some(v) = self.providers.anthropic.base_url {
            config.providers.anthropic.base_url = Some(v);
        }
        if let Some(p) = self.providers.anthropic.proxy
            && let Some(url) = p.url.filter(|s| !s.trim().is_empty())
        {
            config.providers.anthropic.proxy = Some(ProviderProxyConfig {
                url,
                no_proxy: p.no_proxy,
            });
        }
        if let Some(v) = self.providers.openai.api_key_env {
            config.providers.openai.api_key_env = v;
        }
        if let Some(v) = self.providers.openai.base_url {
            config.providers.openai.base_url = Some(v);
        }
        if let Some(p) = self.providers.openai.proxy
            && let Some(url) = p.url.filter(|s| !s.trim().is_empty())
        {
            config.providers.openai.proxy = Some(ProviderProxyConfig {
                url,
                no_proxy: p.no_proxy,
            });
        }
        if let Some(v) = self.providers.openrouter.api_key_env {
            config.providers.openrouter.api_key_env = v;
        }
        if let Some(v) = self.providers.openrouter.base_url {
            config.providers.openrouter.base_url = Some(v);
        }
        if let Some(v) = self.providers.openrouter.referer {
            config.providers.openrouter.referer = Some(v);
        }
        if let Some(p) = self.providers.openrouter.proxy
            && let Some(url) = p.url.filter(|s| !s.trim().is_empty())
        {
            config.providers.openrouter.proxy = Some(ProviderProxyConfig {
                url,
                no_proxy: p.no_proxy,
            });
        }
        if let Some(v) = self.providers.mistral.api_key_env {
            config.providers.mistral.api_key_env = v;
        }
        if let Some(v) = self.providers.mistral.base_url {
            config.providers.mistral.base_url = Some(v);
        }
        if let Some(p) = self.providers.mistral.proxy
            && let Some(url) = p.url.filter(|s| !s.trim().is_empty())
        {
            config.providers.mistral.proxy = Some(ProviderProxyConfig {
                url,
                no_proxy: p.no_proxy,
            });
        }
        if let Some(v) = self.providers.openai_responses.api_key_env {
            config.providers.openai_responses.api_key_env = v;
        }
        if let Some(v) = self.providers.openai_responses.base_url {
            config.providers.openai_responses.base_url = Some(v);
        }
        if let Some(p) = self.providers.openai_responses.proxy
            && let Some(url) = p.url.filter(|s| !s.trim().is_empty())
        {
            config.providers.openai_responses.proxy = Some(ProviderProxyConfig {
                url,
                no_proxy: p.no_proxy,
            });
        }
        if let Some(v) = self.providers.gemini.api_key_env {
            config.providers.gemini.api_key_env = v;
        }
        if let Some(v) = self.providers.gemini.base_url {
            config.providers.gemini.base_url = Some(v);
        }
        if let Some(p) = self.providers.gemini.proxy
            && let Some(url) = p.url.filter(|s| !s.trim().is_empty())
        {
            config.providers.gemini.proxy = Some(ProviderProxyConfig {
                url,
                no_proxy: p.no_proxy,
            });
        }
        if let Some(v) = self.keybindings.submit {
            config.keybindings.submit = v;
        }
        if let Some(v) = self.keybindings.abort {
            config.keybindings.abort = v;
        }
        if let Some(v) = self.keybindings.new_line {
            config.keybindings.new_line = v;
        }
        if let Some(v) = self.retry.max_attempts {
            config.retry.max_attempts = v;
        }
        if let Some(v) = self.retry.initial_delay_ms {
            config.retry.initial_delay_ms = v;
        }
        if let Some(v) = self.retry.max_delay_ms {
            config.retry.max_delay_ms = v;
        }
        if let Some(v) = self.compaction.enabled {
            config.compaction.enabled = v;
        }
        if let Some(v) = self.compaction.threshold_tokens {
            config.compaction.threshold_tokens = v;
        }
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from config loading and parsing.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Load and parse a TOML config file. Returns defaults if the file doesn't
/// exist. Returns a clear error for malformed TOML.
pub fn load_config_file(path: &Path) -> Result<OpiConfig, ConfigError> {
    if !path.exists() {
        return Ok(OpiConfig::default());
    }
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_toml(&contents, path)
}

fn parse_toml(contents: &str, path: &Path) -> Result<OpiConfig, ConfigError> {
    let raw: TomlConfig = toml::from_str(contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })?;
    let mut config = OpiConfig::default();
    raw.merge_into(&mut config);
    Ok(config)
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// External configuration sources for precedence resolution.
pub struct ConfigSource {
    /// Model from CLI `--model` flag.
    pub cli_model: Option<String>,
    /// Explicit config path from CLI `--config` flag.
    pub config_path: Option<PathBuf>,
    /// Model from env var `OPI_MODEL`.
    pub env_model: Option<String>,
    /// Project root directory (for `.opi/config.toml`).
    pub project_dir: Option<PathBuf>,
    /// User config file path override (for testing). When `None`, uses
    /// the platform-default path from `user_config_path()`.
    pub user_config_path: Option<PathBuf>,
}

/// Resolve configuration from all sources with correct precedence:
/// CLI > env > project config > user config > built-in defaults.
pub fn resolve_config(source: ConfigSource) -> Result<OpiConfig, ConfigError> {
    let user_path = source.user_config_path.unwrap_or_else(user_config_path);
    let mut config = load_config_file(&user_path)?;

    if let Some(project_dir) = &source.project_dir {
        let project_config_path = project_dir.join(".opi").join("config.toml");
        let project_raw = load_raw_config(&project_config_path)?;
        project_raw.merge_into(&mut config);
    }

    // --config file overrides project and user config
    if let Some(config_path) = &source.config_path {
        if !config_path.exists() {
            return Err(ConfigError::Read {
                path: config_path.clone(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "config file not found"),
            });
        }
        let cli_raw = load_raw_config(config_path)?;
        cli_raw.merge_into(&mut config);
    }

    // Env model only applies when --config was NOT explicitly provided,
    // so that an explicit config file's model takes precedence over env.
    if source.config_path.is_none()
        && let Some(env_model) = &source.env_model
    {
        config.defaults.model = env_model.clone();
    }

    if let Some(cli_model) = &source.cli_model {
        config.defaults.model = cli_model.clone();
    }

    Ok(config)
}

fn load_raw_config(path: &Path) -> Result<TomlConfig, ConfigError> {
    if !path.exists() {
        return Ok(TomlConfig::default());
    }
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

/// Return the platform-specific user config path.
pub fn user_config_path() -> PathBuf {
    if cfg!(windows) {
        // Windows: %APPDATA%\opi\config.toml
        std::env::var("APPDATA")
            .map(|p| PathBuf::from(p).join("opi").join("config.toml"))
            .unwrap_or_else(|_| PathBuf::from(".opi").join("config.toml"))
    } else {
        // Unix: ~/.config/opi/config.toml
        dirs_home()
            .map(|h| h.join(".config").join("opi").join("config.toml"))
            .unwrap_or_else(|| PathBuf::from(".opi").join("config.toml"))
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}
