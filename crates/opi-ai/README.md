# opi-ai

[![Crates.io](https://img.shields.io/crates/v/opi-ai.svg)](https://crates.io/crates/opi-ai)
[![Docs.rs](https://docs.rs/opi-ai/badge.svg)](https://docs.rs/opi-ai)

> Provider-neutral LLM API for [opi](https://github.com/OdradekAI/opi), with streaming events, text/image content, tool-call message types, provider/model registration, retry helpers, shared HTTP client support, usage accumulation, and cost calculation.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.5.0`.

`opi-ai` exposes the common `Provider` trait plus provider-neutral request, message, model, and stream event types. The crate contains real streaming implementations for Anthropic, OpenAI Chat Completions, OpenAI Responses, Gemini, AWS Bedrock Converse, Azure OpenAI, and Google Vertex AI, plus OpenAI-compatible profiles for OpenRouter and Mistral. `ProviderRegistry` resolves `provider:model` specs, accepts custom providers, and supports model overrides for deployments or fine-tuned models.

## Providers

| Module | Provider id | API style |
|--------|-------------|-----------|
| `anthropic` | `anthropic` | Anthropic Messages SSE |
| `openai_chat` | `openai` | OpenAI Chat Completions SSE |
| `openai_responses` | `openai-responses` | OpenAI Responses SSE |
| `openrouter` | `openrouter` | OpenAI-compatible OpenRouter profile |
| `mistral` | `mistral` | OpenAI-compatible Mistral profile |
| `gemini` | `gemini` | Gemini `streamGenerateContent?alt=sse` |
| `bedrock` | `bedrock` | AWS Bedrock Converse streaming with SigV4 signing |
| `azure_openai` | `azure` | Azure OpenAI deployment-specific Chat Completions |
| `vertex` | `vertex` | Vertex AI Gemini streaming |

Each provider maps native wire events into `AssistantStreamEvent`, including text deltas, thinking deltas when available, tool-call deltas, terminal completion, usage, and errors.

## Core API

- `Provider`: backend trait with `id()`, `models()`, and `stream(Request) -> EventStream`.
- `Request`: model, system prompt, messages, tools, token limits, temperature, thinking config, stop sequences, metadata, and cancellation token.
- `Message`: provider-facing user, assistant, and tool-result messages.
- `InputContent` / `OutputContent`: text and image content with URL, base64, or raw-byte image sources.
- `AssistantStreamEvent`: provider-neutral stream variants for start/text/thinking/tool/done/error.
- `ModelInfo`: model descriptor with context window, max output tokens, image support, streaming support, and thinking support.
- `registry::ProviderRegistry`: resolves `provider:model` specs, registers custom providers, layers model overrides, lists all models, and exposes model capabilities.
- `http::HttpClient`: shared `reqwest` client wrapper with connection pooling and explicit or environment-derived proxy support.
- `retry`: retry config, exponential backoff, and `Retry-After` parsing.
- `Usage`, `CumulativeUsage`, `Pricing`, `CostBreakdown`, `calculate_cost`: token and cost helpers.
- `test_support::MockProvider`: builder-style mock provider used by downstream tests.

## Image Support

Image input is represented by `InputContent::Image` with media types `image/png`, `image/jpeg`, `image/gif`, and `image/webp`. Providers serialize this content to their native wire formats where supported. `validate_request_capabilities` rejects known text-only models before a network call.

Bedrock supports byte/base64 image sources through Converse, but rejects URL-sourced images locally because the Bedrock Converse API expects image bytes.

## Usage

```rust
use futures_util::StreamExt;
use opi_ai::anthropic::AnthropicProvider;
use opi_ai::message::{InputContent, Message, UserMessage};
use opi_ai::provider::{Provider, Request, ThinkingConfig};
use tokio_util::sync::CancellationToken;

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let provider = AnthropicProvider::new(
    std::env::var("ANTHROPIC_API_KEY")?,
    None,
);

let request = Request {
    model: "claude-sonnet-4-5-20250514".into(),
    system: Some("You are concise.".into()),
    messages: vec![Message::User(UserMessage {
        content: vec![InputContent::Text { text: "Hi".into() }],
        timestamp_ms: 0,
    })],
    tools: vec![],
    max_tokens: Some(1024),
    temperature: None,
    thinking: ThinkingConfig::default(),
    stop_sequences: vec![],
    metadata: None,
    cancel: CancellationToken::new(),
};

let mut stream = provider.stream(request);
while let Some(event) = stream.next().await {
    println!("{:?}", event?);
}
# Ok(()) }
```

## Modules

| Module | Purpose |
|--------|---------|
| `provider` | `Provider`, `Request`, `EventStream`, `ModelInfo`, provider errors, capability validation |
| `message` | Provider-facing messages, tool-call content, and image content |
| `stream` | Stream events, stop reasons, usage, cumulative usage, pricing helpers |
| `registry` | `provider:model` resolution and capability lookup |
| `http` | Shared HTTP client, connection pooling, proxy config, proxy env discovery |
| `retry` | Retry/backoff/rate-limit helpers |
| `model` | Lightweight `Model` descriptor |
| `anthropic` | Anthropic Messages provider and SSE mapper |
| `openai_chat` | OpenAI-compatible Chat Completions provider and compatibility profile adapter |
| `openai_responses` | OpenAI Responses provider |
| `openrouter` | OpenRouter provider profile |
| `mistral` | Mistral provider profile |
| `gemini` | Gemini provider |
| `bedrock` | AWS Bedrock Converse provider, event-stream parser, SigV4 signing, credential resolution |
| `azure_openai` | Azure OpenAI deployment provider |
| `vertex` | Google Vertex AI Gemini provider |
| `config` | Shared config error type |
| `test_support` | Hidden mock provider for tests |

## License

MIT. See the workspace [LICENSE](../../LICENSE).
