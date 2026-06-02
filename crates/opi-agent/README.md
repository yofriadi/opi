# opi-agent

[![Crates.io](https://img.shields.io/crates/v/opi-agent.svg)](https://crates.io/crates/opi-agent)
[![Docs.rs](https://docs.rs/opi-agent/badge.svg)](https://docs.rs/opi-agent)

> General-purpose agent runtime for [opi](https://github.com/OdradekAI/opi): streaming turns, tool calling, hooks, event emission, message queues, sessions, and context compaction.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.4.0`.

`opi-agent` provides the provider-independent runtime used by the `opi` binary. It handles the turn loop, JSON Schema validation for tools, parallel/sequential tool execution, retry-aware provider streaming, image-capability checks, event subscriptions, steering/follow-up queues, JSONL session storage, and threshold/manual/overflow compaction primitives.

The `Transport` trait is available as an abstraction for stdio/SSE tool transports, but external transport-backed tools are not wired into the main loop yet.

## Core Abstractions

```rust
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDef;
    fn execute(&self, call_id: &str, args: serde_json::Value,
               signal: CancellationToken,
               on_update: Option<UpdateCallback>) -> ...;
    fn execution_mode(&self) -> ExecutionMode { ExecutionMode::Parallel }
}

pub trait AgentHooks: Send + Sync {
    async fn transform_context(...) -> Result<Vec<AgentMessage>, AgentError>;
    fn convert_to_llm(...) -> Result<Vec<Message>, AgentError>;
    async fn before_tool_call(...) -> BeforeToolCallResult;
    async fn after_tool_call(...) -> AfterToolCallResult;
    async fn should_stop_after_turn(...) -> bool;
    async fn prepare_next_turn(...) -> Option<PrepareNextTurnUpdate>;
}
```

`Agent` wraps the loop with `prompt`, `prompt_with_content`, `continue_`, `abort`, `subscribe`, `steer`, `follow_up`, `add_tool`, model switching, and message-buffer helpers.

## Agent Loop

```text
agent_loop
  -> transform_context
  -> convert_to_llm
  -> validate request capabilities
  -> provider.stream(Request)
  -> emit/accumulate AssistantStreamEvent values
  -> detect tool calls
  -> validate args with jsonschema
  -> before_tool_call hook
  -> execute tools
     -> all parallel tools run together
     -> any sequential tool makes the batch sequential
  -> after_tool_call hook
  -> stop if all tool results terminate
  -> should_stop_after_turn hook
  -> prepare_next_turn hook
  -> drain steering queue
  -> pop one follow-up message when no tools are pending
```

Retryable provider errors (`RateLimited`, `Timeout`) can be retried through `AgentLoopConfig.retry`. Retry start/end events are emitted through `AgentEvent`.

## Sessions and Compaction

Session storage uses append-only JSONL:

- First line: `SessionHeader`.
- Entries: `MessageEntry`, `CompactionEntry`, and `LeafEntry`.
- Reader supports crash recovery by skipping corrupt entries and truncated trailing lines.

Compaction support includes:

- `CompactionConfig { enabled, threshold_tokens }`.
- `CompactionReason::{Manual, Threshold, Overflow}`.
- `CompactionEngine::should_compact`.
- `CompactionEngine::compact`.
- `CompactionHooks` for custom summary generation, with a core fallback summary.

`opi-coding-agent` owns the higher-level coordinator that connects these primitives to runtime persistence.

## Events

`AgentEvent` reports agent lifecycle, turn lifecycle, message streaming, tool execution, queues, automatic retries, compaction, session persistence errors, and agent end.

`AgentSessionEvent` is the session-level wire protocol used by JSON output. It wraps agent events and adds compaction, retry, thinking-level, session-info, and session-summary events.

## Quick Example

```rust
use opi_agent::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};

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

Create an `Agent` with a boxed `opi_ai::Provider`, tool list, model, optional system prompt, `AgentLoopConfig`, and an `AgentHooks` implementation. Use `prompt_with_content` when a user turn contains text plus images.

## Modules

| Module | Purpose |
|--------|---------|
| `agent` | Stateful `Agent` wrapper, model switching, cancellation, queues, message buffer management |
| root `agent_loop` | Provider/tool turn loop |
| `tool` | `Tool`, `ToolResult`, `ToolError`, `ExecutionMode`, update callbacks |
| `hooks` | Hook trait and hook context/result types |
| `event` | Runtime event protocol |
| `session_event` | Session-level event protocol for JSON mode |
| `session` | JSONL session header, entries, writer, reader, recovery |
| `compaction` | Context compaction engine and hooks |
| `state` | Conversation state holder |
| `message` | Agent-level message variants |
| `loop_types` | Loop context, config, and errors |
| `validation` | JSON Schema argument validation |
| `transport` | Transport trait for external tool servers |

## License

MIT. See the workspace [LICENSE](../../LICENSE).
