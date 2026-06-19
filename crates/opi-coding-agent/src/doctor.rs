//! Top-level `opi doctor` command (Phase 7 task 7.4).
//!
//! `opi doctor` is a local, network-free health summary, distinct from
//! `opi package doctor`. It reports shared [`Diagnostic`] values for a set of
//! scopes: `config`, `provider`, `package`, `session`, `tui`, and `rpc`. It
//! makes **no paid model calls and no network checks by default** — provider
//! scope inspects credential *presence* only, never the credential value, and
//! nothing here constructs a provider or opens a connection. Network checks are
//! an explicit later design and intentionally absent.
//!
//! The command surface is:
//!
//! ```text
//! opi doctor
//! opi doctor --json
//! opi doctor --scope config,provider,package,session,tui,rpc
//! ```
//!
//! Exit-code policy (see the Phase 7 design):
//!
//! | result                 | exit code |
//! |------------------------|----------:|
//! | no errors / warnings   |         0 |
//! | one or more errors     |         2 |
//! | doctor failed internally |       1 |
//!
//! Exit code 1 (internal failure) is produced by the CLI layer for an
//! unparseable `--scope` list; scope checks themselves are best-effort and turn
//! any collection error into an error-severity diagnostic (exit 2), never a
//! crash. [`DoctorReport::exit_code`] covers the 0/2 policy; the binary wrapper
//! adds 1 for argument failures.
//!
//! This module is pure and synchronous so it can be unit-tested without a
//! runtime; all environment and filesystem access is threaded through
//! [`DoctorContext`] by the binary.

use std::path::Path;

use serde::Serialize;

use opi_agent::diagnostic::{
    SOURCE_CONFIG, SOURCE_PACKAGE, SOURCE_PROVIDER, SOURCE_RPC, SOURCE_SESSION, SOURCE_TUI,
};
use opi_agent::{Diagnostic, RedactionMode, Severity};

use crate::config::{ConfigError, OpiConfig};
use crate::diagnostic_bridge::{diagnostic_from_config, diagnostic_from_package};
use crate::package_resolver::resolve_installed_packages;
use crate::rpc::RPC_SCHEMA_VERSION;

/// One of the six doctor scopes.
///
/// `#[non_exhaustive]` is deliberately NOT used: the design fixes exactly six
/// scopes for Phase 7, and the `--scope` flag values are part of the user-facing
/// surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorScope {
    Config,
    Provider,
    Package,
    Session,
    Tui,
    Rpc,
}

impl DoctorScope {
    /// All six scopes in canonical order.
    pub const ALL: &'static [DoctorScope] = &[
        DoctorScope::Config,
        DoctorScope::Provider,
        DoctorScope::Package,
        DoctorScope::Session,
        DoctorScope::Tui,
        DoctorScope::Rpc,
    ];

    /// Stable lowercase scope name used on the wire and in `--scope`.
    pub fn as_str(&self) -> &'static str {
        match self {
            DoctorScope::Config => "config",
            DoctorScope::Provider => "provider",
            DoctorScope::Package => "package",
            DoctorScope::Session => "session",
            DoctorScope::Tui => "tui",
            DoctorScope::Rpc => "rpc",
        }
    }

    /// Parse a comma-separated scope list (e.g. `"config,tui"`).
    ///
    /// Blank input parses to an empty selection, which the caller interprets as
    /// "all scopes". An unknown token is an error so a typo cannot silently
    /// narrow the check.
    pub fn parse_list(input: &str) -> Result<Vec<DoctorScope>, String> {
        let mut out = Vec::new();
        for raw in input.split(',') {
            let token = raw.trim();
            if token.is_empty() {
                continue;
            }
            let scope = match token {
                "config" => DoctorScope::Config,
                "provider" => DoctorScope::Provider,
                "package" => DoctorScope::Package,
                "session" => DoctorScope::Session,
                "tui" => DoctorScope::Tui,
                "rpc" => DoctorScope::Rpc,
                other => {
                    return Err(format!(
                        "unknown doctor scope {other:?}; valid scopes are config, provider, package, session, tui, rpc"
                    ));
                }
            };
            out.push(scope);
        }
        Ok(out)
    }
}

/// Inputs for a doctor run, all local. Doctor makes no network calls; the
/// `env_var` probe is used only for credential *presence* checks.
pub struct DoctorContext<'a> {
    /// Resolved configuration (defaults if config resolution failed).
    pub config: &'a OpiConfig,
    /// Config resolution error, surfaced as a config-scope error diagnostic.
    pub config_error: Option<&'a ConfigError>,
    /// Workspace root for package resolution.
    pub workspace_root: &'a Path,
    /// User config directory for global package resolution.
    pub user_config_dir: &'a Path,
    /// Session storage directory.
    pub sessions_dir: &'a Path,
    /// `TERM` env value.
    pub term: Option<&'a str>,
    /// `TERM_PROGRAM` env value.
    pub term_program: Option<&'a str>,
    /// `TERM_FEATURES` env value.
    pub term_features: Option<&'a str>,
    /// Whether `NO_COLOR` is set.
    pub no_color: bool,
    /// `COLORTERM` env value.
    pub colorterm: Option<&'a str>,
    /// Probe for an environment variable's presence (credential checks).
    pub env_var: &'a dyn Fn(&str) -> Option<String>,
}

/// A single doctor diagnostic tagged with the scope that produced it.
///
/// Serializes (via `flatten`) as `{ scope, severity, code, source, message,
/// details?, action? }` so `--json` output is one flat JSON object per line.
#[derive(Debug, Clone, Serialize)]
pub struct DoctorEntry {
    pub scope: DoctorScope,
    #[serde(flatten)]
    pub diagnostic: Diagnostic,
}

/// The collected diagnostics from a doctor run.
#[derive(Debug, Clone, Default)]
pub struct DoctorReport {
    /// One entry per emitted diagnostic, in canonical scope order.
    pub entries: Vec<DoctorEntry>,
}

impl DoctorReport {
    /// Whether any entry is error-severity.
    pub fn has_errors(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.diagnostic.severity == Severity::Error)
    }

    /// Doctor exit code for the 0/2 policy (the binary adds 1 for argument
    /// failures). Warnings and info are not failures.
    pub fn exit_code(&self) -> i32 {
        if self.has_errors() { 2 } else { 0 }
    }
}

// Stable, doctor-local diagnostic codes. These describe doctor observations
// rather than runtime failures; they live here (not in the shared opi-agent
// code module) because doctor is an opi-coding-agent command.
const CODE_DOCTOR_CONFIG_MODEL: &str = "doctor_config_model";
const CODE_DOCTOR_CONFIG_PROXY: &str = "doctor_config_proxy";
const CODE_DOCTOR_PROVIDER_CREDENTIALS: &str = "doctor_provider_credentials";
const CODE_DOCTOR_PROVIDER_ENDPOINT: &str = "doctor_provider_endpoint";
const CODE_DOCTOR_PROVIDER_UNKNOWN: &str = "doctor_provider_unknown";
const CODE_DOCTOR_PACKAGE_SUMMARY: &str = "doctor_package_summary";
const CODE_DOCTOR_PACKAGE_RESOLVE: &str = "doctor_package_resolve_failed";
const CODE_DOCTOR_SESSION_DIR: &str = "doctor_session_dir";
const CODE_DOCTOR_TUI_CAPABILITY: &str = "doctor_tui_capability";
const CODE_DOCTOR_RPC_SCHEMA: &str = "doctor_rpc_schema";

/// Run the requested scopes. An empty `scopes` slice means all scopes.
///
/// Output is emitted in canonical [`DoctorScope::ALL`] order; duplicates and
/// out-of-order tokens in a user `--scope` list are normalized.
pub fn run_doctor(scopes: &[DoctorScope], ctx: &DoctorContext) -> DoctorReport {
    let requested: Vec<DoctorScope> = if scopes.is_empty() {
        DoctorScope::ALL.to_vec()
    } else {
        scopes.to_vec()
    };

    let mut entries = Vec::new();
    for &scope in DoctorScope::ALL {
        if !requested.contains(&scope) {
            continue;
        }
        let diagnostics = match scope {
            DoctorScope::Config => config_diagnostics(ctx.config, ctx.config_error),
            DoctorScope::Provider => provider_diagnostics(ctx.config, ctx.env_var),
            DoctorScope::Package => package_diagnostics(ctx.workspace_root, ctx.user_config_dir),
            DoctorScope::Session => session_diagnostics(ctx.sessions_dir),
            DoctorScope::Tui => tui_diagnostics(
                ctx.term,
                ctx.term_program,
                ctx.term_features,
                ctx.no_color,
                ctx.colorterm,
            ),
            DoctorScope::Rpc => rpc_diagnostics(),
        };
        for diagnostic in diagnostics {
            entries.push(DoctorEntry { scope, diagnostic });
        }
    }
    DoctorReport { entries }
}

/// Format the report as human-readable text (one line per diagnostic).
pub fn format_text(report: &DoctorReport) -> String {
    let mut out = String::new();
    for entry in &report.entries {
        let d = &entry.diagnostic;
        out.push_str(&format!(
            "[{}] {}: {}::{}: {}\n",
            d.severity,
            entry.scope.as_str(),
            d.source,
            d.code,
            d.message,
        ));
        if let Some(action) = &d.action {
            out.push_str(&format!("    action: {action}\n"));
        }
    }
    if out.is_empty() {
        out.push_str("no diagnostics\n");
    }
    out
}

/// Format the report as NDJSON: one JSON object per diagnostic.
///
/// `details` is run through the shared Summary redaction at this public
/// boundary so absolute paths and content-sensitive values never ship
/// unredacted (Phase 7 design: "details = redacted structured metadata"). The
/// cross-surface redaction guard tests are owned by task 7.6.
pub fn format_json(report: &DoctorReport) -> String {
    report
        .entries
        .iter()
        .filter_map(|entry| {
            let mut entry = entry.clone();
            let redacted = entry.diagnostic.redacted_details(RedactionMode::Summary);
            entry.diagnostic.details = redacted;
            serde_json::to_string(&entry).ok()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Scope checks
// ---------------------------------------------------------------------------

fn config_diagnostics(config: &OpiConfig, config_error: Option<&ConfigError>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if let Some(err) = config_error {
        out.push(diagnostic_from_config(err));
    }

    match config.defaults.model.split_once(':') {
        Some((provider, model)) => out.push(Diagnostic::new(
            Severity::Info,
            CODE_DOCTOR_CONFIG_MODEL,
            SOURCE_CONFIG,
            format!("selected model resolves to provider {provider:?}, model {model:?}"),
        )),
        None => out.push(
            Diagnostic::new(
                Severity::Warning,
                CODE_DOCTOR_CONFIG_MODEL,
                SOURCE_CONFIG,
                format!(
                    "selected model spec {:?} is not in provider:model form",
                    config.defaults.model
                ),
            )
            .action("set [defaults] model to a provider:model spec"),
        ),
    }

    if let Some((provider, _)) = config.defaults.model.split_once(':') {
        let message = match provider_proxy_url(config, provider) {
            Some(_) => format!("proxy configured for selected provider {provider:?}"),
            None => format!("no explicit proxy configured for selected provider {provider:?}"),
        };
        out.push(Diagnostic::new(
            Severity::Info,
            CODE_DOCTOR_CONFIG_PROXY,
            SOURCE_CONFIG,
            message,
        ));
    }

    out
}

fn provider_diagnostics(
    config: &OpiConfig,
    env_var: &dyn Fn(&str) -> Option<String>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    let Some((provider, model)) = config.defaults.model.split_once(':') else {
        out.push(Diagnostic::new(
            Severity::Warning,
            CODE_DOCTOR_PROVIDER_UNKNOWN,
            SOURCE_PROVIDER,
            format!(
                "selected model spec {:?} is not in provider:model form; skipping provider checks",
                config.defaults.model
            ),
        ));
        return out;
    };

    match provider_credential_probe(config, provider, env_var) {
        Some(probe) => {
            let message = if probe.present {
                format!(
                    "provider {:?} credentials present ({})",
                    provider, probe.label
                )
            } else {
                format!(
                    "provider {:?} credentials not set ({})",
                    provider, probe.label
                )
            };
            let severity = if probe.present {
                Severity::Info
            } else {
                Severity::Warning
            };
            out.push(
                Diagnostic::new(
                    severity,
                    CODE_DOCTOR_PROVIDER_CREDENTIALS,
                    SOURCE_PROVIDER,
                    message,
                )
                .details(serde_json::json!({
                    "provider": provider,
                    "credentials_present": probe.present,
                    "credential_probe": probe.label,
                })),
            );
        }
        None => out.push(Diagnostic::new(
            Severity::Warning,
            CODE_DOCTOR_PROVIDER_UNKNOWN,
            SOURCE_PROVIDER,
            format!("selected provider {provider:?} is not a known built-in or configured profile"),
        )),
    }

    // Model identity / capability metadata (network-free: identity only).
    out.push(Diagnostic::new(
        Severity::Info,
        CODE_DOCTOR_PROVIDER_ENDPOINT,
        SOURCE_PROVIDER,
        format!("selected provider {provider:?}, model {model:?}"),
    ));

    // Required-field shape warnings for providers that need extra config.
    match provider {
        "azure" => {
            let azure = &config.providers.azure;
            if azure.endpoint.is_none() {
                out.push(Diagnostic::new(
                    Severity::Warning,
                    CODE_DOCTOR_PROVIDER_ENDPOINT,
                    SOURCE_PROVIDER,
                    "azure provider has no endpoint configured",
                ));
            }
            if azure.deployments.is_empty() {
                out.push(Diagnostic::new(
                    Severity::Warning,
                    CODE_DOCTOR_PROVIDER_ENDPOINT,
                    SOURCE_PROVIDER,
                    "azure provider has no deployments configured",
                ));
            }
        }
        "vertex" => {
            let vertex = &config.providers.vertex;
            if vertex.project.is_none() || vertex.location.is_none() {
                out.push(Diagnostic::new(
                    Severity::Warning,
                    CODE_DOCTOR_PROVIDER_ENDPOINT,
                    SOURCE_PROVIDER,
                    "vertex provider is missing project or location",
                ));
            }
        }
        _ => {}
    }

    out
}

fn package_diagnostics(workspace_root: &Path, user_config_dir: &Path) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    match resolve_installed_packages(workspace_root, user_config_dir) {
        Ok(resolution) => {
            for pd in &resolution.diagnostics {
                out.push(diagnostic_from_package(pd));
            }
            // Always emit a summary so the scope appears in output even when
            // there are zero packages and zero findings.
            out.push(Diagnostic::new(
                Severity::Info,
                CODE_DOCTOR_PACKAGE_SUMMARY,
                SOURCE_PACKAGE,
                format!(
                    "{} installed package(s) checked; {} diagnostic(s)",
                    resolution.packages.len(),
                    resolution.diagnostics.len()
                ),
            ));
        }
        Err(err) => out.push(Diagnostic::new(
            Severity::Error,
            CODE_DOCTOR_PACKAGE_RESOLVE,
            SOURCE_PACKAGE,
            format!("installed package resolution failed: {err}"),
        )),
    }
    out
}

fn session_diagnostics(sessions_dir: &Path) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    match std::fs::read_dir(sessions_dir) {
        Ok(read_dir) => {
            let count = read_dir
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
                .count();
            out.push(
                Diagnostic::new(
                    Severity::Info,
                    CODE_DOCTOR_SESSION_DIR,
                    SOURCE_SESSION,
                    format!("sessions directory accessible ({count} session file(s))"),
                )
                .details(serde_json::json!({
                    "session_count": count,
                })),
            );
            out.push(Diagnostic::new(
                Severity::Info,
                CODE_DOCTOR_SESSION_DIR,
                SOURCE_SESSION,
                "corrupt-line recovery available on session load",
            ));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // A not-yet-created sessions dir under an existing parent is normal
            // on a fresh install and must not be an error.
            let parent_exists = sessions_dir.parent().is_some_and(|p| p.exists());
            let (severity, message) = if parent_exists {
                (
                    Severity::Info,
                    "sessions directory not yet created (created on first run)".to_string(),
                )
            } else {
                (
                    Severity::Warning,
                    "sessions directory does not exist and its parent is missing".to_string(),
                )
            };
            out.push(Diagnostic::new(
                severity,
                CODE_DOCTOR_SESSION_DIR,
                SOURCE_SESSION,
                message,
            ));
        }
        Err(err) => out.push(Diagnostic::new(
            Severity::Error,
            CODE_DOCTOR_SESSION_DIR,
            SOURCE_SESSION,
            format!("sessions directory not accessible: {err}"),
        )),
    }
    out
}

fn tui_diagnostics(
    term: Option<&str>,
    term_program: Option<&str>,
    term_features: Option<&str>,
    no_color: bool,
    colorterm: Option<&str>,
) -> Vec<Diagnostic> {
    use opi_tui::{CapabilitySource, TerminalGraphicsProtocol, detect_graphics_protocol};

    let protocol = detect_graphics_protocol(
        term,
        term_program,
        term_features,
        &CapabilitySource::EnvVars,
    );
    let protocol_name = match protocol {
        TerminalGraphicsProtocol::Kitty => "kitty",
        TerminalGraphicsProtocol::Iterm2 => "iterm2",
        TerminalGraphicsProtocol::Sixel => "sixel",
        TerminalGraphicsProtocol::Fallback => "text-fallback",
    };
    let color = if no_color {
        "no color (NO_COLOR set)".to_string()
    } else {
        match colorterm {
            Some(value)
                if value.eq_ignore_ascii_case("truecolor")
                    || value.eq_ignore_ascii_case("24bit") =>
            {
                "truecolor".to_string()
            }
            _ => "enabled (terminal default)".to_string(),
        }
    };

    vec![
        Diagnostic::new(
            Severity::Info,
            CODE_DOCTOR_TUI_CAPABILITY,
            SOURCE_TUI,
            format!("terminal graphics protocol: {protocol_name}; color: {color}"),
        )
        .details(serde_json::json!({
            "graphics_protocol": protocol_name,
            "color": color,
            "term": term,
            "term_program": term_program,
        })),
    ]
}

fn rpc_diagnostics() -> Vec<Diagnostic> {
    vec![
        Diagnostic::new(
            Severity::Info,
            CODE_DOCTOR_RPC_SCHEMA,
            SOURCE_RPC,
            format!("SDK/RPC schema version: {RPC_SCHEMA_VERSION}"),
        ),
        Diagnostic::new(
            Severity::Info,
            CODE_DOCTOR_RPC_SCHEMA,
            SOURCE_RPC,
            "startup diagnostics emitted in the rpc_ready header and session_info responses",
        ),
    ]
}

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

fn env_or_default(configured: &str, default: &str) -> String {
    if configured.trim().is_empty() {
        default.to_string()
    } else {
        configured.to_string()
    }
}

struct CredentialProbe {
    label: String,
    present: bool,
}

fn provider_credential_probe(
    config: &OpiConfig,
    provider: &str,
    env_var: &dyn Fn(&str) -> Option<String>,
) -> Option<CredentialProbe> {
    if provider == "bedrock" {
        return Some(bedrock_credential_probe(config, env_var));
    }

    let env_name = provider_credential_env_name(config, provider)?;
    Some(CredentialProbe {
        present: env_value_present(env_var, &env_name),
        label: format!("env {env_name}"),
    })
}

fn bedrock_credential_probe(
    config: &OpiConfig,
    env_var: &dyn Fn(&str) -> Option<String>,
) -> CredentialProbe {
    let bedrock = &config.providers.bedrock;
    let config_access_present = bedrock
        .access_key_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let env_access_present = env_value_present(env_var, "AWS_ACCESS_KEY_ID");
    let secret_env = bedrock
        .secret_access_key_env
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("AWS_SECRET_ACCESS_KEY");
    let secret_present = env_value_present(env_var, secret_env);
    let access_label = if config_access_present {
        "config access_key_id"
    } else {
        "env AWS_ACCESS_KEY_ID"
    };

    CredentialProbe {
        present: (config_access_present || env_access_present) && secret_present,
        label: format!("{access_label} + env {secret_env}"),
    }
}

fn env_value_present(env_var: &dyn Fn(&str) -> Option<String>, name: &str) -> bool {
    env_var(name).is_some_and(|value| !value.trim().is_empty())
}

/// Resolve the credential environment-variable name for a selected provider, if
/// known. Returns `None` for an unknown provider that is not a configured
/// openai-compatible profile.
fn provider_credential_env_name(config: &OpiConfig, provider: &str) -> Option<String> {
    let providers = &config.providers;
    Some(match provider {
        "anthropic" => env_or_default(&providers.anthropic.api_key_env, "ANTHROPIC_API_KEY"),
        "openai" => env_or_default(&providers.openai.api_key_env, "OPENAI_API_KEY"),
        "openrouter" => env_or_default(&providers.openrouter.api_key_env, "OPENROUTER_API_KEY"),
        "mistral" => env_or_default(&providers.mistral.api_key_env, "MISTRAL_API_KEY"),
        "openai-responses" => {
            env_or_default(&providers.openai_responses.api_key_env, "OPENAI_API_KEY")
        }
        "gemini" => env_or_default(&providers.gemini.api_key_env, "GEMINI_API_KEY"),
        "azure" => env_or_default(&providers.azure.api_key_env, "AZURE_OPENAI_API_KEY"),
        "vertex" => env_or_default(&providers.vertex.access_token_env, "VERTEX_ACCESS_TOKEN"),
        "bedrock" => {
            // Bedrock credentials resolve from env, profile, or file; the env
            // access key id is the primary signal, so probe that.
            "AWS_ACCESS_KEY_ID".to_string()
        }
        other => {
            let profile = providers.openai_compatible.get(other)?;
            if profile.api_key_env.trim().is_empty() {
                format!("{}_API_KEY", other.replace('-', "_").to_ascii_uppercase())
            } else {
                profile.api_key_env.clone()
            }
        }
    })
}

/// The configured proxy URL for the selected provider, if any.
fn provider_proxy_url<'a>(config: &'a OpiConfig, provider: &str) -> Option<&'a str> {
    let providers = &config.providers;
    match provider {
        "anthropic" => providers.anthropic.proxy.as_ref().map(|p| p.url.as_str()),
        "openai" => providers.openai.proxy.as_ref().map(|p| p.url.as_str()),
        "openrouter" => providers.openrouter.proxy.as_ref().map(|p| p.url.as_str()),
        "mistral" => providers.mistral.proxy.as_ref().map(|p| p.url.as_str()),
        "openai-responses" => providers
            .openai_responses
            .proxy
            .as_ref()
            .map(|p| p.url.as_str()),
        "gemini" => providers.gemini.proxy.as_ref().map(|p| p.url.as_str()),
        "bedrock" => providers.bedrock.proxy.as_ref().map(|p| p.url.as_str()),
        "azure" => providers.azure.proxy.as_ref().map(|p| p.url.as_str()),
        "vertex" => providers.vertex.proxy.as_ref().map(|p| p.url.as_str()),
        other => providers
            .openai_compatible
            .get(other)
            .and_then(|profile| profile.proxy.as_ref().map(|p| p.url.as_str())),
    }
}
