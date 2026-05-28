//! OpenRouter provider profile  - routes through the OpenAI-compatible adapter.
//!
//! OpenRouter (<https://openrouter.ai>) provides an OpenAI-compatible API that
//! routes requests to many model providers. This module creates a pre-configured
//! [`OpenAiChatProvider`] with OpenRouter's base URL, identification headers, and
//! a curated model list.

use crate::openai_chat::{CompatConfig, OpenAiChatProvider};
use crate::provider::ModelInfo;

/// Default OpenRouter API base URL (without the `/v1` suffix, which the adapter adds).
const BASE_URL: &str = "https://openrouter.ai/api";

/// Create an OpenRouter-configured provider.
///
/// The provider resolves `openrouter:model` specs, routes through the
/// OpenAI Chat Completions adapter, and sends `HTTP-Referer` and `X-Title`
/// headers for app identification on the OpenRouter platform.
pub fn openrouter_provider(api_key: String, base_url: Option<String>) -> OpenAiChatProvider {
    let base = base_url.unwrap_or_else(|| BASE_URL.into());
    OpenAiChatProvider::new_for_profile(
        api_key,
        base,
        "openrouter".into(),
        CompatConfig::default(),
        vec![
            (
                "HTTP-Referer".into(),
                "https://github.com/OdradekAI/opi".into(),
            ),
            ("X-Title".into(), "opi".into()),
        ],
        default_models(),
    )
}

fn default_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "anthropic/claude-sonnet-4".into(),
            display_name: "Claude Sonnet 4 (via OpenRouter)".into(),
            context_window: 200000,
            max_output_tokens: 64000,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: false,
        },
        ModelInfo {
            id: "anthropic/claude-haiku-4".into(),
            display_name: "Claude Haiku 4 (via OpenRouter)".into(),
            context_window: 200000,
            max_output_tokens: 8192,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: false,
        },
        ModelInfo {
            id: "openai/gpt-4o".into(),
            display_name: "GPT-4o (via OpenRouter)".into(),
            context_window: 128000,
            max_output_tokens: 16384,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: false,
        },
        ModelInfo {
            id: "openai/gpt-4o-mini".into(),
            display_name: "GPT-4o Mini (via OpenRouter)".into(),
            context_window: 128000,
            max_output_tokens: 16384,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: false,
        },
        ModelInfo {
            id: "google/gemini-2.5-pro".into(),
            display_name: "Gemini 2.5 Pro (via OpenRouter)".into(),
            context_window: 1048576,
            max_output_tokens: 65536,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: false,
        },
        ModelInfo {
            id: "deepseek/deepseek-r1".into(),
            display_name: "DeepSeek R1 (via OpenRouter)".into(),
            context_window: 131072,
            max_output_tokens: 32768,
            supports_images: false,
            supports_streaming: true,
            supports_thinking: false,
        },
    ]
}
