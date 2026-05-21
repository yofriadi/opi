# opi-web-ui

> [opi](https://github.com/OdradekAI/opi) workspace 中的占位 crate，为后续的 AI 聊天界面 Web 组件预留命名。未来会承载 [pi](https://github.com/earendil-works/pi) `pi-web-ui` 包的 Rust 移植版本。

[English](README.md) · [← opi](../../README.zh.md)

---

## 当前状态（v0.2.0）

**未实现，也不会发布到 crates.io** —— `Cargo.toml` 中显式声明了 `publish = false`。本 crate 当前只用于占位，保持工作区与上游 `pi` 的结构一致。

源码目录里实际存在的东西：

- `lib.rs` —— 导出一个占位结构体 `ChatWidget`。
- `components.rs` —— `ChatWidget::new()` / `Default` 实现，仅此而已。

目前没有任何组件、渲染、HTTP 集成或测试。后续进展请关注 [项目 CHANGELOG](../../CHANGELOG.md) 与 [opi 规范文档](../../docs/opi-spec.zh.md)。

## 许可证

MIT —— 见 workspace 根目录 [`LICENSE`](../../LICENSE)。
