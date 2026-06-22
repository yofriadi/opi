# opi-agent

[![Crates.io](https://img.shields.io/crates/v/opi-agent.svg)](https://crates.io/crates/opi-agent)
[![Docs.rs](https://docs.rs/opi-agent/badge.svg)](https://docs.rs/opi-agent)

> Provider-independent agent runtime used by [opi](https://github.com/OdradekAI/opi).

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.5.3`, inherited from the workspace package version.

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

The loop emits a fixed runtime event order. `AgentStart` fires once before the
first turn and `AgentEnd` fires once on every termination path (normal stop,
hook stop, terminate flags, cancellation, or error). Per turn (`0..max_turns`):

```text
agent_start                              # once, before turn 0
  for each turn:
    cancel check                          # cancelled -> AgentEnd, AgentError::Cancelled
    turn_start
    transform_context                     # AgentHooks::transform_context
    convert_to_llm                        # AgentHooks::convert_to_llm
    validate request capabilities         # failure -> AgentEnd, AgentError::Provider
    provider.stream(Request)
      message_start                       # assistant stream Start
      message_update                      # per text/thinking delta
      message_end                         # complete assistant message
      if tool calls are present:
        validate tool args (jsonschema)
        tool_execution_start              # per tool call
        before_tool_call                  # AgentHooks::before_tool_call (may block)
        tool.execute                      # parallel batch, or sequential if any tool is Sequential
        after_tool_call                   # AgentHooks::after_tool_call (may replace result)
        tool_execution_end                # per tool call
        turn_end                          # assistant message + tool_results
        if every result terminates -> AgentEnd, return Ok
        should_stop_after_turn            # true -> AgentEnd, return Ok (compaction stop)
      else:
        turn_end                          # assistant message, no tool_results
        should_stop_after_turn            # true -> AgentEnd, return Ok
    prepare_next_turn                     # AgentHooks::prepare_next_turn; SKIPPED after a
                                         # terminal should_stop_after_turn; may inject messages
    drain steering queue                  # non-empty -> QueueUpdate, append, next turn
    if no tools are pending:
      pop follow-up queue                 # non-empty -> QueueUpdate, append, next turn
      else -> stop
agent_end                                 # once, on termination
```

Boundaries:

- `should_stop_after_turn` runs after `turn_end` and before `prepare_next_turn`
  and any queue polling. A compaction coordinator returns `true` here to stop
  before the next turn; `prepare_next_turn` and steering/follow-up polling do
  not run after a terminal stop.
- `prepare_next_turn` runs only when `should_stop_after_turn` permits
  continuation, and before steering/follow-up polling. Injected messages are
  included in the next provider request.
- Steering is drained before follow-up. Follow-up is popped only when no tools
  are pending and the steering queue is empty.
- `CompactionEngine` is a context-size primitive; the higher-level coordinator
  that connects compaction to persisted CLI sessions lives in `opi-coding-agent`
  and stops the loop through `should_stop_after_turn`.

Retryable provider errors such as rate limits and timeouts can be retried
through `AgentLoopConfig.retry`. Retry start/end events are surfaced through
`AgentEvent`.

## Hook Semantics

`AgentHooks` customizes the loop. The six methods run in this order and have
these effects:

| Hook | Order / effect |
|------|----------------|
| `transform_context` | Runs before provider conversion; may alter app-level messages. |
| `convert_to_llm` | Converts app messages to provider messages and filters session-only state. |
| `before_tool_call` | Runs after JSON Schema argument validation and before `tool.execute`; may `Deny` to block execution (the deny reason becomes the tool error). |
| `after_tool_call` | Runs after execution and before the final `ToolExecutionEnd` event; may `Replace` the result so the replacement is what is emitted and persisted. |
| `should_stop_after_turn` | Runs after `turn_end` and before steering/follow-up polling; returning `true` stops before the next turn and skips `prepare_next_turn`. |
| `prepare_next_turn` | Runs only when `should_stop_after_turn` permits continuation, and before steering/follow-up polling; may inject messages into the next provider request. |

Extension composition: `ExtensionRegistry::wrap_hooks` runs the base
`AgentHooks` method first, then each extension in registration order. A
`Block` from an extension's `on_before_tool_call` stops the chain at the first
block; later extensions are not consulted. Extension `on_after_tool_call`
observers cannot modify the result; only the base hook can `Replace`.

When an adapter or extension implements only a subset of hooks, the skipped
hooks are recorded as `trace::TraceKind::HookSkipped` records when verbose
tracing is enabled. The runtime pushes the per-run `TraceCollector` to every
extension via `Extension::set_trace_collector` before each run (and clears it
after), so adapters that short-circuit an undeclared hook can record the skip.

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
