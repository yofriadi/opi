# opi-agent

[![Crates.io](https://img.shields.io/crates/v/opi-agent.svg)](https://crates.io/crates/opi-agent)
[![Docs.rs](https://docs.rs/opi-agent/badge.svg)](https://docs.rs/opi-agent)

> Agent runtime — tool calling, hook lifecycle, and queue polling — for [opi](https://github.com/OdradekAI/opi). A Rust port of [pi](https://github.com/earendil-works/pi)'s agent core.

[简体中文](README.zh.md) · [← opi](../../README.md)

---

## Status (v0.2.0)

The Phase 1 runtime ships with the full turn lifecycle, tool execution
(parallel + sequential batching), validated arguments, cancellation, hook
points, and steering / follow-up message queues. Built on
[`opi-ai`](https://crates.io/crates/opi-ai) for provider streaming.

The `Transport` trait is reserved for stdio / SSE tool servers but is not yet
wired into the agent loop.

## Core abstractions

> Hook signatures below are abbreviated; see `hooks.rs` for the full
> `Pin<Box<dyn Future<...>>>` return types.

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

## Agent loop

```
agent_loop()
  ├── for each turn (up to max_turns):
  │     transform_context  → convert_to_llm  → provider.stream(Request)
  │     ├── accumulate AssistantStreamEvent into AssistantContent
  │     ├── detect tool calls
  │     │     ├── validate args against JSON Schema (jsonschema crate)
  │     │     ├── before_tool_call hook (Allow / Deny)
  │     │     ├── execute (parallel when all tools are Parallel,
  │     │     │            sequential if any tool is Sequential)
  │     │     ├── after_tool_call hook (Keep / Replace)
  │     │     ├── early stop if ALL results have terminate=true
  │     │     └── should_stop_after_turn → stop or continue
  │     └── prepare_next_turn → may inject extra messages
  ├── drain steering queue (mode: All)
  └── pop one follow_up message (mode: OneAtATime) when no tools pending
```

## Quick example

```rust
use opi_agent::{Agent, ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};
use std::sync::Arc;

struct EchoTool;

impl Tool for EchoTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "echo".into(),
            description: "Echo back the input.".into(),
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

Wire it up with an `opi_ai::Provider` and `AgentHooks` impl via `Agent::new`,
then call `agent.prompt("...")`.

## Modules

| Module | Purpose |
|--------|---------|
| `agent` | `Agent` wrapper with `prompt`, `continue_`, `abort`, `subscribe` |
| (root `agent_loop`) | Async function that drives one full conversation |
| `tool` | `Tool` trait, `ToolResult`, `ToolError`, `ExecutionMode` |
| `hooks` | `AgentHooks` trait + per-hook context / result types |
| `event` | `AgentEvent` (start / message / tool / turn / queue / end) |
| `state` | `AgentState` (conversation state holder) |
| `message` | `AgentMessage` (LLM message + custom variants) |
| `loop_types` | `AgentLoopContext`, `AgentLoopConfig`, `AgentError` |
| `validation` | `jsonschema`-backed argument validation |
| `transport` | Placeholder `Transport` trait (not yet wired in) |

## License

MIT — see workspace [`LICENSE`](../../LICENSE).
