# opi-ai

[![Crates.io](https://img.shields.io/crates/v/opi-ai.svg)](https://crates.io/crates/opi-ai)
[![Docs.rs](https://docs.rs/opi-ai/badge.svg)](https://docs.rs/opi-ai)

> [opi](https://github.com/OdradekAI/opi) 的 Provider 抽象层 —— [pi](https://github.com/earendil-works/pi) 的 Rust 移植。

[English](README.md) · [← opi](../../README.zh.md)

---

## 当前状态（v0.2.0）

Phase 1 已实现完整的 **Anthropic Messages** 流式管线。`Provider` trait、注册中心、消息类型、12 种 `AssistantStreamEvent` 在设计上预留了多 provider 支持，但本版本只接入了 Anthropic 一家。OpenAI、Google、Mistral、Bedrock、Azure 已经在 `ProviderKind` 枚举中预留位置，将在后续 Phase 接入。

## 已实现的能力

- **`Provider` trait** —— `stream(Request) -> EventStream`，通过 `tokio_util::sync::CancellationToken` 取消请求。
- **`anthropic`** —— Anthropic Messages SSE provider；手写 SSE 解析器，对格式异常事件会显式上报（而非静默丢弃），并兼容 CRLF 换行。
- **`registry::ProviderRegistry`** —— 把 `provider:model` 解析成 `Provider` + `ModelInfo`，并提供能力查询（`context_window`、`max_output_tokens`、`supports_streaming`、`supports_thinking`）。
- **`message`** —— `Message`、`AssistantMessage`、`UserMessage`、`ToolResultMessage`、`ToolDef`、`ToolCall` 等核心类型。
- **`stream::AssistantStreamEvent`** —— 12 种变体（`Start`、`Text*`、`Thinking*`、`ToolCall*`、`Done`、`Error`）；token 用量随 `Done` 中的 `AssistantMessage` 返回，没有独立的 usage 事件。
- **`test_support::MockProvider`** —— Builder 风格的测试 Mock，被 `opi-agent`、`opi-coding-agent` 的集成测试复用。

## 使用示例

```rust
use opi_ai::anthropic::AnthropicProvider;
use opi_ai::message::{InputContent, Message, UserMessage};
use opi_ai::provider::{Provider, Request, ThinkingConfig};
use tokio_util::sync::CancellationToken;
use futures_util::StreamExt;

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let provider = AnthropicProvider::new(
    std::env::var("ANTHROPIC_API_KEY")?,
    None, // 使用默认 base URL
);

let request = Request {
    model: "claude-sonnet-4".into(),
    system: Some("回答要简洁。".into()),
    messages: vec![Message::User(UserMessage {
        content: vec![InputContent::Text { text: "你好".into() }],
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

## 模块速查

| 模块 | 作用 |
|------|------|
| `provider` | `Provider` trait、`Request`、`EventStream`、`ModelInfo`、`ProviderError`、`ProviderKind` |
| `anthropic` | Anthropic SSE provider、SSE 解析器、事件映射器 |
| `registry` | `provider:model` 解析与能力查询 |
| `message` | LLM 消息与工具调用类型 |
| `stream` | `AssistantStreamEvent`、`StopReason`、`Usage` |
| `model` | `Model` 重导出 |
| `config` | `Config`、`Error`（下游 crate 共享） |
| `test_support` | 测试用 Mock provider（`#[doc(hidden)]`） |

## 许可证

MIT —— 见 workspace 根目录 [`LICENSE`](../../LICENSE)。
