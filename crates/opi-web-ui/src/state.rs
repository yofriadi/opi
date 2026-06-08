//! Conversation state machine for processing RPC/SDK events.
//!
//! [`ConversationState`] tracks the full conversation lifecycle: messages,
//! tool calls, thinking blocks, model changes, session metadata, and
//! compaction state. Events are processed via [`ConversationState::process`]
//! and the resulting state can be rendered via
//! [`ConversationState::to_conversation_view`].
//!
//! **Unstable 0.x API** — these types may change between minor versions.

use crate::components::{ChatMessage, ConversationView, StatusBar, ToolCallView};
use crate::event::WebUiEvent;

/// Tracked RPC response for UI display.
#[derive(Debug, Clone)]
pub struct TrackedResponse {
    /// Command name.
    pub command: String,
    /// Whether the command succeeded.
    pub success: bool,
    /// Correlation ID.
    pub id: Option<String>,
    /// Error message, if any.
    pub error: Option<String>,
    /// Response payload, if any.
    pub data: Option<serde_json::Value>,
}

/// Conversation state machine that processes RPC/SDK events.
///
/// Maintains message history, tool call state, thinking blocks,
/// session metadata, and compaction status.
#[derive(Debug, Clone)]
pub struct ConversationState {
    messages: Vec<ChatMessage>,
    tool_calls: Vec<ToolCallView>,
    thinking_blocks: Vec<crate::components::ThinkingBlock>,
    model: Option<String>,
    session_id: Option<String>,
    turn_count: u64,
    message_count: u64,
    agent_running: bool,
    compacting: bool,
    last_response: Option<TrackedResponse>,
    resources: Option<serde_json::Value>,
    last_compaction: Option<serde_json::Value>,
    // Streaming state
    current_text: String,
    current_model: Option<String>,
    current_provider: Option<String>,
    current_thinking: String,
    streaming: bool,
    pending_tool_call_ids: Vec<String>,
}

impl ConversationState {
    /// Create a new empty conversation state.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            tool_calls: Vec::new(),
            thinking_blocks: Vec::new(),
            model: None,
            session_id: None,
            turn_count: 0,
            message_count: 0,
            agent_running: false,
            compacting: false,
            last_response: None,
            resources: None,
            last_compaction: None,
            current_text: String::new(),
            current_model: None,
            current_provider: None,
            current_thinking: String::new(),
            streaming: false,
            pending_tool_call_ids: Vec::new(),
        }
    }

    /// Process a single event, updating state accordingly.
    pub fn process(&mut self, event: WebUiEvent) {
        match event {
            WebUiEvent::RpcReady { .. } => {}
            WebUiEvent::RpcResponse {
                command,
                success,
                id,
                error,
                data,
            } => {
                if success {
                    self.apply_successful_rpc_data(&command, data.as_ref());
                }
                self.last_response = Some(TrackedResponse {
                    command,
                    success,
                    id,
                    error,
                    data,
                });
            }
            WebUiEvent::AgentStart => {
                self.agent_running = true;
            }
            WebUiEvent::AgentEnd { message_count } => {
                self.agent_running = false;
                self.message_count = message_count as u64;
                self.flush_message();
            }
            WebUiEvent::TurnStart => {
                self.turn_count += 1;
            }
            WebUiEvent::TurnEnd => {}
            WebUiEvent::MessageStart { model, provider } => {
                self.streaming = true;
                self.current_text.clear();
                self.current_thinking.clear();
                self.current_model = Some(model);
                self.current_provider = Some(provider);
                self.pending_tool_call_ids.clear();
            }
            WebUiEvent::TextDelta { delta, .. } => {
                self.current_text.push_str(&delta);
            }
            WebUiEvent::ThinkingStart { .. } => {
                self.current_thinking.clear();
            }
            WebUiEvent::ThinkingDelta { delta, .. } => {
                self.current_thinking.push_str(&delta);
            }
            WebUiEvent::ThinkingEnd { content, .. } => {
                self.current_thinking = content;
            }
            WebUiEvent::MessageEnd => {
                self.flush_message();
                self.streaming = false;
            }
            WebUiEvent::ToolStart {
                tool_call_id,
                tool_name,
                args,
            } => {
                self.pending_tool_call_ids.push(tool_call_id.clone());
                self.tool_calls
                    .push(ToolCallView::new(tool_call_id, tool_name, args));
            }
            WebUiEvent::ToolEnd {
                tool_call_id,
                is_error,
                result,
                ..
            } => {
                if let Some(tc) = self
                    .tool_calls
                    .iter_mut()
                    .find(|tc| tc.tool_call_id() == tool_call_id)
                {
                    if is_error {
                        tc.fail(result);
                    } else {
                        tc.complete(result);
                    }
                }
            }
            WebUiEvent::QueueUpdate { .. } => {}
            WebUiEvent::AutoRetryStart { .. } => {}
            WebUiEvent::CompactionStart { .. } => {
                self.compacting = true;
            }
            WebUiEvent::CompactionEnd { aborted: _, .. } => {
                self.compacting = false;
            }
            WebUiEvent::SessionInfo {
                session_id,
                turn_count,
                message_count,
            } => {
                self.session_id = Some(session_id);
                self.turn_count = turn_count;
                self.message_count = message_count;
            }
            WebUiEvent::ModelChanged { model } => {
                self.model = Some(model);
            }
            WebUiEvent::SessionPersistError { .. } => {}
            WebUiEvent::Unknown { .. } => {}
        }
    }

    /// Current messages in the conversation.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Current tool calls.
    pub fn tool_calls(&self) -> &[ToolCallView] {
        &self.tool_calls
    }

    /// Current thinking blocks.
    pub fn thinking_blocks(&self) -> &[crate::components::ThinkingBlock] {
        &self.thinking_blocks
    }

    /// Current model name.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Current session identifier.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Current turn count.
    pub fn turn_count(&self) -> u64 {
        self.turn_count
    }

    /// Current message count.
    pub fn message_count(&self) -> u64 {
        self.message_count
    }

    /// Whether the agent loop is currently running.
    pub fn agent_running(&self) -> bool {
        self.agent_running
    }

    /// Whether compaction is in progress.
    pub fn is_compacting(&self) -> bool {
        self.compacting
    }

    /// Last RPC response received.
    pub fn last_response(&self) -> Option<&TrackedResponse> {
        self.last_response.as_ref()
    }

    /// Last resource metadata received from `session_info`.
    pub fn resources(&self) -> Option<&serde_json::Value> {
        self.resources.as_ref()
    }

    /// Last successful compaction response payload.
    pub fn last_compaction(&self) -> Option<&serde_json::Value> {
        self.last_compaction.as_ref()
    }

    /// Build a [`ConversationView`] from the current state.
    pub fn to_conversation_view(&self) -> ConversationView {
        let mut view = ConversationView::new();
        for msg in &self.messages {
            view.add_message(msg.clone());
        }
        view
    }

    /// Build a [`StatusBar`] from the current state.
    pub fn to_status_bar(&self) -> StatusBar {
        let mut sb = StatusBar::new();
        if let Some(model) = &self.model {
            sb.set_model(model.clone());
        }
        if let Some(session_id) = &self.session_id {
            sb.set_session_id(session_id.clone());
        }
        sb
    }

    /// Flush the current streaming message into the message list.
    fn flush_message(&mut self) {
        let has_content = !self.current_text.is_empty()
            || !self.current_thinking.is_empty()
            || !self.pending_tool_call_ids.is_empty();

        if has_content {
            let model = self.current_model.take().unwrap_or_default();
            let provider = self.current_provider.take().unwrap_or_default();
            let mut msg = ChatMessage::new(std::mem::take(&mut self.current_text), model, provider);

            if !self.current_thinking.is_empty() {
                self.thinking_blocks
                    .push(crate::components::ThinkingBlock::new(std::mem::take(
                        &mut self.current_thinking,
                    )));
                msg = msg.with_thinking(
                    self.thinking_blocks
                        .last()
                        .map(|b| b.content().to_owned())
                        .unwrap_or_default(),
                );
            }

            // Attach tool calls that occurred during this message.
            for id in self.pending_tool_call_ids.drain(..) {
                if let Some(tc) = self.tool_calls.iter().find(|tc| tc.tool_call_id() == id) {
                    msg = msg.with_tool_call(tc.clone());
                }
            }

            self.messages.push(msg);
        }
    }

    fn apply_successful_rpc_data(&mut self, command: &str, data: Option<&serde_json::Value>) {
        let Some(data) = data.and_then(|value| value.as_object()) else {
            return;
        };

        match command {
            "session_info" => {
                if let Some(model) = data.get("model").and_then(|value| value.as_str()) {
                    self.model = Some(model.to_owned());
                }
                if let Some(session_id) = data.get("session_id").and_then(|value| value.as_str()) {
                    self.session_id = Some(session_id.to_owned());
                }
                if let Some(turn_count) = data.get("turn_count").and_then(|value| value.as_u64()) {
                    self.turn_count = turn_count;
                }
                if let Some(message_count) =
                    data.get("message_count").and_then(|value| value.as_u64())
                {
                    self.message_count = message_count;
                }
                if let Some(resources) = data.get("resources") {
                    self.resources = Some(resources.clone());
                }
            }
            "set_model" => {
                if let Some(model) = data.get("model").and_then(|value| value.as_str()) {
                    self.model = Some(model.to_owned());
                }
            }
            "compact" => {
                self.compacting = false;
                self.last_compaction = Some(serde_json::Value::Object(data.clone()));
            }
            _ => {}
        }
    }
}

impl Default for ConversationState {
    fn default() -> Self {
        Self::new()
    }
}
