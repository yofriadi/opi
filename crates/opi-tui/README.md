# opi-tui

[![Crates.io](https://img.shields.io/crates/v/opi-tui.svg)](https://crates.io/crates/opi-tui)
[![Docs.rs](https://docs.rs/opi-tui/badge.svg)](https://docs.rs/opi-tui)

> Terminal UI widgets used by [opi](https://github.com/OdradekAI/opi)'s interactive coding agent. A Rust port of [pi](https://github.com/earendil-works/pi)'s TUI library.

[简体中文](README.zh.md) · [← opi](../../README.md)

---

## Status (v0.2.0)

Phase 1 widgets are functional and used by the `opi` binary's interactive
mode. Built on [`ratatui`](https://crates.io/crates/ratatui) and
[`crossterm`](https://crates.io/crates/crossterm). No async runtime is
required at the library level — `opi-tui` is purely a synchronous widget
toolkit; the consuming application owns the event loop and tokio runtime.

## Widgets

| Widget | Purpose |
|--------|---------|
| `Shell` | Top-level layout composing the message list, status bar, and input editor |
| `MessageList` | Scrollable conversation transcript with role styling |
| `InputEditor` | Multi-line text input with cursor + insertion helpers |
| `StatusBar` | App state, model id, and live status (`idle`, `thinking…`, `streaming…`, `executing tool…`) |
| `ToolCallView` | Per-tool-call line showing name, args, and `ToolCallStatus` |
| `MarkdownView` / `CodeBlock` | Markdown rendering with fenced code-block highlighting |

## Public types

```rust
pub enum Role  { User, Assistant, System, Tool }
pub struct Message { pub role: Role, pub content: String }

pub enum AppState  { Idle, Thinking, Streaming, ToolExecuting }
pub enum ToolCallStatus { Running, Success, Error(String) }

pub enum TuiError { Terminal(String), Render(String) }
```

`Message::new(role, content)` and a `Display` impl on `AppState` /
`ToolCallStatus` keep the call sites short.

## Integration shape

The `opi` binary uses `opi-tui` like this (see
[`crates/opi-coding-agent/src/interactive.rs`](../opi-coding-agent/src/interactive.rs)):

1. Build a `Shell` per frame, passing the current `MessageList`,
   `InputEditor`, `StatusBar`, and (optional) `ToolCallView`.
2. Drive the render loop at ~20 FPS from your tokio task.
3. Update a shared `TuiState` from `AgentEvent` callbacks emitted by
   `opi-agent`.

## License

MIT — see workspace [`LICENSE`](../../LICENSE).
