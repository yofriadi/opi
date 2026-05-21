# opi-ai

[![Crates.io](https://img.shields.io/crates/v/opi-ai.svg)](https://crates.io/crates/opi-ai)
[![Docs.rs](https://docs.rs/opi-ai/badge.svg)](https://docs.rs/opi-ai)

> Provider abstraction with streaming support for [opi](https://github.com/OdradekAI/opi) — a Rust port of [pi](https://github.com/earendil-works/pi).

[简体中文](README.zh.md) · [← opi](../../README.md)

---

## Status (v0.2.0)

Phase 1 ships a complete streaming pipeline for **Anthropic Messages**. The
`Provider` trait, registry, message types, and 12-variant
`AssistantStreamEvent` model are designed to support multiple providers, but
only Anthropic is wired up in this release. OpenAI, Google, Mistral, Bedrock,
and Azure providers are reserved on the `ProviderKind` enum and will follow in
later phases.

## What's in the box

- **`Provider` trait** — `stream(Request) -> EventStream` with cancellation via
  `tokio_util::sync::CancellationToken`.
- **`anthropic`** — Anthropic Messages SSE provider with a hand-written SSE
  parser that surfaces malformed events (instead of silently dropping them) and
  handles CRLF line endings.
- **`registry::ProviderRegistry`** — resolves `provider:model` specs to a
  `Provider` + `ModelInfo`, plus capability queries (`context_window`,
  `max_output_tokens`, `supports_streaming`, `supports_thinking`).
- **`message`** — `Message`, `AssistantMessage`, `UserMessage`,
  `ToolResultMessage`, `ToolDef`, `ToolCall`, content variants.
- **`stream::AssistantStreamEvent`** — 12 variants (`Start`, `Text*`, `Thinking*`,
  `ToolCall*`, `Done`, `Error`); token usage is carried on `Done` via
  `AssistantMessage`, not as a separate stream event.
- **`test_support::MockProvider`** — builder-style mock for integration tests
  (used by `opi-agent` and `opi-coding-agent`).

## Usage

```rust
use opi_ai::anthropic::AnthropicProvider;
use opi_ai::message::{InputContent, Message, UserMessage};
use opi_ai::provider::{Provider, Request, ThinkingConfig};
use tokio_util::sync::CancellationToken;
use futures_util::StreamExt;

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let provider = AnthropicProvider::new(
    std::env::var("ANTHROPIC_API_KEY")?,
    None, // default base URL
);

let request = Request {
    model: "claude-sonnet-4".into(),
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
| `provider` | `Provider` trait, `Request`, `EventStream`, `ModelInfo`, `ProviderError`, `ProviderKind` |
| `anthropic` | Anthropic SSE provider, SSE parser, event mapper |
| `registry` | `provider:model` spec resolution and capability lookup |
| `message` | LLM message types and tool-call types |
| `stream` | `AssistantStreamEvent`, `StopReason`, `Usage` |
| `model` | `Model` re-export |
| `config` | `Config` and `Error` (shared with downstream crates) |
| `test_support` | Mock provider for tests (`#[doc(hidden)]`) |

## License

MIT — see workspace [`LICENSE`](../../LICENSE).
