# opi-tui

[![Crates.io](https://img.shields.io/crates/v/opi-tui.svg)](https://crates.io/crates/opi-tui)
[![Docs.rs](https://docs.rs/opi-tui/badge.svg)](https://docs.rs/opi-tui)

> Ratatui-based terminal UI widgets used by [opi](https://github.com/OdradekAI/opi)'s interactive coding agent.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.3.0`.

`opi-tui` is a synchronous widget library. The application owns the event loop and async runtime. The crate provides transcript, editor, status, markdown, tool-call, diff, select-list, terminal-image, theme, and keybinding primitives used by `opi-coding-agent`.

## Widgets and UI Primitives

| Item | Purpose |
|------|---------|
| `Shell` | Top-level layout composing transcript, status bar, editor, and optional tool call view |
| `MessageList` | Scrollable conversation transcript with role styling, diffs, and image payloads |
| `InputEditor` | Multi-line input buffer with cursor/edit helpers |
| `StatusBar` | App state, model, token/cost status, and live activity |
| `ToolCallView` | Tool-call line with name, args, and status |
| `MarkdownView` / `CodeBlock` | Markdown rendering and fenced code-block presentation |
| `DiffView` | Unified diff rendering for before/after file edits |
| `SelectList` / `SelectListState` | Fuzzy-select list used by model and session pickers |
| `terminal_image` | Kitty/iTerm2/Sixel escape generation plus text fallback |
| `Theme` / `resolve_theme` | Semantic palettes; built-in `default` and `monokai` |
| `Keybindings` / `KeyCombo` | Configurable semantic actions: submit, abort, new line |

## Public Types

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
```

`Message::new(role, content)` builds normal transcript messages. `Message::diff(path, before, after)` builds a tool-role message rendered through `DiffView`. `Message::image(role, payload)` builds an image-only message rendered through terminal graphics escape sequences or text fallback.

## Terminal Images

`terminal_image` exposes:

- `TerminalGraphicsProtocol::{Kitty, Iterm2, Sixel, Fallback}`.
- `detect_graphics_protocol` from terminal environment hints.
- `kitty_escape`, `iterm_escape`, `sixel_escape`, and `text_fallback`.
- `ImageData` with PNG, JPEG, GIF, or WebP metadata.

Current protocol detection recognizes Kitty and iTerm2 explicitly and otherwise falls back to text placeholders.

## Keybindings

Default bindings:

| Action | Default |
|--------|---------|
| submit | `enter` |
| abort | `escape` |
| new line | `alt+enter` |

`KeyCombo` parses lowercase strings such as `enter`, `escape`, `ctrl+c`, `alt+enter`, and `shift+tab`. Invalid config falls back to defaults in the `opi` binary.

## Themes

`Theme` exposes semantic color fields for message roles, status bar, editor, markdown/code, diff view, and tool status. `resolve_theme(name)` currently recognizes:

- `default`
- `monokai`

Unknown names resolve to `default`.

## Integration Shape

The `opi` binary uses this crate from `crates/opi-coding-agent/src/interactive.rs`:

1. Keep application state in the caller.
2. Update that state from `opi_agent::AgentEvent` callbacks.
3. Resolve a `Theme` and `Keybindings`.
4. Build `SelectList` overlays for model/session selection when requested.
5. Build a `Shell` each frame and render it with ratatui.

## License

MIT. See the workspace [LICENSE](../../LICENSE).
