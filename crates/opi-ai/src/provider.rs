//! LLM provider abstraction (S8.1).

use std::pin::Pin;

use futures_core::Stream;
use tokio_util::sync::CancellationToken;

use crate::message::{InputContent, Message, ToolDef};
use crate::stream::AssistantStreamEvent;

/// Provider trait — each concrete provider (Anthropic, OpenAI, etc.) implements this.
pub trait Provider: Send + Sync {
    /// Unique identifier for this provider instance (e.g. "anthropic").
    fn id(&self) -> &str;

    /// Models supported by this provider.
    fn models(&self) -> &[ModelInfo];

    /// Start a streaming request. Returns an `EventStream` that yields events
    /// until a terminal event (`Done` or `Error`) is reached or the caller
    /// cancels via `Request::cancel`.
    fn stream(&self, request: Request) -> EventStream;
}

/// Stream of assistant events from a provider.
pub type EventStream =
    Pin<Box<dyn Stream<Item = Result<AssistantStreamEvent, ProviderError>> + Send>>;

/// A single request to a provider.
pub struct Request {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub thinking: ThinkingConfig,
    pub stop_sequences: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub cancel: CancellationToken,
}

impl Request {
    /// Returns true when any user message contains image input.
    pub fn contains_image_input(&self) -> bool {
        self.messages.iter().any(|message| match message {
            Message::User(user) => user
                .content
                .iter()
                .any(|content| matches!(content, InputContent::Image { .. })),
            _ => false,
        })
    }
}

/// Thinking/reasoning configuration for extended thinking models.
#[derive(Debug, Clone, Default)]
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<u64>,
}

/// Metadata about a model offered by a provider.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub supports_images: bool,
    pub supports_streaming: bool,
    pub supports_thinking: bool,
}

/// Validate request content against model capabilities known by the provider.
///
/// Unknown model IDs are left to the provider implementation so configured
/// custom deployments can still work. Known text-only models fail locally
/// before any network call is attempted.
pub fn validate_request_capabilities(
    provider: &dyn Provider,
    request: &Request,
) -> Result<(), ProviderError> {
    if !request.contains_image_input() {
        return Ok(());
    }

    let model_id = request
        .model
        .split_once(':')
        .map(|(provider_id, model_id)| {
            if provider_id == provider.id() {
                model_id
            } else {
                request.model.as_str()
            }
        })
        .unwrap_or(request.model.as_str());

    let Some(model) = provider.models().iter().find(|m| m.id == model_id) else {
        return Ok(());
    };

    if model.supports_images {
        return Ok(());
    }

    Err(ProviderError::RequestFailed(format!(
        "model '{}' for provider '{}' does not support image input",
        model.id,
        provider.id()
    )))
}

/// Errors that can occur during provider streaming.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("rate limited")]
    RateLimited { retry_after_ms: Option<u64> },
    #[error("request timed out")]
    Timeout,
    #[error("request failed: {0}")]
    RequestFailed(String),
    #[error("stream error: {0}")]
    StreamError(String),
    #[error("authentication failed: {0}")]
    AuthFailed(String),
}

impl ProviderError {
    /// Whether this error is retryable (rate-limited or timeout).
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ProviderError::RateLimited { .. } | ProviderError::Timeout
        )
    }
}

/// Discriminant for the kind of provider backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Google,
    Mistral,
    Bedrock,
    Azure,
}
