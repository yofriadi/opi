//! Hook trait for agent loop customization (S8.2).

use std::future::Future;
use std::pin::Pin;

use tokio_util::sync::CancellationToken;

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

/// Context provided after a tool call has been executed.
pub struct AfterToolCallContext {
    pub tool_call_id: String,
    pub tool_name: String,
    pub result: crate::tool::ToolResult,
}

/// Result of the after_tool_call hook.
#[non_exhaustive]
pub enum AfterToolCallResult {
    /// Keep the original tool result unchanged.
    Keep,
    /// Replace the tool result entirely (field replacement, no deep merge).
    Replace(crate::tool::ToolResult),
}

/// Context provided to the should_stop_after_turn hook.
pub struct ShouldStopAfterTurnContext {
    pub messages: Vec<AgentMessage>,
    pub tool_results: Vec<opi_ai::message::ToolResultMessage>,
}

/// Context provided to the prepare_next_turn hook.
pub struct PrepareNextTurnContext {
    pub messages: Vec<AgentMessage>,
    pub turn: u32,
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

    /// Transform messages before conversion to LLM format.
    fn transform_context(
        &self,
        messages: Vec<AgentMessage>,
        _signal: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<AgentMessage>, AgentError>> + Send>> {
        Box::pin(async { Ok(messages) })
    }

    /// Decide whether the loop should stop after this turn.
    fn should_stop_after_turn(
        &self,
        _ctx: ShouldStopAfterTurnContext,
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
        _ctx: AfterToolCallContext,
    ) -> Pin<Box<dyn Future<Output = AfterToolCallResult> + Send>> {
        Box::pin(async { AfterToolCallResult::Keep })
    }

    /// Prepare context before the next turn begins.
    fn prepare_next_turn(
        &self,
        _ctx: PrepareNextTurnContext,
    ) -> Pin<Box<dyn Future<Output = Option<crate::loop_types::AgentLoopTurnUpdate>> + Send>> {
        Box::pin(async { None })
    }
}
