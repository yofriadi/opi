# permission-gate

An example package demonstrating a permission gate extension.

## What This Is

This is an **example package** that demonstrates how to build a permission gate
using opi's extension hook system. It is **not core policy** and does not add a
permanent permission-popup subsystem to the agent runtime.

## How It Works

The permission gate extension implements `on_before_tool_call` to intercept
tool calls and evaluate them against a configurable policy:

- **AllowAll**: All tool calls pass through.
- **DenyAll**: All tool calls are blocked.
- **DenyList**: Only tools in the deny list are blocked.
- **AllowList**: Only tools in the allow list are permitted.

Every allow/deny decision is recorded in an audit log that can be serialized
and restored through the extension state system.

## Non-Interactive Mode

The extension operates in non-interactive mode by default, making automatic
decisions based on the configured policy. An interactive variant (prompting
the user for each decision) would extend this pattern by adding TUI or RPC
interaction within the `on_before_tool_call` hook.

## Package Structure

```text
permission-gate/
  package.toml                            # Package manifest
  extensions/
    permission-gate/
      extension.toml                      # Extension manifest
```

The actual Rust implementation lives in the test file
`crates/opi-coding-agent/tests/permission_gate_example.rs`, which serves as
both the example code and the integration test suite.

## Process Adapter

This package also declares a process adapter in `package.toml`. The adapter
binary (`package_adapter_example` with mode `permission-gate`) runs as a
child process communicating over the opi extension JSONL protocol. It
implements `before_tool_call` to block mutating tools (bash, write, edit)
while allowing read-only tools through.

Adapter tests live in
`crates/opi-coding-agent/tests/example_adapters.rs`.
