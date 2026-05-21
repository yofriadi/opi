//! Hook trait for agent loop customization (S8.2).

use std::future::Future;
use std::pin::Pin;

use crate::loop_types::AgentError;
use crate::message::AgentMessage;

/// Context provided before a tool call is executed.
pub struct BeforeToolCallContext {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub messages: Vec<AgentMessage>,
}

/// Result of the before_tool_call hook.
#[non_exhaustive]
pub enum BeforeToolCallResult {
    /// Allow the tool call to proceed.
    Allow,
    /// Reject the tool call with an error message.
    Deny { reason: String },
}

/// Hook trait for customizing agent loop behavior.
///
/// Default implementations are no-ops or basic conversions.
pub trait AgentHooks: Send + Sync {
    /// Convert agent messages to provider-facing messages.
    fn convert_to_llm(
        &self,
        messages: &[AgentMessage],
    ) -> Result<Vec<opi_ai::message::Message>, AgentError>;

    /// Decide whether the loop should stop after this turn.
    fn should_stop_after_turn(
        &self,
        _messages: &[AgentMessage],
        _tool_results: &[opi_ai::message::ToolResultMessage],
    ) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        Box::pin(async { false })
    }

    /// Hook called before a tool is executed.
    fn before_tool_call(
        &self,
        _ctx: BeforeToolCallContext,
    ) -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>> {
        Box::pin(async { BeforeToolCallResult::Allow })
    }

    /// Hook called after a tool has been executed.
    fn after_tool_call(
        &self,
        _tool_call_id: &str,
        _tool_name: &str,
        _result: &crate::tool::ToolResult,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}
