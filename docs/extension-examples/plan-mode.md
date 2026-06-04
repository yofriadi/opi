# Plan Mode Extension Example

This is an **example extension** that demonstrates a planning workflow through
custom commands and extension hooks. It is **not core plan mode functionality**
and does not add feature flags to the agent runtime.

## Overview

The plan mode extension provides commands to enter and exit a planning state.
While in planning mode, mutating tools (write, edit, bash) are blocked and
read-only tools continue to work. Plan notes are tracked and state persists
through serialization.

## Commands

| Command | Behavior |
|---------|----------|
| `plan-mode/enter` | Activates plan mode, optionally records a note. |
| `plan-mode/exit` | Deactivates plan mode. |
| `plan-mode/status` | Returns current mode, notes, and tool statistics. |

## How It Works

1. The extension registers `on_command` to handle `plan-mode/enter`,
   `plan-mode/exit`, and `plan-mode/status`.
2. `on_before_tool_call` checks the current mode. If planning, mutating tools
   are blocked with a descriptive reason; read-only tools are allowed.
3. Plan notes are accumulated across enter cycles and persisted through
   `serialize_state`/`restore_state`.
4. `on_event` observes parent agent events for diagnostic purposes.

## Tool Gating

| Tool | Planning Mode | Normal Mode |
|------|--------------|-------------|
| write | blocked | allowed |
| edit | blocked | allowed |
| bash | blocked | allowed |
| read | allowed | allowed |
| glob | allowed | allowed |
| grep | allowed | allowed |
| find | allowed | allowed |
| ls | allowed | allowed |

## Test Coverage

Tests in `crates/opi-coding-agent/tests/plan_mode_example.rs` cover:

- **Mode transitions**: Enter activates planning, exit returns to normal.
- **Tool gating**: Mutating tools blocked, read-only tools allowed in plan mode.
- **Normal mode**: All tools allowed when not planning.
- **Agent integration**: Blocked tool calls propagate through the agent loop.
- **State persistence**: Plan state round-trips through serialization.
- **Event observation**: Parent agent events received by extension.
- **Multiple cycles**: Enter/exit toggles work repeatedly.
- **Unknown commands**: Unrecognized commands return `None` (passthrough).

## Package Structure

```text
examples/plan-mode/
  package.toml                            # Package manifest
  extensions/
    plan-mode/
      extension.toml                      # Extension manifest
```

## Why This Is an Example

The plan mode extension does not introduce any new concepts into the core
runtime. It uses only the standard extension API (`Extension` trait,
`ExtensionRegistry`, `ExtensionCommand`) and demonstrates how to build
workflow-specific tool gating without modifying opi itself.
