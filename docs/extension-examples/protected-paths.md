# Protected Paths Extension Example

This is an **example extension** that demonstrates how to build path-based
access control using opi's extension hook system. It is **not core policy** and
does not modify opi's built-in file tool behavior.

## Overview

The protected paths extension intercepts file-tool operations through the
`on_before_tool_call` hook and evaluates them against configurable path rules.
Every allow/deny decision is recorded in an audit log that persists through the
extension state serialization system.

## Policies

| Policy | Behavior |
|--------|----------|
| `AllowAll` | All paths permitted (no restrictions). |
| `DenyPaths` | Listed paths and their children are blocked. |
| `AllowPaths` | Only listed paths and their children are permitted. |

## How It Works

1. The extension registers `on_before_tool_call` to intercept file-tool
   operations (read, write, edit, bash).
2. For read/write/edit: the `path` argument is extracted and normalized.
3. For bash: the workspace root is used as the implicit cwd.
4. The normalized path is checked against the configured policy.
5. If the policy denies the path, the hook returns
   `ExtensionHookResult::Block` with a reason string. The agent loop treats
   this as a tool error and reports it back to the model.
6. If the policy allows the path, the hook returns
   `ExtensionHookResult::Continue` and execution proceeds normally.
7. Every decision is appended to an internal audit log.
8. Non-file tools (glob, grep, etc.) pass through without evaluation.

## Path Normalization

The extension normalizes paths before checking them against rules:

- Relative paths are resolved against the workspace root.
- `..` components are resolved to prevent parent-directory traversal.
- Symlinks are resolved via `canonicalize` when the target exists.
- For non-existent files, parent-directory canonicalization ensures
  consistent path forms across existing and non-existing targets.

## Test Coverage

Tests in `crates/opi-coding-agent/tests/protected_paths_example.rs` cover:

- **Allow**: `AllowAll` permits read/write; `AllowPaths` permits listed paths.
- **Deny**: `DenyPaths` blocks matching paths; allows non-matching paths.
- **Edit**: Edit tool blocked on protected files.
- **Bash cwd**: Bash blocked when workspace root is in the deny list.
- **Path normalization**: `..` traversal resolved; absolute paths outside
  workspace blocked.
- **Symlink traversal**: Paths through symlinks to protected directories are
  blocked (platform-dependent; skipped if symlinks unavailable).
- **Non-file tools**: Tools without a `path` argument pass through without
  audit entries.
- **Audit/event output**: Audit log records allow and deny decisions across
  turns; extension receives agent lifecycle events; state round-trips through
  serialization.

## Package Structure

```text
examples/protected-paths/
  package.toml                            # Package manifest
  extensions/
    protected-paths/
      extension.toml                      # Extension manifest
```

## Why This Is an Example

The protected paths extension does not introduce any new concepts into the
core runtime. It uses only the standard extension API (`Extension` trait,
`ExtensionRegistry`, `CompositeHooks`). Its purpose is to show extension
authors how to build path-based access control without modifying opi itself.
