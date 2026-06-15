# opi-web-ui

> Embeddable web UI component layer for the [opi](https://github.com/OdradekAI/opi) agent toolkit.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.5.0`.

`opi-web-ui` is `publish = false` and provides a concrete component layer that consumes RPC/SDK JSON event values from the opi agent toolkit and renders them as typed Rust state and HTML components. It is not a standalone browser app. A separate release decision may change the publish status in the future.

## Architecture

- **`event`** — Parses raw JSON values from the RPC JSONL protocol into typed `WebUiEvent` variants, preserving unknown event types for forward-compatible handling.
- **`state`** — `ConversationState` processes events and maintains message history, RPC responses, tool call state, thinking blocks, session metadata, and compaction status.
- **`components`** — Typed UI component models: `ChatMessage`, `ToolCallView`, `ThinkingBlock`, `StatusBar`, `ConversationView`.
- **`render`** — `Render` trait for HTML output with XSS-safe escaping.

## Unstable 0.x API

All types are subject to change between versions. Pin an exact version and test against upgrades.

## Usage

```rust
use opi_web_ui::event::WebUiEvent;
use opi_web_ui::state::ConversationState;
use opi_web_ui::render::Render;

let mut state = ConversationState::new();

// Parse RPC JSONL events
let raw = serde_json::json!({"type": "AgentStart"});
let event = WebUiEvent::parse(&raw).unwrap();
state.process(event);

// Stream text
state.process(WebUiEvent::MessageStart {
    model: "claude-sonnet-4-5".to_owned(),
    provider: "anthropic".to_owned(),
});
state.process(WebUiEvent::TextDelta { index: 0, delta: "Hello".to_owned() });
state.process(WebUiEvent::MessageEnd);

// Render to HTML
let view = state.to_conversation_view();
let html = view.render_html();
```

## Dependencies

- `serde`, `serde_json` — JSON serialization
- `thiserror` — error types

`opi-agent` is used only as a dev-dependency for tests. The runtime crate boundary is intentionally JSON-shaped so web-facing code does not need to depend on provider or agent internals.

## Boundary

Future work belongs here only when it implements reusable web-facing state or UI components. The terminal coding agent lives in `opi-coding-agent`; provider and message types live in `opi-ai`; agent runtime primitives live in `opi-agent`.

## License

MIT. See the workspace [LICENSE](../../LICENSE).
