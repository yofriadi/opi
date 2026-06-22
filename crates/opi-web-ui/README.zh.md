# opi-web-ui

> [opi](https://github.com/OdradekAI/opi) agent 工具包中未发布的 Web 侧状态与组件层。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本是 `0.5.2`，继承自 workspace 包版本。

`opi-web-ui` 设置了 `publish = false`。它不是独立浏览器应用，也不会启动服务器。它
为嵌入方控制的 Web 表面提供类型化 Rust 状态和 HTML 组件模型，用于消费 `opi`
RPC/SDK JSON 事件。

所有公开类型都是不稳定的 0.x API。请固定精确版本，并在升级时测试。

## 范围

| 模块 | 作用 |
|------|------|
| `event` | 把原始 RPC JSON 值解析为 `WebUiEvent` 变体，并保留未知事件类型。 |
| `state` | 根据事件维护对话状态：消息、工具调用、thinking blocks、会话 metadata、资源 metadata、压缩状态和最近 RPC 响应。 |
| `components` | 类型化组件模型：`ChatMessage`、`ToolCallView`、`ThinkingBlock`、`StatusBar` 和 `ConversationView`。 |
| `render` | `Render` trait，以及用于 XSS 安全字符串输出的 HTML 转义。 |

本 crate 有意保持 JSON 形状的运行时边界。它运行时不依赖 `opi-ai` 或 `opi-agent`；
`opi-agent` 只作为测试 dev-dependency 使用。

## 用法

```rust
use opi_web_ui::event::WebUiEvent;
use opi_web_ui::render::Render;
use opi_web_ui::state::ConversationState;

let mut state = ConversationState::new();

// 解析原始 RPC/SDK 事件。
let raw = serde_json::json!({"type": "AgentStart"});
let event = WebUiEvent::parse(&raw).unwrap();
state.process(event);

// 调用方已经拥有类型化事件时，也可以直接处理。
state.process(WebUiEvent::MessageStart {
    model: "claude-sonnet-4-5".to_owned(),
    provider: "anthropic".to_owned(),
});
state.process(WebUiEvent::TextDelta {
    index: 0,
    delta: "你好".to_owned(),
});
state.process(WebUiEvent::MessageEnd);

let html = state.to_conversation_view().render_html();
let status = state.to_status_bar().render_html();
```

`ConversationState` 为消息、工具调用、thinking blocks、模型、会话 id、turn/message
计数、Agent 运行状态、压缩状态、最近 RPC 响应、资源 metadata 和最近一次成功压缩
payload 提供只读访问器。

## 边界

只在需要可复用 Web 侧状态或 HTML 组件模型时使用本 crate。终端 UI 属于
`opi-tui`，CLI/harness 行为属于 `opi-coding-agent`，Provider 类型属于 `opi-ai`，
运行时主循环基础能力属于 `opi-agent`。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
