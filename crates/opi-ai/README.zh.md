# opi-ai

[![Crates.io](https://img.shields.io/crates/v/opi-ai.svg)](https://crates.io/crates/opi-ai)
[![Docs.rs](https://docs.rs/opi-ai/badge.svg)](https://docs.rs/opi-ai)

> [opi](https://github.com/OdradekAI/opi) 使用的 Provider 无关 LLM API。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本是 `0.5.2`，继承自 workspace 包版本。

`opi-ai` 负责模型/Provider 层：请求和消息类型、流式事件、模型元数据、Provider
注册、HTTP/代理连接、重试辅助、图片内容、用量累计和尽力而为的费用辅助。它不
实现 Agent 主循环或内置编程工具；这些能力分别位于 `opi-agent` 和
`opi-coding-agent`。

## Provider

| 模块 | Provider id | 后端 |
|------|-------------|------|
| `anthropic` | `anthropic` | Anthropic Messages streaming |
| `openai_chat` | `openai` | OpenAI Chat Completions streaming |
| `openai_responses` | `openai-responses` | OpenAI Responses streaming |
| `openrouter` | `openrouter` | OpenAI-compatible OpenRouter profile |
| `mistral` | `mistral` | OpenAI-compatible Mistral profile |
| `gemini` | `gemini` | Gemini `streamGenerateContent?alt=sse` |
| `bedrock` | `bedrock` | AWS Bedrock Converse streaming，使用 SigV4 签名 |
| `azure_openai` | `azure` | Azure OpenAI deployment 专用 Chat Completions |
| `vertex` | `vertex` | Google Vertex AI Gemini streaming |

内置模型列表刻意保持有限，主要用于能力校验和模型列表输出。站点专用模型、
fine-tuned 模型和 deployment 应通过 registry override 或配置的
OpenAI-compatible profile 加入。

## 核心 API

| 项 | 作用 |
|----|------|
| `Provider` | 后端 trait，包含 `id`、`models` 和 `stream(Request)`。 |
| `Request` | Provider 请求：模型、消息、工具、token 限制、thinking 配置、metadata、取消信号。 |
| `Message` | 面向 Provider 的 user、assistant 和 tool-result 消息。 |
| `InputContent` / `OutputContent` | 文本与图片内容块。 |
| `AssistantStreamEvent` | Provider 无关流式事件，覆盖 start、text、thinking、tool call、done 和 error。 |
| `ModelInfo` | 模型元数据：上下文窗口、输出上限、图片、流式和 thinking 支持。 |
| `ProviderRegistry` | 解析 `provider:model`、注册自定义 Provider、叠加模型覆盖。 |
| `HttpClient` | 共享 `reqwest` client，支持连接池和显式/环境变量代理。 |
| `retry` | 重试配置、指数退避和 `Retry-After` 解析。 |
| `Usage` / `CumulativeUsage` | token 累计和费用辅助。 |
| `test_support::MockProvider` | 供下游测试使用的确定性 mock provider。 |

## 图片支持

图片输入使用 `InputContent::Image` 表示。支持的媒体类型是 PNG、JPEG、GIF 和
WebP。所选模型支持图片时，Provider 会把图片序列化为各自的原生 wire 格式。

`validate_request_capabilities` 会在发起网络请求前拒绝已知纯文本模型。Bedrock
通过 Converse 支持 byte/base64 图片源，但会在本地拒绝 URL 图片，因为 Bedrock
Converse 需要图片 bytes。

## 最小示例

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

## 模块

`provider`、`message`、`stream`、`registry`、`http`、`retry`、`model`、
`anthropic`、`openai_chat`、`openai_responses`、`openrouter`、`mistral`、
`gemini`、`bedrock`、`azure_openai`、`vertex`、`config` 和 `test_support`。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
