//! TOML config loading (S9.1/S9.1.1).
//!
//! Loads and resolves opi configuration with precedence:
//! CLI > env > project config > user config > built-in defaults.
//!
//! Phase 1 fields: model, max_iterations, tool_timeout_ms, theme,
//! thinking, providers.anthropic.api_key_env.

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
}

/// `[defaults]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct DefaultsConfig {
    pub model: String,
    pub max_iterations: u32,
    pub tool_timeout_ms: u64,
    pub theme: String,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            model: "anthropic:claude-sonnet-4".into(),
            max_iterations: 50,
            tool_timeout_ms: 30_000,
            theme: "default".into(),
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
}

/// `[providers.anthropic]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct AnthropicProviderConfig {
    pub api_key_env: String,
}

impl Default for AnthropicProviderConfig {
    fn default() -> Self {
        Self {
            api_key_env: "ANTHROPIC_API_KEY".into(),
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
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlDefaults {
    model: Option<String>,
    max_iterations: Option<u32>,
    tool_timeout_ms: Option<u64>,
    theme: Option<String>,
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
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TomlAnthropic {
    api_key_env: Option<String>,
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
        if let Some(v) = self.thinking.enabled {
            config.thinking.enabled = v;
        }
        if let Some(v) = self.thinking.budget_tokens {
            config.thinking.budget_tokens = v;
        }
        if let Some(v) = self.providers.anthropic.api_key_env {
            config.providers.anthropic.api_key_env = v;
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

    if let Some(env_model) = &source.env_model {
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
