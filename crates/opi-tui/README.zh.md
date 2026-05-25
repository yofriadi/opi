# opi-tui

[![Crates.io](https://img.shields.io/crates/v/opi-tui.svg)](https://crates.io/crates/opi-tui)
[![Docs.rs](https://docs.rs/opi-tui/badge.svg)](https://docs.rs/opi-tui)

> [opi](https://github.com/OdradekAI/opi) 交互式编程 Agent 使用的 ratatui 终端 UI 组件库。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.3.0`。

`opi-tui` 是同步 widget library。事件循环和异步 runtime 由调用方持有。本 crate 提供 `opi-coding-agent` 使用的对话记录、编辑器、状态栏、Markdown、工具调用、diff、主题和按键绑定基础组件。

## 组件与 UI 基础类型

| 项 | 作用 |
|----|------|
| `Shell` | 顶层布局，组合对话记录、状态栏、编辑器和可选工具调用视图 |
| `MessageList` | 可滚动对话记录，按角色着色 |
| `InputEditor` | 多行输入缓冲区，带光标和编辑辅助 |
| `StatusBar` | 应用状态、模型、token/费用状态和实时活动 |
| `ToolCallView` | 展示工具名、参数和状态的工具调用行 |
| `MarkdownView` / `CodeBlock` | Markdown 渲染和 fenced code block 展示 |
| `DiffView` | 为文件编辑 before/after 渲染 unified diff |
| `Theme` / `resolve_theme` | 语义调色板；内置 `default` 与 `monokai` |
| `Keybindings` / `KeyCombo` | 可配置语义动作：submit、abort、new line |

## 公共类型

```rust
pub enum Role { User, Assistant, System, Tool }

pub struct Message {
    pub role: Role,
    pub content: String,
    pub diff: Option<DiffPayload>,
}

pub struct DiffPayload {
    pub path: String,
    pub before: String,
    pub after: String,
}

pub enum AppState { Idle, Thinking, Streaming, ToolExecuting }
pub enum ToolCallStatus { Running, Success, Error(String) }
pub enum TuiError { Terminal(String), Render(String) }
```

`Message::new(role, content)` 构造普通对话消息。`Message::diff(path, before, after)` 构造通过 `DiffView` 渲染的 tool-role 消息。

## 按键绑定

默认绑定：

| 动作 | 默认值 |
|------|--------|
| submit | `enter` |
| abort | `escape` |
| new line | `alt+enter` |

`KeyCombo` 解析小写字符串，例如 `enter`、`escape`、`ctrl+c`、`alt+enter`、`shift+tab`。在 `opi` 二进制中，非法配置会回退到默认绑定。

## 主题

`Theme` 为消息角色、状态栏、编辑器、Markdown/code、diff view 和工具状态提供语义颜色字段。`resolve_theme(name)` 目前识别：

- `default`
- `monokai`

未知名称会解析为 `default`。

## 集成方式

`opi` 二进制在 `crates/opi-coding-agent/src/interactive.rs` 中使用本 crate：

1. 调用方维护应用状态。
2. 从 `opi_agent::AgentEvent` 回调更新状态。
3. 解析 `Theme` 和 `Keybindings`。
4. 每帧构建一个 `Shell`，并通过 ratatui 渲染。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
