# Permission Gate Extension Example

This is an **example extension** that demonstrates how to build a permission
gate using opi's extension hook system. It is **not core policy** and does not
add a permanent permission-popup subsystem to the agent runtime.

## Overview

The permission gate extension intercepts tool calls through the
`on_before_tool_call` hook and evaluates them against a configurable policy.
Every allow/deny decision is recorded in an audit log that persists through
the extension state serialization system.

## Policies

| Policy | Behavior |
|--------|----------|
| `AllowAll` | All tool calls pass through. |
| `DenyAll` | All tool calls are blocked. |
| `DenyList` | Only tools in the deny list are blocked. |
| `AllowList` | Only tools in the allow list are permitted. |

## How It Works

1. The extension registers `on_before_tool_call` to intercept every tool call.
2. The hook evaluates the tool name against the configured policy.
3. If the policy denies the call, the hook returns `ExtensionHookResult::Block`
   with a reason string. The agent loop treats this as a tool error and
   reports it back to the model.
4. If the policy allows the call, the hook returns `ExtensionHookResult::Continue`
   and execution proceeds normally.
5. Every decision is appended to an internal audit log.
6. The extension receives agent lifecycle events through `on_event`, enabling
   event-driven audit or monitoring.

## Non-Interactive Mode

The example operates in non-interactive mode, making automatic decisions based
on the configured policy. No user prompts or TUI interaction is involved.

An interactive variant would extend this pattern by:

- Checking a "mode" flag in `on_before_tool_call`.
- When interactive, emitting a prompt event or RPC command to request user
  approval.
- Using a channel or future to await the user's response.
- This does not require any core runtime changes — the extension API already
  supports async hooks.

## Test Coverage

Tests in `crates/opi-coding-agent/tests/permission_gate_example.rs` cover:

- **Allow**: `AllowAll` and `AllowList` policies permit tool calls.
- **Deny**: `DenyAll` and `DenyList` policies block tool calls with reasons.
- **Audit/event output**: Audit log records decisions across turns; extension
  receives agent lifecycle events through `on_event`.
- **Non-interactive behavior**: Automatic decisions without user prompting for
  all policy types.

## Package Structure

```text
examples/permission-gate/
  package.toml                            # Package manifest
  extensions/
    permission-gate/
      extension.toml                      # Extension manifest
```

## Why This Is an Example

The permission gate does not introduce any new concepts into the core
runtime. It uses only the standard extension API (`Extension` trait,
`ExtensionRegistry`, `CompositeHooks`). Its purpose is to show extension
authors how to build permission-gating logic without modifying opi itself.
