# Pi-Aligned Tool Policy Design

## Context

Opi is a Rust implementation inspired by pi. Phase 3 left three behavior
differences that make the default product feel more conservative than pi:

- Interactive mode exposes mutating tools to the model, but denies `write`,
  `edit`, and `bash` unless `--allow-mutating` or config opt-in is set.
- File tools resolve paths only inside the workspace.
- The default active tool set is all built-in tools, including search/list
  tools and opi-native `glob`, while pi's default active tools are
  `read`, `write`, `edit`, and `bash`.

The goal is to align opi's default interactive experience with pi while keeping
an explicit automation safety boundary for non-interactive runs.

## Goals

- Make interactive mode behave like a default pi coding agent.
- Keep non-interactive mode safe by default.
- Stop advertising tools to the model that the runtime will later deny.
- Separate available built-in tools from default active tools.
- Make path handling an explicit policy instead of a hard-coded workspace-only
  rule.
- Preserve existing `--tools`, `--no-tools`, and `--no-builtin-tools` shapes so
  Phase 4 extension tooling can build on them.

## Non-Goals

- Do not implement Phase 4 extensions, packages, or custom tool loading.
- Do not add a permanent permission-popup subsystem.
- Do not make MCP part of core.
- Do not add a new `--toolset` flag in this fix. It can be added later if the
  existing `--tools` interface proves too verbose.
- Do not fully clone pi's unrestricted path behavior for all modes.

## Chosen Approach

Use pi-aligned defaults for interactive mode and stricter defaults for
non-interactive mode.

Interactive default active tools:

```text
read, write, edit, bash
```

Non-interactive default active tools:

```text
read, grep, find, ls, glob
```

When non-interactive mode is invoked with `--allow-mutating` or
`defaults.allow_mutating_tools = true`, its default active tools become:

```text
read, write, edit, bash
```

Explicit `--tools` always acts as an allowlist. If an allowlist contains a
mutating tool in non-interactive mode without mutating opt-in, startup should
return a clear configuration error instead of advertising the tool and denying
it later.

## Tool Policy

Introduce an application-level run mode:

```rust
enum RunMode {
    Interactive,
    NonInteractive,
}
```

Tool selection should be resolved from:

- run mode;
- CLI/config mutating opt-in;
- `--tools`;
- `--no-tools`;
- `--no-builtin-tools`.

The resolved active tool names should be computed before `CodingHarness`
constructs tool instances. `before_tool_call` should remain available for hooks,
but it should not be the primary mechanism for blocking default mutating tools
in non-interactive mode.

Tool selection precedence remains:

```text
--no-tools > --tools > --no-builtin-tools > default
```

`--no-builtin-tools` still resolves to no built-in tools because Phase 4
extension/custom tools do not exist yet.

## Path Policy

Replace the single workspace-only path validation rule with explicit path
policy:

```rust
enum PathPolicy {
    WorkspaceOnly,
    AllowOutsideWorkspace,
}
```

Path resolution should support:

- relative paths resolved against the tool cwd/workspace root;
- absolute paths;
- `~` expansion;
- optional pi-style `@path` prefix stripping.

Every file-tool result should include resolved path metadata:

- `resolved_path`;
- `workspace_root`;
- `inside_workspace`.

Policy by tool and mode:

| Mode | Tool | Path policy |
| --- | --- | --- |
| interactive | `read` | `AllowOutsideWorkspace` |
| interactive | `write`, `edit` | `WorkspaceOnly` |
| non-interactive | `read`, `write`, `edit` | `WorkspaceOnly` |
| both | `grep`, `find`, `ls`, `glob` | workspace-rooted search |

`bash` keeps `cwd = workspace_root`. It can still access paths outside the
workspace when enabled, so safety is controlled through active tool selection,
visible command text, timeout, and cancellation.

## CLI And Config

Keep `--allow-mutating`, but update its description and docs so it is clear
that interactive mode is allowed to mutate by default. The flag primarily
controls non-interactive mode and any explicitly restrictive tool policy.

Keep `defaults.allow_mutating_tools`, but interpret it as a default mutating
opt-in for non-interactive mode. It should not be required for ordinary
interactive coding.

README and spec updates should distinguish:

- available built-in tools: `read`, `write`, `edit`, `bash`, `grep`, `find`,
  `ls`, plus opi-native `glob`;
- interactive default active tools: `read`, `write`, `edit`, `bash`;
- non-interactive safe default active tools: `read`, `grep`, `find`, `ls`,
  `glob`;
- mutating opt-in behavior for non-interactive mode.

`docs/opi-spec.md` should also be updated to reflect that Phase 3 is complete
and Phase 4 is next, since the document still describes Phase 2 as the current
implementation state.

## Error Handling

Invalid combinations should fail before provider construction where possible.

Examples:

- `opi --non-interactive --tools bash "..."` without `--allow-mutating` should
  fail with a message explaining that `bash` requires mutating opt-in in
  non-interactive mode.
- `opi --json --tools write "..."` without mutating opt-in should fail the same
  way.
- `opi --tools read,unknown` should keep the existing behavior if currently
  ignored, or be tightened only if the implementation already has a local
  validation path. This design does not require changing unknown-tool behavior.

Tool execution errors should continue returning tool results, not process-level
configuration failures.

## Testing

Focused tests should cover:

- interactive default active tools are `read`, `write`, `edit`, `bash`;
- non-interactive default active tools are `read`, `grep`, `find`, `ls`,
  `glob`;
- non-interactive mutating opt-in enables `read`, `write`, `edit`, `bash`;
- explicit non-interactive mutating allowlist without opt-in fails before run;
- `--no-tools` and `--no-builtin-tools` still resolve to no built-in tools;
- `--tools read,grep` works in both modes;
- interactive `read` can read an absolute temp-file path outside the workspace;
- non-interactive/file policy rejects outside-workspace read;
- `write` and `edit` reject outside-workspace paths in both modes;
- `~` path expansion works for allowed reads;
- path metadata includes `inside_workspace`.

After implementation, run:

```sh
cargo test -p opi-coding-agent --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

## Documentation Updates

Update:

- `crates/opi-coding-agent/README.md`;
- `crates/opi-coding-agent/README.zh.md`;
- `docs/opi-spec.md`.

Documentation should avoid describing opi as default read-only. It should state
that interactive mode is a coding-agent mode with pi-style defaults, while
non-interactive mode is conservative unless mutating tools are explicitly
enabled.

## Risks

- Changing interactive defaults may surprise users who have relied on the old
  safety block. Mitigation: document the change clearly.
- Allowing interactive `read` outside the workspace increases accidental secret
  exposure risk. Mitigation: only `read` is relaxed, results record
  `inside_workspace`, and non-interactive remains workspace-only.
- If tool policy logic is split across CLI, harness, and hooks, behavior can
  drift again. Mitigation: centralize resolution in one policy module and test
  it directly.

## Acceptance Criteria

- Interactive default behavior matches pi's default active tool set.
- Non-interactive mode does not advertise or execute mutating tools unless
  explicitly opted in.
- Tool visibility and execution policy are consistent.
- Path policy is explicit and tested.
- Documentation reflects the new defaults and the intentional remaining
  differences from pi.
