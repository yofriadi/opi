# Sub-Agent Extension Example

This is an **example extension** that demonstrates how to orchestrate nested
agent workflows using opi's extension hook system. It is **not core sub-agent
functionality** and does not add feature flags to the agent runtime.

## Overview

The sub-agent extension handles custom commands to create and run child agents
with isolated state, bounded cancellation, and observable event routing. Each
child run uses a fresh provider, tools, and cancellation token.

## Commands

| Command | Behavior |
|---------|----------|
| `sub-agent/run` | Creates a child agent, runs it, returns the result. |
| `sub-agent/list` | Returns the run history with status and results. |

## How It Works

1. The extension registers `on_command` to handle `sub-agent/run` and
   `sub-agent/list` commands.
2. For `sub-agent/run`: a fresh `Agent` is created with a child provider
   (from a factory), child tools, and its own cancellation token.
3. The child agent runs to completion (or cancellation/error).
4. Results are recorded in a run history and returned as a JSON response
   including the `SDK_SCHEMA_VERSION`.
5. Child agent events are captured via `Agent::subscribe` and stored for
   parent inspection.
6. The extension also observes parent agent events via `on_event`.

## Isolation

- Each child run gets a fresh `Agent` instance with its own provider, tools,
  and hooks.
- Child state does not leak to the parent extension.
- Run history is tracked independently per extension instance.

## Cancellation

Child agents have their own `CancellationToken` stored in
`active_child_cancel`. External code (or other extensions) can cancel a
running child by triggering this token. The child run returns a "cancelled"
status with an error message.

## Test Coverage

Tests in `crates/opi-coding-agent/tests/sub_agent_example.rs` cover:

- **Completion**: Child run completes, result routed to parent; child with
  tool calls completes with tool events.
- **Error propagation**: Child provider errors surface to the parent command
  response.
- **Cancellation**: Child run cancelled mid-execution via cancellation token.
- **Event routing**: Full child agent lifecycle events observable by parent;
  parent agent events observed via `on_event`.
- **Isolated state**: Multiple child runs have independent providers and
  results.
- **Session visibility**: Run history queryable via `sub-agent/list` command.
- **State round-trip**: Run history serializes and restores through the
  extension state system.
- **Unknown commands**: Unrecognized commands return `None` (passthrough).

## Package Structure

```text
examples/sub-agent/
  package.toml                            # Package manifest
  extensions/
    sub-agent/
      extension.toml                      # Extension manifest
```

## Why This Is an Example

The sub-agent extension does not introduce any new concepts into the core
runtime. It uses only the standard extension API (`Extension` trait,
`ExtensionRegistry`, `ExtensionCommand`) and the SDK types (`SdkResponse`,
`SDK_SCHEMA_VERSION`). Its purpose is to show extension authors how to
orchestrate nested agent workflows without modifying opi itself.
