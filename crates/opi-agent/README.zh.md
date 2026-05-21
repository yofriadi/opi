# opi-agent

[![Crates.io](https://img.shields.io/crates/v/opi-agent.svg)](https://crates.io/crates/opi-agent)
[![Docs.rs](https://docs.rs/opi-agent/badge.svg)](https://docs.rs/opi-agent)

> [opi](https://github.com/OdradekAI/opi) 的 Agent 运行时 —— 工具调用、Hook 生命周期、消息队列轮询。基于 [pi](https://github.com/earendil-works/pi) agent core 的 Rust 移植。

[English](README.md) · [← opi](../../README.zh.md)

---

## 当前状态（v0.2.0）

Phase 1 运行时已经完整：包含一轮完整的 turn 生命周期、工具执行（支持并行 / 串行批次）、参数校验、取消、Hook、steering 与 follow-up 队列。流式 Provider 由 [`opi-ai`](https://crates.io/crates/opi-ai) 提供。

`Transport` trait 已经预留给 stdio / SSE 类型的工具服务器，但本版本尚未接入主循环。

## 核心抽象

> 下方 Hook 签名为示意；完整类型见 `hooks.rs` 中的 `Pin<Box<dyn Future<...>>>`。

```rust
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDef;            // name + JSON Schema
    fn execute(&self, call_id: &str, args: serde_json::Value,
               signal: CancellationToken,
               on_update: Option<UpdateCallback>) -> ...;
    fn execution_mode(&self) -> ExecutionMode { ExecutionMode::Parallel }
}

pub trait AgentHooks: Send + Sync {
    async fn transform_context(...) -> Result<Vec<AgentMessage>, AgentError>;
    fn convert_to_llm(...) -> Result<Vec<Message>, AgentError>;
    async fn before_tool_call(...) -> BeforeToolCallResult;     // Allow | Deny
    async fn after_tool_call(...) -> AfterToolCallResult;       // Keep | Replace
    async fn should_stop_after_turn(...) -> bool;
    async fn prepare_next_turn(...) -> Option<PrepareNextTurnUpdate>;
}
```

## Agent 主循环

```
agent_loop()
  ├── 每一轮（直到 max_turns）：
  │     transform_context  → convert_to_llm  → provider.stream(Request)
  │     ├── 把 AssistantStreamEvent 累积成 AssistantContent
  │     ├── 检测工具调用
  │     │     ├── 用 JSON Schema 校验参数（jsonschema crate）
  │     │     ├── before_tool_call hook（Allow / Deny）
  │     │     ├── 执行（全部 Parallel 则并行，任意 Sequential 则串行）
  │     │     ├── after_tool_call hook（Keep / Replace）
  │     │     ├── 若所有结果 terminate=true，提前结束
  │     │     └── should_stop_after_turn → 决定继续还是停止
  │     └── prepare_next_turn → 可注入额外消息
  ├── drain steering 队列（模式：All）
  └── 无待执行工具时，从 follow_up 队列取一条（模式：OneAtATime）
```

## 用法示例

```rust
use opi_agent::{Agent, ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};
use std::sync::Arc;

struct EchoTool;

impl Tool for EchoTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "echo".into(),
            description: "原样回显输入。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
            }),
        }
    }

    fn execute(&self, _id: &str, args: serde_json::Value,
               _signal: tokio_util::sync::CancellationToken,
               _on_update: Option<opi_agent::tool::UpdateCallback>)
        -> std::pin::Pin<Box<dyn std::future::Future<
            Output = Result<ToolResult, ToolError>> + Send>>
    {
        let text = args.get("text").and_then(|v| v.as_str())
            .unwrap_or("").to_owned();
        Box::pin(async move {
            Ok(ToolResult {
                content: vec![OutputContent::Text { text }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }

    fn execution_mode(&self) -> ExecutionMode { ExecutionMode::Parallel }
}
```

再用 `opi_ai::Provider` 和你的 `AgentHooks` 实现，通过 `Agent::new` 构造 Agent，调用 `agent.prompt("...")` 即可。

## 模块速查

| 模块 | 作用 |
|------|------|
| `agent` | `Agent` 封装，提供 `prompt` / `continue_` / `abort` / `subscribe` |
| （根模块 `agent_loop`） | 推动完整对话的异步入口 |
| `tool` | `Tool` trait、`ToolResult`、`ToolError`、`ExecutionMode` |
| `hooks` | `AgentHooks` trait 与每个 Hook 的上下文 / 结果类型 |
| `event` | `AgentEvent`（开始 / 消息 / 工具 / turn / 队列 / 结束） |
| `state` | `AgentState`（会话状态承载器） |
| `message` | `AgentMessage`（LLM 消息 + 自定义变体） |
| `loop_types` | `AgentLoopContext`、`AgentLoopConfig`、`AgentError` |
| `validation` | 基于 `jsonschema` 的参数校验 |
| `transport` | `Transport` trait 占位（尚未接入主循环） |

## 许可证

MIT —— 见 workspace 根目录 [`LICENSE`](../../LICENSE)。
