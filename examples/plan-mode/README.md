# plan-mode

An example package demonstrating a plan mode extension.

## What This Is

This is an **example package** that demonstrates a planning workflow through
custom commands and extension hooks without adding built-in plan mode to the
core runtime.

## How It Works

The plan mode extension implements `on_command` to manage planning state:

- **`plan-mode/enter`**: Activates planning mode, optionally with a note.
  Returns the planning prompt.
- **`plan-mode/exit`**: Deactivates planning mode, returns to normal.
- **`plan-mode/status`**: Returns current mode, plan notes, and tool stats.

When planning mode is active, `on_before_tool_call` blocks mutating tools
(write, edit, bash) and allows read-only tools (read, glob, grep, find, ls).

## Key Properties

- **Mutating tool gating**: write, edit, bash blocked in plan mode.
- **Read-only allowed**: read, glob, grep, find, ls continue working.
- **State persistence**: Plan state round-trips through serialize/restore.
- **Event observation**: Parent agent events visible via `on_event`.
- **No feature flags**: No core runtime changes required.

## Package Structure

```text
plan-mode/
  package.toml                            # Package manifest
  extensions/
    plan-mode/
      extension.toml                      # Extension manifest
```

The actual Rust implementation lives in the test file
`crates/opi-coding-agent/tests/plan_mode_example.rs`, which serves as both
the example code and the integration test suite.
