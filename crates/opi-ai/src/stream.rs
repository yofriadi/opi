//! Streaming response events.

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    Thinking(String),
    Usage {
        input_tokens: u32,
        output_tokens: u32,
    },
    Stop,
}
