# opi-web-ui

> [opi](https://github.com/OdradekAI/opi) agent 工具包的可嵌入 Web UI 组件层。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.4.0`。

`opi-web-ui` 为 `publish = false`，提供具体的组件层，消费 opi agent 工具包的 RPC/SDK 事件并将其渲染为类型化的 Rust 状态和 HTML 组件。后续发布决策可能改变其发布状态。

## 架构

- **`event`** — 将 RPC JSONL 协议的原始 JSON 值解析为类型化的 `WebUiEvent` 变体。
- **`state`** — `ConversationState` 处理事件并维护消息历史、工具调用状态、思考块、会话元数据和压缩状态。
- **`components`** — 类型化 UI 组件模型：`ChatMessage`、`ToolCallView`、`ThinkingBlock`、`StatusBar`、`ConversationView`。
- **`render`** — `Render` trait，用于 HTML 输出，支持 XSS 安全转义。

## 不稳定的 0.x API

所有类型均可能在版本间变更。请固定精确版本并在升级时进行测试。

## 用法

```rust
use opi_web_ui::event::WebUiEvent;
use opi_web_ui::state::ConversationState;
use opi_web_ui::render::Render;

let mut state = ConversationState::new();

// 解析 RPC JSONL 事件
let raw = serde_json::json!({"type": "AgentStart"});
let event = WebUiEvent::parse(&raw).unwrap();
state.process(event);

// 流式文本
state.process(WebUiEvent::MessageStart {
    model: "claude-sonnet-4-5".to_owned(),
    provider: "anthropic".to_owned(),
});
state.process(WebUiEvent::TextDelta { index: 0, delta: "你好".to_owned() });
state.process(WebUiEvent::MessageEnd);

// 渲染为 HTML
let view = state.to_conversation_view();
let html = view.render_html();
```

## 依赖

- `opi-ai` — Provider 无关的流事件和消息类型
- `opi-agent` — SDK 命令/响应类型和 agent 事件
- `serde`、`serde_json` — JSON 序列化
- `thiserror` — 错误类型

## 边界说明

只有实现面向 Web 的可复用 UI 组件后，相关功能才应进入这里。终端编程 Agent 位于 `opi-coding-agent`；Provider 和消息类型位于 `opi-ai`；Agent 运行时基础能力位于 `opi-agent`。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
