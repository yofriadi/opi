# opi-web-ui

> [opi](https://github.com/OdradekAI/opi) workspace 中预留的 Web UI 组件 crate。

[English](README.md) | [opi workspace](../../README.zh.md)

## 当前状态

当前 crate 版本：`0.4.0`。

`opi-web-ui` 仍是占位 crate，不会发布到 crates.io（`publish = false`）。它用于保持 workspace 边界稳定，并为后续可复用 Web 聊天组件预留包边界。

当前源码内容：

- `lib.rs`：声明模块并重新导出 `ChatWidget`。
- `components.rs`：空的 `ChatWidget` 类型，带 `new()` 和 `Default`。

目前还没有真实 widget、渲染适配、HTTP 集成、浏览器绑定、文档预览组件或测试。该 crate 依赖 `opi-ai`、`serde`、`serde_json` 和 `thiserror`，但占位实现尚未实质使用这些依赖。

## 公共 API

```rust
use opi_web_ui::ChatWidget;

let widget = ChatWidget::new();
let default_widget = ChatWidget::default();
```

## 边界说明

只有实现面向 Web 的可复用 UI 组件后，相关功能才应进入这里。终端编程 Agent 位于 `opi-coding-agent`；Provider 和消息类型位于 `opi-ai`；Agent 运行时基础能力位于 `opi-agent`。

在真实组件存在之前，不应把该 crate 描述为已实现的 Web UI。

## 许可证

MIT。详见 workspace [LICENSE](../../LICENSE)。
