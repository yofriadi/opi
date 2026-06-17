# opi-ai

[![Crates.io](https://img.shields.io/crates/v/opi-ai.svg)](https://crates.io/crates/opi-ai)
[![Docs.rs](https://docs.rs/opi-ai/badge.svg)](https://docs.rs/opi-ai)

> [opi](https://github.com/OdradekAI/opi) 的 Provider 无关 LLM API，包含流式事件、文本/图片内容、工具调用消息类型、Provider/模型注册、retry 辅助、共享 HTTP client、用量累计与费用计算。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.5.2`，继承自 workspace package 版本。

`opi-ai` 暴露统一的 `Provider` trait，以及 Provider 无关的请求、消息、模型和流式事件类型。当前包含 Anthropic、OpenAI Chat Completions、OpenAI Responses、Gemini、AWS Bedrock Converse、Azure OpenAI、Google Vertex AI 的真实流式实现，并通过 OpenAI-compatible adapter 支持 OpenRouter 与 Mistral profile。`ProviderRegistry` 会解析 `provider:model` spec，支持注册自定义 Provider，并支持为 deployment 或 fine-tuned 模型叠加模型覆盖。

## Provider

| 模块 | Provider id | API 形式 |
|------|-------------|----------|
| `anthropic` | `anthropic` | Anthropic Messages SSE |
| `openai_chat` | `openai` | OpenAI Chat Completions SSE |
| `openai_responses` | `openai-responses` | OpenAI Responses SSE |
| `openrouter` | `openrouter` | OpenAI-compatible OpenRouter profile |
| `mistral` | `mistral` | OpenAI-compatible Mistral profile |
| `gemini` | `gemini` | Gemini `streamGenerateContent?alt=sse` |
| `bedrock` | `bedrock` | AWS Bedrock Converse streaming，使用 SigV4 签名 |
| `azure_openai` | `azure` | Azure OpenAI deployment 专用 Chat Completions |
| `vertex` | `vertex` | Vertex AI Gemini streaming |

每个 Provider 都会把原生 wire event 映射为 `AssistantStreamEvent`，覆盖文本增量、可用时的 thinking 增量、工具调用增量、终止完成、用量与错误。内置模型列表有意保持有限，用于能力校验和模型列表输出；deployment、fine-tuned 模型或站点专用模型 ID 应通过 registry 模型覆盖或已配置 Provider profile 提供。

## 核心 API

- `Provider`：后端 trait，提供 `id()`、`models()`、`stream(Request) -> EventStream`。
- `Request`：模型、系统提示词、消息、工具、token 限制、temperature、thinking 配置、stop sequences、metadata 和取消 token。
- `Message`：面向 Provider 的 user、assistant 与 tool-result 消息。
- `InputContent` / `OutputContent`：文本和图片内容，图片来源支持 URL、base64 或原始 bytes。
- `AssistantStreamEvent`：Provider 无关流式事件，覆盖 start/text/thinking/tool/done/error。
- `ModelInfo`：模型描述，包含上下文窗口、最大输出 token、图片支持、流式支持与 thinking 支持。
- `ApiKind`：兼容 adapter 使用的 wire protocol family 标记。
- `registry::ProviderRegistry`：解析 `provider:model` spec，注册自定义 Provider，叠加模型覆盖，列出所有模型，并暴露模型能力查询。
- `http::HttpClient`：共享 `reqwest` client 封装，支持连接池与显式或环境变量代理。
- `retry`：retry 配置、指数退避与 `Retry-After` 解析。
- `Usage`、`CumulativeUsage`、`Pricing`、`CostBreakdown`、`calculate_cost`：token 与费用辅助工具。
- `test_support::MockProvider`：供下游测试复用的 builder 风格 mock provider。

## 图片支持

图片输入使用 `InputContent::Image` 表示，媒体类型支持 `image/png`、`image/jpeg`、`image/gif` 和 `image/webp`。支持图片的 Provider 会把该内容序列化为各自的原生 wire 格式。`validate_request_capabilities` 会在发起网络请求前拒绝已知的纯文本模型。

Bedrock Converse 支持 bytes/base64 图片来源，但会在本地拒绝 URL 图片来源，因为 Bedrock Converse API 需要图片 bytes。

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
| `provider` | `Provider`、`Request`、`EventStream`、`ModelInfo`、Provider 错误、能力校验 |
| `message` | 面向 Provider 的消息、工具调用内容与图片内容 |
| `stream` | 流式事件、停止原因、用量、累计用量、费用辅助工具 |
| `registry` | `provider:model` 解析与能力查询 |
| `http` | 共享 HTTP client、连接池、代理配置、代理环境变量发现 |
| `retry` | retry/backoff/rate-limit 辅助 |
| `model` | 轻量 `Model` 描述符 |
| `anthropic` | Anthropic Messages Provider 与 SSE mapper |
| `openai_chat` | OpenAI-compatible Chat Completions Provider 与兼容 profile adapter |
| `openai_responses` | OpenAI Responses Provider |
| `openrouter` | OpenRouter Provider profile |
| `mistral` | Mistral Provider profile |
| `gemini` | Gemini Provider |
| `bedrock` | AWS Bedrock Converse Provider、event-stream parser、SigV4 签名、凭据解析 |
| `azure_openai` | Azure OpenAI deployment Provider |
| `vertex` | Google Vertex AI Gemini Provider |
| `config` | 共享配置错误类型 |
| `test_support` | 隐藏的测试 Mock Provider |

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
