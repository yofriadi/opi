//! Agent event protocol (S7.4).

use crate::message::AgentMessage;

/// Callback type for emitting agent events to subscribers.
pub type AgentEventSink = Box<dyn Fn(AgentEvent) + Send + Sync>;

/// Events emitted during the agent loop lifecycle.
#[non_exhaustive]
#[derive(Debug)]
pub enum AgentEvent {
    /// The agent loop has started.
    AgentStart,
    /// The agent loop has ended. No more loop events will be emitted.
    AgentEnd { messages: Vec<AgentMessage> },
    /// A new turn (provider request/response cycle) has started.
    TurnStart,
    /// A turn has ended with the assistant message and any tool results.
    TurnEnd {
        message: AgentMessage,
        tool_results: Vec<opi_ai::message::ToolResultMessage>,
    },
    /// An assistant message has started streaming.
    MessageStart { message: AgentMessage },
    /// An assistant message has been updated with a stream event.
    MessageUpdate {
        message: AgentMessage,
        assistant_event: Box<opi_ai::stream::AssistantStreamEvent>,
    },
    /// An assistant message has finished streaming.
    MessageEnd { message: AgentMessage },
    /// Tool execution is about to begin.
    ToolExecutionStart {
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
    },
    /// Tool execution has produced a progress update.
    ToolExecutionUpdate {
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
        partial_result: serde_json::Value,
    },
    /// Tool execution has completed.
    ToolExecutionEnd {
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
        is_error: bool,
    },
    /// Queue messages were delivered to the conversation.
    QueueUpdate {
        steering: Vec<String>,
        follow_up: Vec<String>,
    },
}
