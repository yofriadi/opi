# opi-web-ui

> Unpublished web-facing state and component layer for the
> [opi](https://github.com/OdradekAI/opi) agent toolkit.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.5.2`, inherited from the workspace package version.

`opi-web-ui` has `publish = false`. It is not a standalone browser app and does
not start a server. It provides typed Rust state and HTML component models for
consuming `opi` RPC/SDK JSON events in an embedder-controlled web surface.

All public types are unstable 0.x APIs. Pin an exact version and test upgrades.

## Scope

| Module | Purpose |
|--------|---------|
| `event` | Parses raw RPC JSON values into `WebUiEvent` variants and preserves unknown event types. |
| `state` | Maintains conversation state from events: messages, tool calls, thinking blocks, session metadata, resource metadata, compaction status, and the last RPC response. |
| `components` | Typed component models: `ChatMessage`, `ToolCallView`, `ThinkingBlock`, `StatusBar`, and `ConversationView`. |
| `render` | `Render` trait plus HTML escaping for XSS-safe string output. |

The crate intentionally keeps its runtime boundary JSON-shaped. It does not
depend on `opi-ai` or `opi-agent` at runtime; `opi-agent` is a dev-dependency
for tests only.

## Usage

```rust
use opi_web_ui::event::WebUiEvent;
use opi_web_ui::render::Render;
use opi_web_ui::state::ConversationState;

let mut state = ConversationState::new();

// Parse a raw RPC/SDK event.
let raw = serde_json::json!({"type": "AgentStart"});
let event = WebUiEvent::parse(&raw).unwrap();
state.process(event);

// Process typed events directly when the caller already has them.
state.process(WebUiEvent::MessageStart {
    model: "claude-sonnet-4-5".to_owned(),
    provider: "anthropic".to_owned(),
});
state.process(WebUiEvent::TextDelta {
    index: 0,
    delta: "Hello".to_owned(),
});
state.process(WebUiEvent::MessageEnd);

let html = state.to_conversation_view().render_html();
let status = state.to_status_bar().render_html();
```

`ConversationState` exposes read-only accessors for messages, tool calls,
thinking blocks, model, session id, turn/message counts, agent-running state,
compaction state, the last RPC response, resource metadata, and the last
successful compaction payload.

## Boundary

Use this crate for reusable web-facing state or HTML component models. Keep
terminal UI in `opi-tui`, CLI/harness behavior in `opi-coding-agent`, provider
types in `opi-ai`, and runtime loop primitives in `opi-agent`.

## License

MIT. See the workspace [LICENSE](../../LICENSE).
