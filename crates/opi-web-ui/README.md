# opi-web-ui

> Placeholder crate in the [opi](https://github.com/OdradekAI/opi) workspace, reserved for web UI components for AI chat interfaces. A Rust port of [pi](https://github.com/earendil-works/pi)'s `pi-web-ui` package will eventually live here.

[简体中文](README.zh.md) · [← opi](../../README.md)

---

## Status (v0.2.0)

**Not implemented and not published to crates.io** — the `Cargo.toml` carries
`publish = false`. The crate exists only to reserve the name and keep the
workspace layout aligned with upstream `pi`.

What's currently in the source tree:

- `lib.rs` — exports a single `ChatWidget` placeholder struct.
- `components.rs` — `ChatWidget::new()` / `Default` impl; nothing else.

There are no widgets, no rendering, no HTTP integration, and no tests yet.
Track progress in the [project changelog](../../CHANGELOG.md) and the
[opi-spec](../../docs/opi-spec.md).

## License

MIT — see workspace [`LICENSE`](../../LICENSE).
