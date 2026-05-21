//! Agent-level message types (S7.2).

/// Messages within the agent loop.
///
/// Wraps provider-facing `Message` types and adds session-level variants
/// that never reach the provider.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum AgentMessage {
    /// A provider-facing message (user, assistant, or tool result).
    Llm(opi_ai::message::Message),
}
