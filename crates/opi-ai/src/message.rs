//! Provider-facing message types (S7.1).

use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User(UserMessage),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    #[serde(rename = "tool_result")]
    ToolResult(ToolResultMessage),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserMessage {
    pub content: Vec<InputContent>,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub content: Vec<AssistantContent>,
    pub api: crate::ApiKind,
    pub provider: String,
    pub model: String,
    pub response_model: Option<String>,
    pub response_id: Option<String>,
    pub usage: crate::stream::Usage,
    pub stop_reason: crate::stream::StopReason,
    pub error_message: Option<String>,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub tool_call_id: String,
    pub tool_name: String,
    pub content: Vec<OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub timestamp_ms: i64,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AssistantContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_call")]
    ToolCall { tool_call: ToolCall },
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutputContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
