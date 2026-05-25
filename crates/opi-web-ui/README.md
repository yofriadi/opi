# opi-web-ui

> Reserved web UI component crate in the [opi](https://github.com/OdradekAI/opi) workspace.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.3.0`.

`opi-web-ui` is still a placeholder and is not published to crates.io (`publish = false`). The crate exists to keep the workspace layout stable and reserve the package boundary for future reusable web chat components.

Current source contents:

- `lib.rs`: module declaration and `ChatWidget` re-export.
- `components.rs`: empty `ChatWidget` type with `new()` and `Default`.

There are no real widgets, rendering adapters, HTTP integrations, browser bindings, or tests yet. The crate depends on `opi-ai`, `serde`, `serde_json`, and `thiserror`, but those dependencies are not meaningfully exercised by the placeholder implementation.

## Public API

```rust
use opi_web_ui::ChatWidget;

let widget = ChatWidget::new();
let default_widget = ChatWidget::default();
```

## Roadmap Boundary

Expected future work belongs here only when web-facing reusable components are implemented. The terminal coding agent lives in `opi-coding-agent`; provider and message types live in `opi-ai`.

## License

MIT. See the workspace [LICENSE](../../LICENSE).
