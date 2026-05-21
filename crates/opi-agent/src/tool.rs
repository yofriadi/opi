//! Tool calling abstraction (S8.2).

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

/// Result of a tool execution.
#[derive(Clone)]
pub struct ToolResult {
    pub content: Vec<OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub terminate: bool,
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
