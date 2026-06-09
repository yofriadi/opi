# protected-paths

An example package demonstrating a protected paths extension.

## What This Is

This is an **example package** that demonstrates how to build path-based access
control using opi's extension hook system. It is **not core policy** and does
not modify opi's built-in file tool behavior.

## How It Works

The protected paths extension implements `on_before_tool_call` to intercept
file-tool operations and evaluate them against configurable path rules:

- **AllowAll**: All paths permitted (no restrictions).
- **DenyPaths**: Listed paths and their children are blocked.
- **AllowPaths**: Only listed paths and their children are permitted.

Path normalization resolves relative paths, `..` traversal, and symlinks
before checking against rules. Every allow/deny decision is recorded in an
audit log that can be serialized and restored through the extension state
system.

## Covered Tools

- **read, write, edit**: Path checked against rules from the `path` argument.
- **bash**: Workspace root checked as implicit cwd.
- **Other tools** (glob, grep, etc.): Pass through unaffected.

## Package Structure

```text
protected-paths/
  package.toml                            # Package manifest
  extensions/
    protected-paths/
      extension.toml                      # Extension manifest
```

The actual Rust implementation lives in the test file
`crates/opi-coding-agent/tests/protected_paths_example.rs`, which serves as
both the example code and the integration test suite.

## Process Adapter

This package also declares a process adapter in `package.toml`. The adapter
binary (`package_adapter_example` with mode `protected-paths`) runs as a
child process communicating over the opi extension JSONL protocol. It
implements `before_tool_call` to block file operations on protected system
paths (`/etc/`, `/proc/`, `/sys/`) while allowing other paths through.

Adapter tests live in
`crates/opi-coding-agent/tests/example_adapters.rs`.
