//! UI component types for web-based AI chat rendering.
//!
//! Provides typed component models for chat messages, tool call views,
//! thinking blocks, status bars, and conversation containers. All components
//! implement the [`Render`] trait for HTML output.
//!
//! **Unstable 0.x API** — these types may change between minor versions.

use crate::render::{Render, escape_html};

/// Execution status of a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    /// Tool is currently running.
    Running,
    /// Tool completed successfully.
    Completed,
    /// Tool execution failed.
    Failed,
}

/// A rendered tool call with status tracking.
#[derive(Debug, Clone)]
pub struct ToolCallView {
    tool_call_id: String,
    tool_name: String,
    args: serde_json::Value,
    status: ToolCallStatus,
    result: Option<serde_json::Value>,
    is_error: bool,
}

impl ToolCallView {
    /// Create a new tool call view in running state.
    pub fn new(tool_call_id: String, tool_name: String, args: serde_json::Value) -> Self {
        Self {
            tool_call_id,
            tool_name,
            args,
            status: ToolCallStatus::Running,
            result: None,
            is_error: false,
        }
    }

    /// Tool call identifier.
    pub fn tool_call_id(&self) -> &str {
        &self.tool_call_id
    }

    /// Tool name (e.g. "read", "bash").
    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    /// Tool input arguments.
    pub fn args(&self) -> &serde_json::Value {
        &self.args
    }

    /// Current execution status.
    pub fn status(&self) -> ToolCallStatus {
        self.status
    }

    /// Tool result, if completed.
    pub fn result(&self) -> Option<&serde_json::Value> {
        self.result.as_ref()
    }

    /// Whether the tool execution resulted in an error.
    pub fn is_error(&self) -> bool {
        self.is_error
    }

    /// Mark the tool as completed successfully.
    pub fn complete(&mut self, result: serde_json::Value) {
        self.status = ToolCallStatus::Completed;
        self.result = Some(result);
        self.is_error = false;
    }

    /// Mark the tool as failed.
    pub fn fail(&mut self, result: serde_json::Value) {
        self.status = ToolCallStatus::Failed;
        self.result = Some(result);
        self.is_error = true;
    }
}

impl Render for ToolCallView {
    fn render_html(&self) -> String {
        let status_class = match self.status {
            ToolCallStatus::Running => "running",
            ToolCallStatus::Completed => "completed",
            ToolCallStatus::Failed => "error",
        };
        let mut html = format!(r#"<div class="tool-call {status_class}">"#,);
        html.push_str(&format!(
            r#"<div class="tool-call-header">{name}</div>"#,
            name = escape_html(&self.tool_name),
        ));
        if let Some(result) = &self.result {
            let result_str = match result {
                serde_json::Value::String(s) => escape_html(s),
                other => escape_html(&other.to_string()),
            };
            html.push_str(&format!(
                r#"<pre class="tool-call-result">{result_str}</pre>"#
            ));
        }
        html.push_str("</div>");
        html
    }
}

/// A thinking/reasoning block in an assistant message.
#[derive(Debug, Clone)]
pub struct ThinkingBlock {
    content: String,
}

impl ThinkingBlock {
    /// Create a new thinking block.
    pub fn new(content: String) -> Self {
        Self { content }
    }

    /// Thinking content text.
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Render for ThinkingBlock {
    fn render_html(&self) -> String {
        format!(
            r#"<div class="thinking"><pre>{}</pre></div>"#,
            escape_html(&self.content)
        )
    }
}

/// A chat message in the conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    text: String,
    model: String,
    provider: String,
    thinking: Option<String>,
    tool_calls: Vec<ToolCallView>,
}

impl ChatMessage {
    /// Create a new chat message with text content.
    pub fn new(text: String, model: String, provider: String) -> Self {
        Self {
            text,
            model,
            provider,
            thinking: None,
            tool_calls: Vec::new(),
        }
    }

    /// Message text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Model that produced this message.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Provider that produced this message.
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Thinking content, if any.
    pub fn thinking(&self) -> Option<&str> {
        self.thinking.as_deref()
    }

    /// Tool calls associated with this message.
    pub fn tool_calls(&self) -> &[ToolCallView] {
        &self.tool_calls
    }

    /// Attach thinking content.
    pub fn with_thinking(mut self, thinking: String) -> Self {
        self.thinking = Some(thinking);
        self
    }

    /// Attach a tool call.
    pub fn with_tool_call(mut self, tool_call: ToolCallView) -> Self {
        self.tool_calls.push(tool_call);
        self
    }
}

impl Render for ChatMessage {
    fn render_html(&self) -> String {
        let mut html = String::from(r#"<div class="chat-message">"#);
        if let Some(thinking) = &self.thinking {
            html.push_str(&ThinkingBlock::new(thinking.clone()).render_html());
        }
        for tc in &self.tool_calls {
            html.push_str(&tc.render_html());
        }
        html.push_str(&format!(
            r#"<div class="message-text">{}</div>"#,
            escape_html(&self.text)
        ));
        html.push_str("</div>");
        html
    }
}

/// Status bar showing session metadata.
#[derive(Debug, Clone, Default)]
pub struct StatusBar {
    model: Option<String>,
    session_id: Option<String>,
}

impl StatusBar {
    /// Create an empty status bar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Current model name.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Current session identifier.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Set the model name.
    pub fn set_model(&mut self, model: String) {
        self.model = Some(model);
    }

    /// Set the session identifier.
    pub fn set_session_id(&mut self, session_id: String) {
        self.session_id = Some(session_id);
    }
}

impl Render for StatusBar {
    fn render_html(&self) -> String {
        let model = self.model.as_deref().unwrap_or("unknown");
        let session = self.session_id.as_deref().unwrap_or("no session");
        format!(
            r#"<div class="status-bar"><span class="status-model">{}</span><span class="status-session">{}</span></div>"#,
            escape_html(model),
            escape_html(session),
        )
    }
}

/// A conversation view aggregating messages.
#[derive(Debug, Clone, Default)]
pub struct ConversationView {
    messages: Vec<ChatMessage>,
}

impl ConversationView {
    /// Create an empty conversation view.
    pub fn new() -> Self {
        Self::default()
    }

    /// Messages in the conversation.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Add a message to the conversation.
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }
}

impl Render for ConversationView {
    fn render_html(&self) -> String {
        let mut html = String::from(r#"<div class="conversation">"#);
        for msg in &self.messages {
            html.push_str(&msg.render_html());
        }
        html.push_str("</div>");
        html
    }
}
