//! Adapter JSONL protocol types for process-based extension adapters.
//!
//! Defines the wire protocol between the opi host and adapter child processes.
//! The protocol is JSONL (one JSON object per line) with a `type` discriminator
//! field on every message.
//!
//! # Message Flow
//!
//! ```text
//! Host                              Adapter
//!  |                                   |
//!  |--- initialize ------------------>|  (handshake)
//!  |<-- capabilities -----------------|  (advertise tools/commands/hooks)
//!  |                                   |
//!  |--- tool_call -------------------->|  (request)
//!  |<-- tool_result ------------------|  (response)
//!  |                                   |
//!  |--- command ---------------------->|  (request)
//!  |<-- command_result ---------------|  (response)
//!  |                                   |
//!  |--- hook ------------------------->|  (request)
//!  |<-- hook_result -------------------|  (response)
//!  |                                   |
//!  |--- event ------------------------>|  (fire-and-forget)
//!  |                                   |
//!  |--- state_serialize -------------->|  (request)
//!  |<-- state_result ------------------|  (response)
//!  |                                   |
//!  |--- state_restore ---------------->|  (request)
//!  |<-- state_result ------------------|  (response)
//!  |                                   |
//!  |--- cancel ----------------------->|  (best-effort, no response)
//!  |                                   |
//!  |--- shutdown ---------------------->|  (best-effort, no response)
//!  |                                   |
//! ```
//!
//! # Version Negotiation
//!
//! The host sends its protocol version in the `initialize` message. The adapter
//! responds with `capabilities` only if it supports the same version. If the
//! version does not match, the host disables the runtime adapter and static
//! package resources still load. Version negotiation is **exact-match** in the
//! Phase 5 MVP.
//!
//! # Failure Semantics
//!
//! | Failure                          | Behavior                                            |
//! |----------------------------------|-----------------------------------------------------|
//! | adapter spawn fails              | package becomes degraded; static resources load     |
//! | initialize times out             | runtime adapter disabled; diagnostic explains        |
//! | protocol version mismatch        | runtime adapter disabled; doctor reports versions   |
//! | tool call times out              | error tool result returned for that call            |
//! | adapter crashes                  | runtime unavailable; pending calls fail             |
//! | before-tool hook times out       | fail closed, block the tool                         |
//! | after-tool hook times out        | fail open, record diagnostic                        |
//! | event delivery backpressures     | drop event, record diagnostic                       |
//! | state serialization fails        | continue shutdown, report persistence diagnostic    |
//!
//! # Unstable
//!
//! This module is part of the **unstable 0.x extension API**. Breaking changes
//! may occur between minor versions without a major version bump.

use serde::{Deserialize, Serialize};

/// Protocol version string for the JSONL adapter protocol.
///
/// Must match the `protocol` field in the package manifest `[adapter]` table
/// exactly. The host validates this during initialization handshake.
pub const PROTOCOL_VERSION: &str = "opi-extension-jsonl-v1";

// ---------------------------------------------------------------------------
// Capability structs
// ---------------------------------------------------------------------------

/// A tool capability advertised by an adapter during initialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterToolCapability {
    /// Tool name (e.g. `"todo_add"`).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// A command capability advertised by an adapter during initialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterCommandCapability {
    /// Command name (e.g. `"todo/list"`).
    pub name: String,
    /// Human-readable description of the command.
    pub description: String,
}

/// A model override advertised by an adapter.
///
/// When the adapter declares a model override, the host routes tool calls
/// for the specified tools through the adapter's preferred model instead of
/// the session default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterModelOverride {
    /// The model identifier to use.
    pub model: String,
    /// Tool names whose calls should be routed through this model.
    pub tools: Vec<String>,
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages
// ---------------------------------------------------------------------------

/// Messages sent from the opi host to the adapter child process.
///
/// Each variant carries a `type` tag for JSONL discrimination via
/// `#[serde(tag = "type")]`. All request-response messages carry an `id`
/// field; the host owns id generation and correlates responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdapterHostMessage {
    /// Start the initialization handshake.
    Initialize {
        id: String,
        /// Protocol version the host supports.
        protocol: String,
        /// Package name from the manifest.
        package: String,
    },

    /// Invoke a tool on the adapter.
    ToolCall {
        id: String,
        /// Tool name as advertised in capabilities.
        tool: String,
        /// Tool input arguments as a JSON object.
        args: serde_json::Value,
    },

    /// Dispatch a command to the adapter.
    Command {
        id: String,
        /// Command name as advertised in capabilities.
        name: String,
        /// Command arguments as a JSON object.
        args: serde_json::Value,
    },

    /// Invoke a lifecycle hook on the adapter.
    Hook {
        id: String,
        /// Hook name (e.g. `"before_tool_call"`, `"event"`).
        hook: String,
        /// Hook-specific payload.
        payload: serde_json::Value,
    },

    /// Fire-and-forget event notification.
    ///
    /// The host may drop events under backpressure. The adapter must not
    /// block on event processing.
    Event { event: serde_json::Value },

    /// Request the adapter to serialize its current state.
    StateSerialize { id: String },

    /// Request the adapter to restore a previously serialized state.
    StateRestore {
        id: String,
        state: serde_json::Value,
    },

    /// Best-effort cancellation of an in-flight request.
    ///
    /// Sent when the host's cancellation token fires. The adapter should
    /// stop work, but the host still enforces the local timeout.
    Cancel { id: String, reason: String },

    /// Best-effort shutdown notification.
    ///
    /// Sent when the host is shutting down. The adapter should clean up
    /// and exit. The host will reap the child process after a timeout.
    Shutdown { id: String, reason: String },
}

// ---------------------------------------------------------------------------
// Adapter -> Host messages
// ---------------------------------------------------------------------------

/// Messages sent from the adapter child process to the opi host.
///
/// Each variant carries a `type` tag for JSONL discrimination. Response
/// messages carry the same `id` as the corresponding request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdapterProcessMessage {
    /// Response to `initialize`: advertise capabilities.
    Capabilities {
        id: String,
        /// Tools this adapter provides.
        tools: Vec<AdapterToolCapability>,
        /// Commands this adapter handles.
        commands: Vec<AdapterCommandCapability>,
        /// Hook names this adapter implements (e.g. `"before_tool_call"`).
        hooks: Vec<String>,
        /// Model overrides for specific tools.
        model_overrides: Vec<AdapterModelOverride>,
    },

    /// Response to `tool_call`.
    ToolResult {
        id: String,
        /// Result content blocks (typically `{"type":"text","text":"..."}`).
        content: Vec<serde_json::Value>,
        /// Whether this result represents an error.
        is_error: bool,
    },

    /// Response to `command`.
    CommandResult {
        id: String,
        /// Command result data.
        data: serde_json::Value,
    },

    /// Response to `hook`.
    HookResult {
        id: String,
        /// `"continue"` to allow the operation, `"block"` to deny it.
        action: String,
        /// Optional extra data (e.g. block reason).
        data: Option<serde_json::Value>,
    },

    /// Response to `state_serialize` or `state_restore`.
    StateResult {
        id: String,
        /// Serialized adapter state.
        state: serde_json::Value,
    },

    /// Error from the adapter.
    ///
    /// May or may not be correlated with a specific request `id`.
    Error {
        /// Request ID this error relates to, if any.
        id: Option<String>,
        /// Human-readable error message.
        message: String,
    },
}

impl AdapterProcessMessage {
    /// Returns `true` if this is a `ToolResult` with `is_error` set.
    pub fn is_error(&self) -> bool {
        match self {
            AdapterProcessMessage::ToolResult { is_error, .. } => *is_error,
            _ => false,
        }
    }
}
