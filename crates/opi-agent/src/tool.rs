//! Tool calling abstraction (S8.2).

pub mod result;

use std::future::Future;
use std::pin::Pin;

use opi_ai::message::{OutputContent, ToolDef};
use tokio_util::sync::CancellationToken;

/// Callback for progress updates during tool execution.
pub type UpdateCallback = Box<dyn Fn(serde_json::Value) + Send + Sync>;

/// Tool trait — each concrete tool implements this.
pub trait Tool: Send + Sync {
    /// Return the tool's definition (name, description, JSON Schema for input).
    fn definition(&self) -> ToolDef;

    /// Execute the tool with validated arguments.
    fn execute(
        &self,
        call_id: &str,
        arguments: serde_json::Value,
        signal: CancellationToken,
        on_update: Option<UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>>;

    /// Whether this tool must run sequentially.
    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Parallel
    }
}

/// Owned, lightweight diagnostic entry carried on a [`ToolResult`].
///
/// Deliberately not coupled to [`crate::diagnostic`] so `tool.rs` keeps its
/// zero-internal-dependency layering. Task 11.8 lifts each entry into a Phase 7
/// [`Diagnostic`](crate::diagnostic::Diagnostic) plus a diagnostic-linked trace
/// record; until then the carrier is carried empty by every built-in tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDiagnostic {
    /// Stable snake_case code (forward-compatible with `CODE_TOOL_*` constants).
    pub code: String,
    /// Human-readable cause description.
    pub message: String,
    /// Structured per-cause payload; becomes `Diagnostic::details` at the 11.8 lift.
    pub context: serde_json::Value,
}

/// Result of a tool execution.
#[derive(Clone)]
pub struct ToolResult {
    pub content: Vec<OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub terminate: bool,
    /// Whether `content` was truncated (large file, capped output, partial walk).
    pub truncated: bool,
    /// Tool-owned structured failure context; lifted into diagnostics/trace in 11.8.
    pub diagnostics: Vec<ToolDiagnostic>,
}

impl ToolResult {
    /// Create an error tool result from a validation error.
    pub fn from_validation_error(err: crate::validation::ValidationError) -> Self {
        let message = err.to_string();
        Self {
            content: vec![OutputContent::Text { text: message }],
            details: None,
            is_error: true,
            terminate: false,
            truncated: false,
            diagnostics: Vec::new(),
        }
    }
}

/// Errors from tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("cancelled")]
    Cancelled,
}

/// Whether a tool runs sequentially or in parallel with others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Sequential,
    Parallel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_result_carries_truncated_and_diagnostics() {
        let result = ToolResult {
            content: Vec::new(),
            details: None,
            is_error: false,
            terminate: false,
            truncated: true,
            diagnostics: vec![ToolDiagnostic {
                code: "test_code".to_string(),
                message: "test message".to_string(),
                context: serde_json::json!({ "k": "v" }),
            }],
        };
        assert!(result.truncated);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "test_code");
        assert_eq!(result.diagnostics[0].message, "test message");
    }
}
