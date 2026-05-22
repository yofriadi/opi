//! Types for the agent loop (S6.1, S8.2).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::message::AgentMessage;
use crate::tool::Tool;
use opi_ai::provider::Provider;

/// Errors that can occur during the agent loop.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("authentication failed: {0}")]
    AuthFailed(String),
    #[error("tool error: {0}")]
    Tool(String),
    #[error("hook error: {0}")]
    Hook(String),
    #[error("cancelled")]
    Cancelled,
    #[error("max turns exceeded ({0})")]
    MaxTurnsExceeded(u32),
}

/// Input context for the agent loop.
pub struct AgentLoopContext {
    /// The LLM provider.
    pub provider: Box<dyn Provider>,
    /// Available tools.
    pub tools: Vec<Box<dyn Tool>>,
    /// Initial conversation messages.
    pub messages: Vec<AgentMessage>,
    /// Model identifier to use.
    pub model: String,
    /// Optional system prompt.
    pub system: Option<String>,
    /// Steering queue (high-priority user messages injected before next turn).
    pub steering_queue: Option<Arc<Mutex<VecDeque<String>>>>,
    /// Follow-up queue (messages injected when agent would otherwise stop).
    pub follow_up_queue: Option<Arc<Mutex<VecDeque<String>>>>,
}

/// Configuration for the agent loop.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Maximum number of turns before stopping.
    pub max_turns: u32,
    /// Maximum output tokens per request.
    pub max_tokens: Option<u64>,
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Retry configuration for retryable provider errors.
    pub retry: Option<opi_ai::retry::RetryConfig>,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_turns: 50,
            max_tokens: None,
            temperature: None,
            retry: None,
        }
    }
}

/// Update returned by `prepare_next_turn` to modify the next turn.
pub struct AgentLoopTurnUpdate {
    pub extra_messages: Vec<AgentMessage>,
}
