//! Agent event protocol (S7.4).

use serde::{Deserialize, Serialize};

use crate::message::AgentMessage;
use crate::session_event::{CompactionReason, CompactionResult};

/// Callback type for emitting agent events to subscribers.
pub type AgentEventSink = Box<dyn Fn(AgentEvent) + Send + Sync>;

/// Events emitted during the agent loop lifecycle.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
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
        #[serde(
            serialize_with = "serde_json_bridge::serialize",
            deserialize_with = "deserialize_boxed_stream_event"
        )]
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
        details: Option<serde_json::Value>,
        is_error: bool,
        #[serde(default)]
        truncated: bool,
    },
    /// Queue messages were delivered to the conversation.
    QueueUpdate {
        steering: Vec<String>,
        follow_up: Vec<String>,
    },
    /// A retryable provider error occurred; a retry attempt is starting.
    AutoRetryStart {
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        error_message: String,
    },
    /// A retry attempt concluded (either successfully or after exhausting attempts).
    AutoRetryEnd {
        success: bool,
        attempt: u32,
        final_error: Option<String>,
    },
    /// Compaction has started. Emitted by the harness outside the agent loop.
    CompactionStart { reason: CompactionReason },
    /// Compaction has finished. Emitted by the harness outside the agent loop.
    CompactionEnd {
        reason: CompactionReason,
        result: Option<CompactionResult>,
        aborted: bool,
        error_message: Option<String>,
    },
    /// Session persistence failed (disk full, permissions, etc.).
    SessionPersistError { message: String },
}

fn deserialize_boxed_stream_event<'de, D>(
    deserializer: D,
) -> Result<Box<opi_ai::stream::AssistantStreamEvent>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    let event: opi_ai::stream::AssistantStreamEvent =
        serde_json::from_value(v).map_err(serde::de::Error::custom)?;
    Ok(Box::new(event))
}

mod serde_json_bridge {
    use serde::{Serialize as _, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: serde::Serialize,
        S: Serializer,
    {
        serde_json::to_value(value)
            .map_err(serde::ser::Error::custom)
            .and_then(|v| v.serialize(serializer))
    }
}
