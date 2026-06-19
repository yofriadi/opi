//! Shared diagnostic model and redaction core (Phase 7).
//!
//! This module defines the workspace-wide diagnostic vocabulary: a structured
//! [`Diagnostic`] record with a stable severity, snake_case code, subsystem
//! source, human message, optional structured details, and an optional next
//! action. It also provides redaction helpers that scrub known secrets and
//! sensitive content from diagnostic details before they leave the runtime.
//!
//! Redaction reuses [`crate::streaming_proxy::SecretRedactor`] so that the
//! secret patterns used by the streaming proxy (API keys, bearer tokens, and
//! sensitive field names) stay consistent across the codebase.
//!
//! Human/terminal formatting (color, alignment, tables) is intentionally kept
//! out of this layer; callers near the CLI render diagnostics. The [`Diagnostic`]
//! [`core::fmt::Display`] implementation is a stable single-line form suitable
//! for logs and tests, not a presentation format.

use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::streaming_proxy::SecretRedactor;

/// Diagnostic severity, ordered `Error` > `Warning` > `Info`.
///
/// The declaration order gives the derived [`Ord`] the desired ranking so that
/// diagnostics can be sorted by severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational observation; never represents a failure.
    Info,
    /// Recoverable or degraded behavior worth surfacing.
    Warning,
    /// A failure that should be acted on.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        })
    }
}

/// How aggressively diagnostic details are redacted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionMode {
    /// Default: scrub known secrets and sensitive content (prompts, tool
    /// output, environment blocks, commands, working directories, and absolute
    /// paths) so details are safe for a support report.
    Summary,
    /// Include additional local metadata but still scrub known secrets.
    Verbose,
}

/// The default redaction mode is [`RedactionMode::Summary`] (safe by default).
impl Default for RedactionMode {
    fn default() -> Self {
        RedactionMode::Summary
    }
}

/// A structured diagnostic record shared across runtime, provider, tool,
/// package, adapter, session, config, and RPC surfaces.
///
/// `code` and `source` are `&'static str` because they are stable identifiers
/// by design; a diagnostic is constructed from known literals, not dynamic
/// strings. `message` is human-readable and may be dynamic inside the runtime;
/// use [`Diagnostic::redacted_payload`] before crossing public boundaries.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    /// Severity of the observation.
    pub severity: Severity,
    /// Stable snake_case identifier suitable for tests and matching.
    pub code: &'static str,
    /// Owning subsystem, e.g. one of the `SOURCE_*` constants.
    pub source: &'static str,
    /// Short human-readable explanation.
    pub message: String,
    /// Optional structured metadata; emitted redacted via [`Self::redacted_details`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Optional suggested next step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

/// Public diagnostic representation after applying the selected redaction mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticPayload {
    /// Severity of the observation.
    pub severity: Severity,
    /// Stable snake_case identifier suitable for tests and matching.
    pub code: String,
    /// Owning subsystem, e.g. one of the `SOURCE_*` constants.
    pub source: String,
    /// Redacted short human-readable explanation.
    pub message: String,
    /// Optional redacted structured metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Optional redacted suggested next step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

impl Diagnostic {
    /// Create a diagnostic with the required stable fields.
    pub fn new(
        severity: Severity,
        code: &'static str,
        source: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code,
            source,
            message: message.into(),
            details: None,
            action: None,
        }
    }

    /// Attach structured details.
    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Attach a suggested next action.
    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// Return the details redacted for the given mode, or `None` if unset.
    ///
    /// This helper preserves the legacy internal behavior: it only touches
    /// `details`. Use [`Self::redacted_payload`] before exposing diagnostics
    /// outside the runtime.
    pub fn redacted_details(&self, mode: RedactionMode) -> Option<serde_json::Value> {
        self.details.as_ref().map(|value| redact(value, mode))
    }

    /// Return a public diagnostic payload with all dynamic fields redacted.
    pub fn redacted_payload(&self, mode: RedactionMode) -> DiagnosticPayload {
        DiagnosticPayload {
            severity: self.severity,
            code: self.code.to_owned(),
            source: self.source.to_owned(),
            message: redact_text(&self.message, mode),
            details: self.redacted_details(mode),
            action: self.action.as_ref().map(|action| redact_text(action, mode)),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.action {
            Some(action) => write!(
                f,
                "[{}] {}::{}: {} (action: {})",
                self.severity, self.source, self.code, self.message, action
            ),
            None => write!(
                f,
                "[{}] {}::{}: {}",
                self.severity, self.source, self.code, self.message
            ),
        }
    }
}

impl fmt::Display for DiagnosticPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.action {
            Some(action) => write!(
                f,
                "[{}] {}::{}: {} (action: {})",
                self.severity, self.source, self.code, self.message, action
            ),
            None => write!(
                f,
                "[{}] {}::{}: {}",
                self.severity, self.source, self.code, self.message
            ),
        }
    }
}

/// Stable subsystem source identifiers, forming the shared diagnostic
/// vocabulary. Application-specific sources live where they are produced but
/// share these constant spellings.
pub const SOURCE_PROVIDER: &str = "provider";
pub const SOURCE_TOOL: &str = "tool";
pub const SOURCE_PACKAGE: &str = "package";
pub const SOURCE_ADAPTER: &str = "adapter";
pub const SOURCE_SESSION: &str = "session";
pub const SOURCE_CONFIG: &str = "config";
pub const SOURCE_RPC: &str = "rpc";
pub const SOURCE_TUI: &str = "tui";
/// Agent-runtime-internal observations: hook failures, cancellation, and the
/// max-turns guard. Distinct from `SOURCE_RPC` (the JSONL protocol) and
/// `SOURCE_TOOL` (built-in/extension tools) so the producing subsystem is
/// reported accurately regardless of whether the loop is driven by RPC,
/// interactive, or non-interactive mode.
pub const SOURCE_AGENT: &str = "agent";

/// Stable snake_case diagnostic codes. Declared as named constants so a typo
/// in a code literal is a compile error (when referenced via the constant) and
/// a typo in the constant value is caught by the code-stability tests.
pub mod code {
    pub const CODE_PROVIDER_AUTH_FAILED: &str = "provider_auth_failed";
    pub const CODE_PROVIDER_RATE_LIMITED: &str = "provider_rate_limited";
    pub const CODE_PROVIDER_TIMEOUT: &str = "provider_timeout";
    pub const CODE_PROVIDER_REQUEST_FAILED: &str = "provider_request_failed";
    pub const CODE_PROVIDER_STREAM_ERROR: &str = "provider_stream_error";
    /// Generic provider failure surfaced through the agent loop when the
    /// structured [`opi_ai::provider::ProviderError`] category is no longer
    /// recoverable (e.g. retries exhausted).
    pub const CODE_PROVIDER_ERROR: &str = "provider_error";
    pub const CODE_TOOL_FAILED: &str = "tool_failed";
    pub const CODE_HOOK_FAILED: &str = "hook_failed";
    pub const CODE_AGENT_CANCELLED: &str = "agent_cancelled";
    pub const CODE_AGENT_MAX_TURNS_EXCEEDED: &str = "agent_max_turns_exceeded";
    // Runtime emission codes (agent loop).
    pub const CODE_PROVIDER_RETRY_ATTEMPT: &str = "provider_retry_attempt";
    pub const CODE_PROVIDER_RETRY_SUCCEEDED: &str = "provider_retry_succeeded";
    pub const CODE_PROVIDER_RETRY_EXHAUSTED: &str = "provider_retry_exhausted";
    pub const CODE_PROVIDER_CAPABILITY_INVALID: &str = "provider_capability_invalid";
    pub const CODE_TOOL_UNKNOWN: &str = "tool_unknown";
    pub const CODE_TOOL_VALIDATION_FAILED: &str = "tool_validation_failed";
    pub const CODE_TOOL_EXECUTION_FAILED: &str = "tool_execution_failed";
    // Session/compaction classification codes.
    pub const CODE_SESSION_COMPACTED: &str = "session_compacted";
    pub const CODE_COMPACTION_NOTHING_TO_COMPACT: &str = "compaction_nothing_to_compact";
    pub const CODE_SESSION_CORRUPT_ENTRIES: &str = "session_corrupt_entries";
    pub const CODE_SESSION_TRUNCATED_LINE: &str = "session_truncated_line";
    pub const CODE_SESSION_CORRUPT_WITH_TRUNCATION: &str = "session_corrupt_with_truncation";
    // opi-coding-agent bridges (package/config). Package diagnostics carry a
    // dynamic granular code in `details.package_code`; the shared code is stable.
    pub const CODE_PACKAGE_DIAGNOSTIC: &str = "package_diagnostic";
    pub const CODE_PACKAGE_RESOLUTION_FAILED: &str = "package_resolution_failed";
    pub const CODE_CONFIG_PARSE_FAILED: &str = "config_parse_failed";
    pub const CODE_CONFIG_READ_FAILED: &str = "config_read_failed";
    pub const CODE_ADAPTER_PROTOCOL_UNSUPPORTED: &str = "adapter_protocol_unsupported";
    pub const CODE_ADAPTER_KIND_UNSUPPORTED: &str = "adapter_kind_unsupported";
    pub const CODE_ADAPTER_COMMAND_INVALID: &str = "adapter_command_invalid";
    pub const CODE_ADAPTER_STARTUP_FAILED: &str = "adapter_startup_failed";
    pub const CODE_ADAPTER_REGISTRATION_FAILED: &str = "adapter_registration_failed";
    pub const CODE_ADAPTER_HOST_DIAGNOSTIC: &str = "adapter_host_diagnostic";
    /// A local trace sink failed mid-run and was disabled (fail-open).
    pub const CODE_TRACE_SINK_FAILED: &str = "trace_sink_failed";
    /// A requested trace could not be prepared before the run (fail-closed).
    pub const CODE_TRACE_SETUP_FAILED: &str = "trace_setup_failed";
}

const REDACTED: &str = "[REDACTED]";

/// Field names whose full values are redacted in summary mode because they
/// carry prompts, tool output, environment blocks, commands, or working
/// directories.
const CONTENT_SENSITIVE_KEYS: &[&str] = &[
    "prompt",
    "prompts",
    "tool_output",
    "tool_result",
    "env",
    "environment",
    "command",
    "args",
    "cwd",
    "body",
    "request_body",
    "response_body",
    "provider_error",
    "headers",
    "stdout",
    "stderr",
    "tool_error",
    "hook_error",
    "trace_error",
    "package_error",
    "package_message",
    "adapter_error",
];

/// Heuristic absolute-path detector used in summary mode. Matches Windows drive
/// paths (`C:\`, `D:/`), UNC paths (`\\server`), and common POSIX absolute
/// roots (`/Users/`, `/home/`, `/var/`, ...). The Windows alternative requires a
/// non-alphanumeric boundary before the drive letter so that URLs such as
/// `https://` are not mistaken for drive paths.
static ABSOLUTE_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|[^A-Za-z0-9])[A-Za-z]:[\\/]|\\\\|/(?:Users|home|root|tmp|var|etc|opt|mnt|private|proc|sys|dev|srv|lib|run|app|data|usr|bin|sbin)/",
    )
    .expect("absolute path redaction regex must compile")
});

/// Redact a structured JSON value for the given mode.
///
/// Both modes scrub known secrets via [`SecretRedactor`] (API keys, bearer
/// tokens, and sensitive field names). [`RedactionMode::Summary`] additionally
/// redacts content-sensitive fields and any string value that looks like an
/// absolute path.
pub fn redact(value: &serde_json::Value, mode: RedactionMode) -> serde_json::Value {
    let scrubbed = SecretRedactor::default().redact(value);
    match mode {
        RedactionMode::Summary => redact_summary(&scrubbed),
        RedactionMode::Verbose => scrubbed,
    }
}

/// Redact a single string using the same policy as structured details.
pub fn redact_text(text: &str, mode: RedactionMode) -> String {
    match redact(&serde_json::Value::String(text.to_owned()), mode) {
        serde_json::Value::String(redacted) => redacted,
        _ => REDACTED.to_owned(),
    }
}

fn redact_summary(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (key, value) in map {
                let redacted = if is_content_sensitive_key(key) {
                    serde_json::Value::String(REDACTED.to_owned())
                } else {
                    redact_summary(value)
                };
                out.insert(key.clone(), redacted);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(redact_summary).collect())
        }
        serde_json::Value::String(s) => {
            if ABSOLUTE_PATH_RE.is_match(s) {
                serde_json::Value::String(REDACTED.to_owned())
            } else {
                serde_json::Value::String(s.clone())
            }
        }
        other => other.clone(),
    }
}

fn is_content_sensitive_key(key: &str) -> bool {
    CONTENT_SENSITIVE_KEYS
        .iter()
        .any(|sensitive| sensitive.eq_ignore_ascii_case(key))
}

// ---------------------------------------------------------------------------
// Classification bridges: map provider and agent-loop errors into Diagnostics.
// ---------------------------------------------------------------------------

/// Remediation hint attached to authentication failures.
const ACTION_CHECK_CREDENTIALS: &str = "check the API key or provider credentials";
/// Remediation hint attached to rate-limited responses.
const ACTION_SLOW_DOWN: &str = "reduce request frequency or wait for the retry-after delay";

/// Classify a [`opi_ai::provider::ProviderError`] into a [`Diagnostic`].
///
/// The provider taxonomy itself lives in `opi-ai` (`ProviderError::category`);
/// this bridge only fixes the diagnostic `severity`, `code`, and `source` for
/// each category. Runtime behavior is unchanged — the error is still returned
/// as-is; this only describes how it would be surfaced diagnostically.
impl From<&opi_ai::provider::ProviderError> for Diagnostic {
    fn from(error: &opi_ai::provider::ProviderError) -> Self {
        use opi_ai::provider::ProviderError;
        match error {
            ProviderError::RateLimited { retry_after_ms } => Diagnostic::new(
                Severity::Warning,
                code::CODE_PROVIDER_RATE_LIMITED,
                SOURCE_PROVIDER,
                "rate limited by provider",
            )
            .details_option(retry_after_ms.map(|ms| serde_json::json!({ "retry_after_ms": ms })))
            .action(ACTION_SLOW_DOWN),
            ProviderError::Timeout => Diagnostic::new(
                Severity::Warning,
                code::CODE_PROVIDER_TIMEOUT,
                SOURCE_PROVIDER,
                "provider request timed out",
            ),
            ProviderError::RequestFailed(message) => Diagnostic::new(
                Severity::Error,
                code::CODE_PROVIDER_REQUEST_FAILED,
                SOURCE_PROVIDER,
                "provider request failed",
            )
            .details(serde_json::json!({ "provider_error": message })),
            ProviderError::StreamError(message) => Diagnostic::new(
                Severity::Error,
                code::CODE_PROVIDER_STREAM_ERROR,
                SOURCE_PROVIDER,
                "provider stream failed",
            )
            .details(serde_json::json!({ "provider_error": message })),
            ProviderError::AuthFailed(message) => Diagnostic::new(
                Severity::Error,
                code::CODE_PROVIDER_AUTH_FAILED,
                SOURCE_PROVIDER,
                "provider authentication failed",
            )
            .details(serde_json::json!({ "provider_error": message }))
            .action(ACTION_CHECK_CREDENTIALS),
        }
    }
}

/// Classify an [`crate::loop_types::AgentError`] into a [`Diagnostic`].
///
/// `AgentError::Provider`/`AuthFailed` map onto the provider vocabulary; tool
/// and hook failures carry their own sources; cancellation is informational
/// (harness/user-initiated, not a failure); the max-turns guard is a warning.
impl From<&crate::loop_types::AgentError> for Diagnostic {
    fn from(error: &crate::loop_types::AgentError) -> Self {
        use crate::loop_types::AgentError;
        match error {
            AgentError::Provider(message) => Diagnostic::new(
                Severity::Error,
                code::CODE_PROVIDER_ERROR,
                SOURCE_PROVIDER,
                "provider error",
            )
            .details(serde_json::json!({ "provider_error": message })),
            AgentError::AuthFailed(message) => Diagnostic::new(
                Severity::Error,
                code::CODE_PROVIDER_AUTH_FAILED,
                SOURCE_PROVIDER,
                "provider authentication failed",
            )
            .details(serde_json::json!({ "provider_error": message }))
            .action(ACTION_CHECK_CREDENTIALS),
            AgentError::Tool(message) => Diagnostic::new(
                Severity::Error,
                code::CODE_TOOL_FAILED,
                SOURCE_TOOL,
                "tool failed",
            )
            .details(serde_json::json!({ "tool_error": message })),
            AgentError::Hook(message) => Diagnostic::new(
                Severity::Error,
                code::CODE_HOOK_FAILED,
                SOURCE_AGENT,
                "hook failed",
            )
            .details(serde_json::json!({ "hook_error": message })),
            AgentError::Cancelled => Diagnostic::new(
                Severity::Info,
                code::CODE_AGENT_CANCELLED,
                SOURCE_AGENT,
                "agent run cancelled",
            ),
            AgentError::MaxTurnsExceeded(max_turns) => Diagnostic::new(
                Severity::Warning,
                code::CODE_AGENT_MAX_TURNS_EXCEEDED,
                SOURCE_AGENT,
                format!("max turns exceeded ({max_turns})"),
            )
            .details(serde_json::json!({ "max_turns": max_turns }))
            .action("increase max_turns or narrow the task"),
            AgentError::TraceSetup(message) => Diagnostic::new(
                Severity::Error,
                code::CODE_TRACE_SETUP_FAILED,
                SOURCE_AGENT,
                "trace setup failed",
            )
            .details(serde_json::json!({ "trace_error": message }))
            .action("check the trace path is writable and its parent directory exists"),
        }
    }
}

impl Diagnostic {
    /// Like [`Diagnostic::details`] but takes an `Option`, leaving details
    /// unset when `None`. Used by classification bridges that only attach
    /// details for some variants.
    fn details_option(self, details: Option<serde_json::Value>) -> Self {
        match details {
            Some(value) => self.details(value),
            None => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacted_constant_is_stable() {
        assert_eq!(REDACTED, "[REDACTED]");
    }

    #[test]
    fn content_sensitive_keys_match_case_insensitively() {
        assert!(is_content_sensitive_key("prompt"));
        assert!(is_content_sensitive_key("TOOL_OUTPUT"));
        assert!(is_content_sensitive_key("Env"));
        assert!(!is_content_sensitive_key("endpoint"));
    }
}
