# opi-ai

[![Crates.io](https://img.shields.io/crates/v/opi-ai.svg)](https://crates.io/crates/opi-ai)
[![Docs.rs](https://docs.rs/opi-ai/badge.svg)](https://docs.rs/opi-ai)

> Provider-neutral LLM API for [opi](https://github.com/OdradekAI/opi), with streaming events, tool-call message types, retry helpers, usage accumulation, and cost calculation.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.3.0`.

`opi-ai` exposes a common `Provider` trait and provider-neutral message/event model. It includes real streaming implementations for Anthropic, OpenAI Chat Completions, OpenAI Responses, Gemini, plus OpenAI-compatible profiles for OpenRouter and Mistral.

## Providers

| Module | Provider id | API style |
|--------|-------------|-----------|
| `anthropic` | `anthropic` | Anthropic Messages SSE |
| `openai_chat` | `openai` | OpenAI Chat Completions SSE |
| `openai_responses` | `openai-responses` | OpenAI Responses SSE |
| `openrouter` | `openrouter` | OpenAI-compatible OpenRouter profile |
| `mistral` | `mistral` | OpenAI-compatible Mistral profile |
| `gemini` | `gemini` | Gemini `streamGenerateContent?alt=sse` |

Each provider maps native wire events into `AssistantStreamEvent`, including text deltas, thinking deltas when available, tool-call deltas, terminal completion, and errors.

## Core API

- `Provider`: backend trait with `id()`, `models()`, and `stream(Request) -> EventStream`.
- `Request`: model, system prompt, messages, tools, token limits, temperature, thinking config, metadata, and cancellation token.
- `AssistantStreamEvent`: 12 provider-neutral stream variants for start/text/thinking/tool/done/error.
- `message`: `Message`, `AssistantMessage`, `UserMessage`, `ToolResultMessage`, `ToolDef`, `ToolCall`, and content variants.
- `registry::ProviderRegistry`: resolves `provider:model` specs and exposes model capabilities.
- `retry`: retry config, exponential backoff, and retry-after header parsing.
- `Usage`, `CumulativeUsage`, `Pricing`, `CostBreakdown`, `calculate_cost`: token and cost helpers.
- `test_support::MockProvider`: builder-style mock provider used by downstream tests.

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
| `provider` | `Provider`, `Request`, `EventStream`, `ModelInfo`, `ProviderError`, `ProviderKind` |
| `message` | Provider-facing messages and tool-call content |
| `stream` | Stream events, stop reasons, usage, cumulative usage, pricing helpers |
| `registry` | `provider:model` resolution and capability lookup |
| `retry` | Retry/backoff/rate-limit helpers |
| `anthropic` | Anthropic Messages provider and SSE mapper |
| `openai_chat` | OpenAI-compatible Chat Completions provider and compatibility profile adapter |
| `openai_responses` | OpenAI Responses provider |
| `openrouter` | OpenRouter provider profile |
| `mistral` | Mistral provider profile |
| `gemini` | Gemini provider |
| `config` | Shared config error type |
| `test_support` | Hidden mock provider for tests |

## License

MIT. See the workspace [LICENSE](../../LICENSE).
