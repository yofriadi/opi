# opi-tui

[![Crates.io](https://img.shields.io/crates/v/opi-tui.svg)](https://crates.io/crates/opi-tui)
[![Docs.rs](https://docs.rs/opi-tui/badge.svg)](https://docs.rs/opi-tui)

> [opi](https://github.com/OdradekAI/opi) 交互式编码 Agent 所使用的终端 UI 组件库。[pi](https://github.com/earendil-works/pi) TUI 库的 Rust 移植版本。

[English](README.md) · [← opi](../../README.zh.md)

---

## 当前状态（v0.2.0）

Phase 1 的组件已经可用，并被 `opi` 二进制的交互模式直接使用。底层基于 [`ratatui`](https://crates.io/crates/ratatui) 与 [`crossterm`](https://crates.io/crates/crossterm)。crate 自身**不需要异步运行时**，是一套纯同步的组件工具集；事件循环和 tokio runtime 由调用方持有。

## 组件总览

| 组件 | 作用 |
|------|------|
| `Shell` | 顶层布局，组合消息列表、状态栏与输入编辑器 |
| `MessageList` | 可滚动的对话记录，按角色着色 |
| `InputEditor` | 多行文本输入框，自带光标与插入辅助方法 |
| `StatusBar` | 显示应用状态、模型 id 与实时状态（`idle` / `thinking…` / `streaming…` / `executing tool…`） |
| `ToolCallView` | 每个工具调用的单行视图，展示工具名、参数与 `ToolCallStatus` |
| `MarkdownView` / `CodeBlock` | Markdown 渲染，支持围栏代码块的高亮 |

## 公共类型

```rust
pub enum Role  { User, Assistant, System, Tool }
pub struct Message { pub role: Role, pub content: String }

pub enum AppState  { Idle, Thinking, Streaming, ToolExecuting }
pub enum ToolCallStatus { Running, Success, Error(String) }

pub enum TuiError { Terminal(String), Render(String) }
```

`Message::new(role, content)` 与 `AppState` / `ToolCallStatus` 上的 `Display` 实现可以让调用点保持简短。

## 集成形态

`opi` 二进制使用 `opi-tui` 的方式（详见 [`crates/opi-coding-agent/src/interactive.rs`](../opi-coding-agent/src/interactive.rs)）：

1. 每一帧构造一个 `Shell`，传入当前的 `MessageList`、`InputEditor`、`StatusBar`，以及可选的 `ToolCallView`。
2. 在 tokio 任务中以约 20 FPS 的频率推动渲染循环。
3. `opi-agent` 通过 `AgentEvent` 回调更新一个共享的 `TuiState`。

## 许可证

MIT —— 见 workspace 根目录 [`LICENSE`](../../LICENSE)。
