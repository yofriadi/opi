# opi-agent

[![Crates.io](https://img.shields.io/crates/v/opi-agent.svg)](https://crates.io/crates/opi-agent)
[![Docs.rs](https://docs.rs/opi-agent/badge.svg)](https://docs.rs/opi-agent)

> Provider-independent agent runtime used by [opi](https://github.com/OdradekAI/opi).

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.5.2`, inherited from the workspace package version.

`opi-agent` owns the agent loop and runtime primitives: tool contracts,
JSON Schema argument validation, parallel/sequential tool execution, lifecycle
hooks, event emission, steering/follow-up queues, session JSONL storage,
branch reconstruction, context compaction, SDK/RPC types, extensions, local
diagnostics, redacted trace envelopes, and streaming proxy support.

It depends on `opi-ai` for provider and message types. It does not implement the
`opi` CLI, terminal UI, or built-in filesystem/shell tools; those live in
`opi-coding-agent` and `opi-tui`.

## Core Abstractions

| Item | Purpose |
|------|---------|
| `Agent` | Stateful wrapper around the loop with prompt, continue, abort, subscribe, steering, follow-up, model switching, and tool registration helpers. |
| `Tool` | JSON Schema based tool contract with cancellable execution and optional progress updates. |
| `ExecutionMode` | Controls whether a tool can run in a parallel batch or forces sequential execution. |
| `AgentHooks` | Lifecycle hooks for context transforms, LLM conversion, tool policy/results, stopping, and next-turn preparation. |
| `AgentEvent` | Runtime event stream for lifecycle, streaming text, tool calls, queues, retries, compaction, and end state. |
| `AgentSessionEvent` | Session-level event protocol used by `opi --json`. |
| `AgentLoopConfig` | Loop limits, retry config, compaction config, and related runtime settings. |

## Loop Shape

```text
agent_loop
  -> transform_context
  -> convert_to_llm
  -> validate request capabilities
  -> provider.stream(Request)
  -> emit and accumulate AssistantStreamEvent values
  -> detect tool calls
  -> validate tool args with jsonschema
  -> before_tool_call hook
  -> execute tools in parallel or sequential batches
  -> after_tool_call hook
  -> should_stop_after_turn hook
  -> prepare_next_turn hook
  -> drain steering and follow-up queues
```

Retryable provider errors such as rate limits and timeouts can be retried
through `AgentLoopConfig.retry`. Retry start/end events are surfaced through
`AgentEvent`.

## Sessions and Compaction

Session storage is append-only JSONL:

- First line: `SessionHeader`.
- Entries: `MessageEntry`, `CompactionEntry`, and `LeafEntry`.
- Reader recovery skips corrupt entries and truncated trailing lines.
- `session_branch::SessionTree` reconstructs active branches from `parent_id`
  links and the latest `LeafEntry`.

Compaction primitives include threshold/manual/overflow reasons,
`CompactionEngine::should_compact`, `CompactionEngine::compact`, and
`CompactionHooks` for custom summary generation. `opi-coding-agent` owns the
higher-level coordinator that connects these primitives to persisted CLI
sessions.

## SDK, Extensions, Diagnostics, and Proxy

- `sdk` defines schema-versioned command/response types shared by RPC JSONL
  mode and embedders. `SDK_SCHEMA_VERSION` is `3`.
- `extension` provides `Extension` and `ExtensionRegistry` for lifecycle hooks,
  custom tools, custom commands, event observers, extension state, custom
  providers, and model overrides.
- `diagnostic` and `diagnostic_sink` provide typed diagnostics with redaction
  helpers for public JSON/text boundaries.
- `trace` stores a local, redacted trace envelope for the latest run when a
  caller opts in.
- `streaming_proxy` forwards JSONL commands/events over arbitrary
  `BufRead`/`Write` transports, emits a `proxy_ready` header, buffers events,
  supports cancellation, and redacts common secret patterns by default.

All SDK/RPC/proxy surfaces are unstable 0.x APIs. Clients should check schema
versions and pin exact crate versions when needed.

## Public Modules

`agent`, `compaction`, `diagnostic`, `diagnostic_sink`, `event`, `extension`,
`hooks`, `loop_types`, `message`, `sdk`, `session`, `session_branch`,
`session_event`, `state`, `streaming_proxy`, `tool`, `trace`, and `validation`.

The crate root re-exports the most common runtime types, including `Agent`,
`Tool`, `ToolResult`, `ToolError`, `ExecutionMode`, `AgentHooks`, `AgentEvent`,
`AgentSessionEvent`, `AgentLoopConfig`, `SdkCommand`, `SdkResponse`, and
`ToolDef`.

## Testing Support

Use `opi_ai::test_support::MockProvider` with custom `Tool` implementations for
deterministic loop tests. Tests that touch session storage should use isolated
temporary directories.

## License

MIT. See the workspace [LICENSE](../../LICENSE).
