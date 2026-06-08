# Productized Extensions and Package Ecosystem Design

## Overview

Phase 5 productizes the Phase 4 extensibility substrate without turning opi
into a TypeScript or npm clone of pi. The MVP goal is a complete local-first
package loop:

```text
package source
  -> add/remove/list/doctor
  -> manifest/filter
  -> minimal lock file
  -> resource discovery
  -> process JSONL adapter
  -> tools/commands/hooks/events/state
  -> diagnostics
```

The design follows pi's product idea of shareable packages, while keeping the
Rust implementation native to opi's crate boundaries and release model.

## Goals

- Let users add local and git-backed packages globally or per project.
- Preserve the existing `package.toml` resource discovery model and extend it
  into an installed-package workflow.
- Add an `opi package ...` MVP command group for package lifecycle management:
  `add`, `remove`, `list`, and `doctor`.
- Add a process JSONL adapter protocol so packages can provide runtime
  behavior without patching opi core.
- Bridge adapter-provided tools, commands, selected hooks, event observation,
  cancellation, and state into the existing runtime.
- Keep workflow-heavy capabilities such as MCP, sub-agents, plan mode, todos,
  and permission gates outside core as packages or extension examples.
- Provide strong diagnostics for source resolution, manifest parsing, lock
  drift, adapter startup, protocol mismatches, and resource containment errors.

## Non-Goals

- No npm registry install support in Phase 5.
- No package marketplace or gallery.
- No dynamic Rust library loading.
- No built-in Node.js or TypeScript `jiti` runtime.
- No hot reload.
- No runtime dynamic registration after adapter initialization.
- No inter-adapter event bus.
- No external provider streaming bridge.
- No custom TUI component protocol.
- No custom message renderer protocol.
- No package permission system or sandbox enforcement.
- No pi session v3 compatibility.
- No new shared `opi-types` crate.

## Current State

Phase 4 already provides:

- `opi-agent` extension traits for tools, commands, hooks, events, state,
  providers, and model overrides.
- `opi-coding-agent` resource discovery for extensions, packages, skills,
  prompt fragments, and themes.
- `package.toml` package composition for resource bundles.
- RPC and SDK command/event types, including `extension_command`.
- Example package directories for permission gates, protected paths, sub-agent,
  plan mode, todo, and MCP adapter workflows.

The missing product loop is installation, removal, listing, validation,
minimal lock state, executable adapters, cancellation, event observation, and
user-facing diagnostics.

## Architecture

Phase 5 adds product behavior in `opi-coding-agent` and keeps runtime semantics
in `opi-agent`.

| Module | Crate | Responsibility |
|---|---|---|
| Package CLI | `opi-coding-agent` | `opi package ...` commands and JSON output |
| Package Store | `opi-coding-agent` | source parsing, install directories, git clone/refresh, local references, lock files |
| Package Manifest | `opi-coding-agent` | parse and validate package metadata, filters, adapters, compatibility |
| Adapter Host | `opi-coding-agent` | spawn adapter processes and bridge them into the extension registry |
| Adapter Protocol Types | `opi-coding-agent` | JSONL command/event schema for external packages |
| Diagnostics | `opi-coding-agent` | startup and `doctor` reporting |

`opi-agent` must not know package installation details. It should only expose
the runtime concepts that a package adapter ultimately maps into: tools,
commands, hooks, state, providers, and model metadata.

Adapter protocol types start in `opi-coding-agent` because the process adapter
is a coding-agent product surface. They should move to `opi-agent` only after a
non-CLI embedder needs to host the same external adapter protocol.

## Package Store Model

Use separate files for user intent and resolved installation state.

| File | Location | Purpose |
|---|---|---|
| `packages.toml` | user config dir or `.opi/packages.toml` | declared packages and filters |
| `package-lock.toml` | user config dir or `.opi/package-lock.toml` | source path, git commit when available, cache path, manifest hash |
| `package.toml` | package root | package-owned resources, adapter entrypoint, compatibility |

Global scope writes under the opi user config directory. Project scope writes
under `.opi/` in the workspace. Project packages override global packages by
package identity.

The lock file is intentionally minimal. It is not a dependency solver and does
not model transitive package dependencies. It exists to make startup
diagnostics deterministic: opi can tell whether a git checkout, local path, or
package manifest changed since the last successful add or doctor run.

Project `packages.toml` may be committed when a team wants shared packages.
Project `package-lock.toml` should be committed only when reproducible package
checkout state is desired. Global package config and lock files are user-local
state and should not be committed.

## Package Sources

Phase 5 supports:

```text
/absolute/path/to/package
./relative/package
git:https://github.com/user/repo@ref
git:ssh://git@github.com/user/repo@ref
git:github.com/user/repo@ref
```

Local relative paths are resolved against the configuration file that declares
them. Git sources are cloned into a package cache under the selected scope.
Versioned git refs are pinned. Broad package update is deferred, but the lock
format should preserve enough source metadata for a later explicit update
command.

Phase 5 shells out to the `git` CLI for clone and ref resolution instead of
adding `git2` or `gix`. This keeps the first package-store slice small and
reuses the user's existing SSH and credential configuration. Missing `git`,
authentication failures, and disabled terminal prompts must surface as package
diagnostics.

Package identity rules:

| Source kind | Identity |
|---|---|
| local | canonical absolute path |
| git | normalized repository URL without ref |

Declaration identity is source-based. Discovered runtime/resource identity is
manifest-name-based. If two active declarations resolve to packages with the
same manifest `name` in the same precedence layer, startup and `doctor` report a
duplicate package error. If global and project packages have the same manifest
`name`, the project package wins.

## CLI Design

Add a command group rather than expanding top-level flags:

| Command | Behavior |
|---|---|
| `opi package add <source> [-l]` | add local or git source; default global, `-l` means project-local |
| `opi package remove <name-or-source> [-l]` | remove declaration; cache remains unless later pruning is added; ambiguous names produce an error |
| `opi package list [--json]` | list global and project packages with state, source, version, and diagnostics |
| `opi package doctor [--json]` | validate all package declarations, locks, manifests, resources, and adapters |

This maps to pi's `install`, `remove`, `list`, `update`, and `config` ideas,
but uses a Rust CLI namespace that keeps package lifecycle separate from
session, model, and run-mode flags.

`update`, `enable`, `disable`, and `info` are deferred until the store and
adapter host have proven stable. A removed package is the MVP disable path.

## Manifest V2

Extend the current flat `package.toml` rather than replacing it.

```toml
name = "todo"
description = "Todo package"
version = "0.1.0"
opi_version = ">=0.5,<0.7"

skills = ["todo"]
fragments = []
themes = []
extensions = ["todo"]

[adapter]
kind = "process-jsonl"
command = "todo-adapter"
args = []
protocol = "opi-extension-jsonl-v1"
timeout_ms = 30000
```

Relative adapter commands are resolved against the package root. Absolute
commands are used as given. PATH lookup is allowed only when the command has no
path separators, and `doctor` must report the resolved executable path.

Compatibility rules:

- Current flat manifests without `[adapter]` remain valid resource bundles.
- `opi_version` is advisory in 0.x but produces a diagnostic when incompatible.
- Resource include lists still narrow package-contained resources.
- Missing include targets remain errors.
- Path containment checks remain mandatory.
- Package permission declarations are deferred until there is an enforcement
  path or a stronger sandbox design. Phase 5 must not create a permission table
  that looks protective but only reports metadata.

## Runtime Adapter Protocol

Adapters are child processes that communicate through JSONL. The first protocol
version is `opi-extension-jsonl-v1`.

Startup:

```text
opi starts
  -> read declared packages
  -> discover package.toml
  -> start adapter process when declared
  -> send initialize
  -> receive capabilities
  -> register tools, commands, hooks, state handlers, optional model metadata
  -> run agent
  -> send shutdown before teardown
```

Phase 5 MVP capabilities:

| Capability | Required | Notes |
|---|---|---|
| tools | yes | adapter declares JSON Schema and executes calls |
| commands | yes | maps to slash commands and RPC `extension_command` |
| hooks | yes | before tool, after tool, prepare next turn, transform context |
| event_observer | yes | adapter receives selected `AgentEvent` values fire-and-forget |
| state | yes | serialize and restore adapter state |
| model_overrides | optional | static model metadata overrides only |

Event and hook scope:

| Surface | Phase 5 behavior |
|---|---|
| `before_tool_call` | blocking hook; timeout fails closed |
| `after_tool_call` | observational hook; timeout fails open |
| `prepare_next_turn` | may inject extra messages before the next model turn |
| `transform_context` | may transform app-level messages before provider conversion |
| `event_observer` | receives selected `AgentEvent` values fire-and-forget |
| session lifecycle control | deferred; adapters cannot cancel session switch, fork, compact, or shutdown |
| provider request/response interception | deferred |

Deferred capabilities:

- full provider streaming bridge;
- provider request/response interception;
- blocking session lifecycle hooks;
- custom TUI components;
- custom message renderers;
- hot reload;
- adapter dynamic registration after initialization;
- inter-adapter event bus;
- built-in adapter sandboxing;
- bundled Node or TypeScript runtime.

Provider registration is intentionally limited in the MVP. Static model
metadata can be represented as capabilities, but an adapter cannot provide a
streaming `Provider` implementation until a separate protocol defines request,
stream, cancellation, retries, usage, and provider error semantics.

## Adapter Host Bridge

The adapter host is implemented as a Rust bridge object in `opi-coding-agent`.
It maps a child process into the existing in-process extension contracts.

```text
ProcessAdapter
  implements Extension
  owns AdapterProcessHandle
  owns declared commands, hooks, model overrides, and state handlers

ProcessAdapterHooks
  implements AgentHooks
  wraps the base coding-agent hooks for transform_context only

ProcessAdapterTool
  implements Tool
  owns tool definition from adapter capabilities
  sends tool_call JSONL messages to AdapterProcessHandle
```

Bridge mapping:

| Runtime contract | Adapter message |
|---|---|
| `Extension::tools()` | capabilities `tools` create `ProcessAdapterTool` values |
| `Tool::execute()` | send `tool_call`, await `tool_result`, forward progress events when present |
| `Extension::on_command()` | send `command`, await `command_result` |
| `Extension::on_before_tool_call()` | send `hook` and await `hook_result` |
| `Extension::on_after_tool_call()` | send `hook`, timeout fail-open |
| `Extension::prepare_next_turn()` | send `hook`, convert returned messages into turn update |
| `AgentHooks::transform_context()` through `ProcessAdapterHooks` | send `hook`, convert returned messages |
| `Extension::on_event()` | send fire-and-forget `event`; do not await in the agent loop |
| `Extension::serialize_state()` | send `state_serialize`, await `state_result` |
| `Extension::restore_state()` | send `state_restore`, await acknowledgement |

Adapters declare which hooks they implement during initialization. The host must
skip IPC for undeclared hooks. This is required for performance: every
synchronous hook runs inside the agent loop and must not pay a JSONL round trip
when no adapter declared interest.

`ProcessAdapterHooks` is an opi-coding-agent bridge, not package installation
logic inside `opi-agent`. It should compose with the existing base hooks and
the in-process `ExtensionRegistry` hook wrapper. Phase 5 only bridges
`transform_context`; provider request interception and blocking session
lifecycle hooks need separate protocol designs and are deferred.

Every request/response message carries an `id`. The host owns id generation,
correlates responses, and times out outstanding calls. Tool execution stores
the adapter request id so a `CancellationToken` can be mapped to a `cancel`
message.

Adapter state is session-scoped in Phase 5 and flows through the existing
extension state hooks. Package-global state, cross-session adapter daemons, and
shared adapter caches are deferred because they need a separate ownership and
cleanup model.

## Adapter Messages

Core message shapes:

```json
{"type":"initialize","id":"1","protocol":"opi-extension-jsonl-v1","package":"todo"}
{"type":"capabilities","id":"1","tools":[],"commands":[],"hooks":["before_tool_call","event"]}
{"type":"tool_call","id":"2","tool":"todo_add","args":{"text":"write spec"}}
{"type":"tool_result","id":"2","content":[{"type":"text","text":"ok"}],"is_error":false}
{"type":"cancel","id":"2","reason":"user_abort"}
{"type":"command","id":"3","name":"todo/list","args":{}}
{"type":"command_result","id":"3","data":{"items":[]}}
{"type":"hook","id":"4","hook":"before_tool_call","tool":"bash","args":{}}
{"type":"hook_result","id":"4","action":"continue"}
{"type":"event","event":{"type":"turn_start","turn":1}}
{"type":"state_serialize","id":"5"}
{"type":"state_result","id":"5","state":{}}
{"type":"shutdown","id":"6","reason":"session_shutdown"}
```

`cancel` is sent for in-flight adapter requests whose corresponding
`CancellationToken` is cancelled. It is best effort: the adapter should stop
work and eventually return or close, but opi still enforces its local timeout.

`event` is fire-and-forget and must never block agent progress. If an adapter's
stdin backpressure would block event delivery, the host may drop event messages
and record a diagnostic.

Protocol negotiation is exact-match in Phase 5. If an adapter advertises a
different protocol than the package manifest or the host supports, the adapter
runtime is disabled and static resources still load.

## Adapter Failure Semantics

| Failure | Behavior |
|---|---|
| adapter spawn fails | package becomes degraded; static resources still load |
| initialize response times out | runtime adapter disabled; diagnostic explains timeout |
| protocol mismatch | runtime adapter disabled; `doctor` reports expected and actual protocol |
| tool call times out | return error tool result for that call |
| adapter crashes | mark runtime unavailable; pending calls fail with adapter-unavailable errors |
| before-tool hook times out | fail closed and block the tool |
| after-tool hook times out | fail open and record diagnostic |
| event delivery fails or backpressures | drop the event and record diagnostic |
| state serialization fails | continue shutdown but report session persistence diagnostic |

The adapter host must kill or reap child processes during shutdown. It must not
leave zombie processes after normal quit, Ctrl+C shutdown, or test teardown.

## Security Model

Packages are trusted code. The CLI and docs must say this directly.

Phase 5 security requirements:

- Store package source and lock data in human-readable TOML.
- Never log secrets from environment variables or provider config.
- Canonicalize local package paths and reject resource escapes.
- Show package adapter command, source, resolved executable path, and scope in
  `list` and `doctor`.
- Refuse to run adapters with unsupported protocols.
- Time out initialize, tool calls, hooks, and shutdown.
- Do not parse package permission declarations until a real enforcement path
  or sandbox design exists.

## Data Flow

Normal startup:

```text
resolve config
  -> load global packages.toml
  -> load project packages.toml
  -> merge declarations by identity and precedence
  -> read package-lock.toml
  -> resolve installed package roots
  -> parse package.toml
  -> compose resources
  -> start declared adapters in deterministic package order
  -> register adapter capabilities
  -> build CodingHarness
```

Adapter startup order is deterministic: sort by discovery precedence, then
package manifest name, then source identity. Phase 5 has no dependency graph
between adapters. If a package needs another package, it must fail with a clear
diagnostic rather than relying on startup order.

`opi package add`:

```text
parse source
  -> resolve package root or clone git source
  -> parse package.toml
  -> write declaration to packages.toml
  -> write package-lock.toml entry
  -> print installed package summary
```

`opi package doctor`:

```text
read declarations
  -> resolve locks
  -> parse manifests
  -> verify opi_version
  -> verify resources and path containment
  -> optionally start adapter in handshake-only mode
  -> emit table or JSON diagnostics
```

## Implementation Slices

| Order | Slice | Done when |
|---:|---|---|
| 1 | Package config and store | global/project declarations and locks read/write correctly, including Windows paths |
| 2 | Package CLI | add/remove/list/doctor work with `--json` where specified |
| 3 | Manifest V2 | current flat manifests still parse; adapter, filters, and `opi_version` parse and validate |
| 4 | Adapter protocol types | handshake, capabilities, events, errors, timeout, cancel, and shutdown are represented and documented |
| 5 | Adapter host | adapter tools, commands, hooks, event observer, cancellation, and state bridge into the existing extension registry |
| 6 | Runnable example packages | todo, permission-gate, and protected-paths demonstrate real process adapters |
| 7 | Docs and alignment | README, README.zh, opi-spec, opi-spec.zh, and pi-alignment-matrix describe the completed scope honestly |

## Testing Strategy

| Level | Coverage |
|---|---|
| unit | source parser, manifest parser, lock read/write, filter resolution, version constraint diagnostics |
| integration | `opi package add/remove/list/doctor` with temp directories |
| git fixture | local bare git repository for clone and ref pin behavior |
| adapter contract | mock adapter process for handshake, tool call, command, hook, event, cancel, timeout, crash, shutdown |
| harness | adapter-registered tool and command are visible through `CodingHarness` and RPC |
| docs guard | docs do not claim npm, marketplace, hot reload, custom TUI, or sandbox enforcement as complete |

After code changes, the normal gate remains:

```sh
cargo clippy --workspace --all-targets -- -D warnings
```

If a test file is created or modified, run the specific test while iterating.

## Documentation Updates

Phase 5 documentation must update:

- `README.md`
- `README.zh.md`
- `docs/opi-spec.md`
- `docs/opi-spec.zh.md`
- `docs/pi-alignment-matrix.md`
- package example READMEs for converted adapters

Documentation must clearly distinguish:

- resource-only packages;
- executable adapter packages;
- the absence of a package permission system in Phase 5;
- deferred npm and marketplace support.

## Compatibility and Migration

Existing example packages and user-created `package.toml` files using the
current flat schema should continue to work as resource-only packages. Manifest
V2 adds optional fields and tables. A package without `[adapter]` must never be
treated as a broken runtime package.

Because all extension and package APIs remain unstable 0.x surfaces, Phase 5
may introduce breaking changes where needed, but the CLI should report clear
migration diagnostics instead of failing silently.

## Success Criteria

Phase 5 is complete when:

1. A user can add a local package globally or per project.
2. A user can add a git package globally or per project.
3. Restarting opi loads declared package resources and adapters.
4. Adapter-provided tools can be called by the agent.
5. Adapter-provided commands can be dispatched through interactive/RPC command
   paths.
6. Adapter before-tool hooks can block a tool call.
7. Adapter event observers can receive agent events without blocking the agent.
8. Cancelling an adapter-backed tool sends a best-effort adapter `cancel`
   message and still enforces an opi-side timeout.
9. Adapter state can survive restart through the existing session/state path.
10. `opi package doctor` explains common source, lock, manifest, resource, and
   adapter failures.
11. Static resource-only packages still work.
12. Core crates do not absorb MCP, sub-agent, plan mode, todo, or permission
    gate product policy.

## Deferred Phase 6 Candidates

- npm package source support.
- package gallery metadata and marketplace discovery.
- package update, enable, disable, and info commands.
- package permission declarations plus enforcement.
- adapter hot reload.
- custom TUI component protocol.
- custom message renderer protocol.
- adapter event bus.
- dynamic registration after initialization.
- external provider streaming bridge.
- stronger sandbox or permission enforcement.
- Node or TypeScript runtime adapters as optional external packages.
