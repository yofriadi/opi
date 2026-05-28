//! Mistral provider profile  - routes through the OpenAI-compatible adapter.
//!
//! Mistral AI (<https://mistral.ai>) provides an OpenAI-compatible Chat
//! Completions API at `https://api.mistral.ai/v1/chat/completions`. This
//! module creates a pre-configured [`OpenAiChatProvider`] with Mistral's
//! base URL and a curated model list.

use crate::openai_chat::{CompatConfig, OpenAiChatProvider};
use crate::provider::ModelInfo;

/// Default Mistral API base URL (without the `/v1` suffix, which the adapter adds).
const BASE_URL: &str = "https://api.mistral.ai";

/// Create a Mistral-configured provider.
///
/// The provider resolves `mistral:model` specs and routes through the
/// OpenAI Chat Completions adapter using standard Bearer token auth.
pub fn mistral_provider(api_key: String, base_url: Option<String>) -> OpenAiChatProvider {
    let base = base_url.unwrap_or_else(|| BASE_URL.into());
    OpenAiChatProvider::new_for_profile(
        api_key,
        base,
        "mistral".into(),
        CompatConfig::default(),
        vec![],
        default_models(),
    )
}

fn default_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "mistral-large-latest".into(),
            display_name: "Mistral Large".into(),
            context_window: 128000,
            max_output_tokens: 8192,
            supports_images: false,
            supports_streaming: true,
            supports_thinking: false,
        },
        ModelInfo {
            id: "mistral-medium-latest".into(),
            display_name: "Mistral Medium".into(),
            context_window: 32000,
            max_output_tokens: 8192,
            supports_images: false,
            supports_streaming: true,
            supports_thinking: false,
        },
        ModelInfo {
            id: "mistral-small-latest".into(),
            display_name: "Mistral Small".into(),
            context_window: 32000,
            max_output_tokens: 8192,
            supports_images: false,
            supports_streaming: true,
            supports_thinking: false,
        },
        ModelInfo {
            id: "codestral-latest".into(),
            display_name: "Codestral".into(),
            context_window: 256000,
            max_output_tokens: 8192,
            supports_images: false,
            supports_streaming: true,
            supports_thinking: false,
        },
    ]
}
