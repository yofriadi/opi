# opi-ai

[![Crates.io](https://img.shields.io/crates/v/opi-ai.svg)](https://crates.io/crates/opi-ai)
[![Docs.rs](https://docs.rs/opi-ai/badge.svg)](https://docs.rs/opi-ai)

> [opi](https://github.com/OdradekAI/opi) 的 Provider 无关 LLM API，包含流式事件、工具调用消息类型、retry 辅助、用量累计与费用计算。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.3.0`。

`opi-ai` 暴露统一的 `Provider` trait，以及 Provider 无关的消息和事件模型。当前包含 Anthropic、OpenAI Chat Completions、OpenAI Responses、Gemini 的真实流式实现，并通过 OpenAI-compatible adapter 支持 OpenRouter 与 Mistral profile。

## Provider

| 模块 | Provider id | API 形式 |
|------|-------------|----------|
| `anthropic` | `anthropic` | Anthropic Messages SSE |
| `openai_chat` | `openai` | OpenAI Chat Completions SSE |
| `openai_responses` | `openai-responses` | OpenAI Responses SSE |
| `openrouter` | `openrouter` | OpenAI-compatible OpenRouter profile |
| `mistral` | `mistral` | OpenAI-compatible Mistral profile |
| `gemini` | `gemini` | Gemini `streamGenerateContent?alt=sse` |

每个 Provider 都把原生 wire event 映射为 `AssistantStreamEvent`，覆盖文本增量、可用时的 thinking 增量、工具调用增量、终止完成与错误。

## 核心 API

- `Provider`：后端 trait，提供 `id()`、`models()`、`stream(Request) -> EventStream`。
- `Request`：模型、系统提示词、消息、工具、token 限制、temperature、thinking 配置、metadata、取消 token。
- `AssistantStreamEvent`：12 种 Provider 无关流式事件，覆盖 start/text/thinking/tool/done/error。
- `message`：`Message`、`AssistantMessage`、`UserMessage`、`ToolResultMessage`、`ToolDef`、`ToolCall` 与内容变体。
- `registry::ProviderRegistry`：解析 `provider:model` spec，并暴露模型能力查询。
- `retry`：retry 配置、指数退避和 retry-after header 解析。
- `Usage`、`CumulativeUsage`、`Pricing`、`CostBreakdown`、`calculate_cost`：token 与费用辅助工具。
- `test_support::MockProvider`：供下游测试复用的 builder 风格 mock provider。

## 使用示例

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
| `provider` | `Provider`、`Request`、`EventStream`、`ModelInfo`、`ProviderError`、`ProviderKind` |
| `message` | 面向 Provider 的消息与工具调用内容 |
| `stream` | 流式事件、停止原因、用量、累计用量、费用辅助工具 |
| `registry` | `provider:model` 解析与能力查询 |
| `retry` | retry/backoff/rate-limit 辅助 |
| `anthropic` | Anthropic Messages Provider 与 SSE mapper |
| `openai_chat` | OpenAI-compatible Chat Completions Provider 与兼容 profile adapter |
| `openai_responses` | OpenAI Responses Provider |
| `openrouter` | OpenRouter Provider profile |
| `mistral` | Mistral Provider profile |
| `gemini` | Gemini Provider |
| `config` | 共享配置错误类型 |
| `test_support` | 隐藏的测试 Mock Provider |

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
