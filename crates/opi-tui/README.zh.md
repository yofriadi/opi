# opi-tui

[![Crates.io](https://img.shields.io/crates/v/opi-tui.svg)](https://crates.io/crates/opi-tui)
[![Docs.rs](https://docs.rs/opi-tui/badge.svg)](https://docs.rs/opi-tui)

> [opi](https://github.com/OdradekAI/opi) 交互式编程 Agent 使用的 ratatui 终端 UI 组件库。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.5.1`。

`opi-tui` 是同步 widget library。事件循环和异步 runtime 由调用方持有。本 crate 提供 `opi-coding-agent` 使用的对话记录、编辑器、状态栏、Markdown、工具调用、diff、选择列表、分支选择器、终端图片、主题和按键绑定基础组件。

## 组件与 UI 基础类型

| 项 | 作用 |
|----|------|
| `Shell` | 顶层布局，组合对话记录、状态栏、编辑器和可选工具调用视图 |
| `MessageList` | 可滚动对话记录，支持按角色着色、diff 与图片 payload |
| `InputEditor` | 多行输入缓冲区，带光标和编辑辅助 |
| `StatusBar` | 应用状态、模型、token/费用状态和实时活动 |
| `ToolCallView` | 展示工具名、参数和状态的工具调用行 |
| `MarkdownView` / `CodeBlock` | Markdown 渲染和 fenced code block 展示 |
| `DiffView` | 为文件编辑 before/after 渲染 unified diff |
| `SelectList` / `SelectListState` | 模型与会话选择器使用的 fuzzy-select 列表 |
| `BranchPicker` / `BranchPickerState` | 会话分支选择器，支持活跃分支标记和按 Unicode 宽度处理行 |
| `terminal_image` | Kitty/iTerm2/Sixel escape 生成与文本 fallback |
| `Theme` / `resolve_theme` | 语义调色板；内置 `default` 与 `monokai` |
| `Keybindings` / `KeyCombo` | 可配置语义动作：submit、abort、new line |

## 公共类型

```rust
pub enum Role { User, Assistant, System, Tool }

pub struct Message {
    pub role: Role,
    pub content: String,
    pub diff: Option<DiffPayload>,
    pub image: Option<ImagePayload>,
}

pub struct DiffPayload {
    pub path: String,
    pub before: String,
    pub after: String,
}

pub struct ImagePayload {
    pub data: ImageData,
    pub protocol: TerminalGraphicsProtocol,
}

pub enum AppState { Idle, Thinking, Streaming, ToolExecuting }
pub enum ToolCallStatus { Running, Success, Error(String) }
pub enum TuiError { Terminal(String), Render(String) }

pub struct BranchItem {
    pub tip_id: String,
    pub label: String,
    pub metadata: String,
    pub is_active: bool,
}
```

`Message::new(role, content)` 构造普通对话消息。`Message::diff(path, before, after)` 构造通过 `DiffView` 渲染的 tool-role 消息。`Message::image(role, payload)` 构造图片消息，并通过终端图形 escape sequence 或文本 fallback 渲染。

## 终端图片

`terminal_image` 暴露：

- `TerminalGraphicsProtocol::{Kitty, Iterm2, Sixel, Fallback}`。
- 基于终端环境线索的 `detect_graphics_protocol`。
- `kitty_escape`、`iterm_escape`、`sixel_escape` 和 `text_fallback`。
- 带 PNG、JPEG、GIF 或 WebP 元数据的 `ImageData`。

当前协议检测会明确识别 Kitty 和 iTerm2，其余情况回退为文本占位符。

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
4. 在需要时构建模型/会话/分支选择器的 `SelectList` overlay。
5. 每帧构建一个 `Shell`，并通过 ratatui 渲染。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
