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

/// A structured diagnostic record shared across runtime, provider, tool,
/// package, adapter, session, config, and RPC surfaces.
///
/// `code` and `source` are `&'static str` because they are stable identifiers
/// by design; a diagnostic is constructed from known literals, not dynamic
/// strings. `message` is human-readable and may be dynamic.
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
    /// Redaction never touches the severity, code, source, message, or action
    /// fields.
    pub fn redacted_details(&self, mode: RedactionMode) -> Option<serde_json::Value> {
        self.details.as_ref().map(|value| redact(value, mode))
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
