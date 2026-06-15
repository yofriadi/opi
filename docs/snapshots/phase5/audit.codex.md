# Phase 5 Audit - Codex

Date: 2026-06-09

## Scope

This audit reviewed Phase 5 against:

- `docs/snapshots/phase5/opi-impl-state.json`
- `docs/superpowers/specs/2026-06-08-productized-extensions-package-ecosystem-design.md`
- `docs/superpowers/plans/2026-06-08-opi-pi-alignment-remediation.md`

The review also inspected the relevant implementation in:

- `crates/opi-coding-agent/src/package_store.rs`
- `crates/opi-coding-agent/src/package_cli.rs`
- `crates/opi-coding-agent/src/package_discovery.rs`
- `crates/opi-coding-agent/src/adapter_protocol.rs`
- `crates/opi-coding-agent/src/adapter_host.rs`
- `crates/opi-coding-agent/src/adapter_extension.rs`
- `crates/opi-coding-agent/src/harness.rs`
- `crates/opi-agent/src/extension.rs`
- `crates/opi-agent/src/session.rs`
- `crates/opi-coding-agent/src/session_coordinator.rs`

No code changes were made. This is a static audit of implementation and test coverage.

## Executive Summary

Phase 5 has useful substrate: package manifest V2 parsing, resource composition,
adapter protocol serde, adapter host request correlation, direct adapter-to-extension
bridging, and runnable example adapter tests are present.

The productized package loop is not complete. The central Phase 5 user path,
`package source -> opi package add -> startup discovery -> adapter registration`,
is not wired end to end in production. The current implementation mostly proves
the pieces independently:

- `opi package add` records a declaration only.
- `package-lock.toml` is not written by the CLI.
- Git package sources are parsed but not cloned by the CLI.
- Normal harness startup does not read `packages.toml` or `package-lock.toml`.
- `start_adapters_from_packages()` is not called by production startup code.
- Adapter state can round-trip through `ExtensionRegistry` in tests, but it is not persisted through session JSONL and restored on resume.

Because of those gaps, Phase 5 should be treated as **partially implemented** rather than exited as a complete productized package ecosystem.

## Findings

### P0 - Installed packages are not connected to runtime startup

The design's MVP loop requires installed package declarations to become loaded resources and running adapters on restart. The implementation does not do that.

Evidence:

- `package_cli::cmd_add()` only validates `PackageSource::parse()` and appends a `PackageDeclaration` to `packages.toml`.
- `CodingHarness::discover_resources()` discovers packages from configured discovery layers (`config.packages.paths`, user `packages/`, project `.opi/packages/`, explicit paths), not from `PackageStore::read_declarations()` or lock entries.
- `start_adapters_from_packages()` exists in `adapter_extension.rs`, but repository search finds it used only by tests and its own definition, not by production startup.

Impact:

- `opi package add ./pkg -l` creates `.opi/packages.toml`, but a later `opi` run does not load that declaration as a package layer.
- Packages installed through the new CLI do not provide tools, commands, hooks, events, or state in normal interactive/non-interactive/RPC runs.
- Phase 5 success criteria 1, 2, 3, 4, 5, 6, 7, and 11 are only satisfied for manually configured package paths or direct test setup, not for the package CLI workflow.

Recommendation:

Add a production package resolver used by harness construction:

1. Read global and project `packages.toml`.
2. Resolve declarations using lock data and source parsing.
3. Clone or refresh git sources when needed.
4. Parse `package.toml` into `PackageResource`.
5. Merge declaration packages with existing resource discovery layers.
6. Call `start_adapters_from_packages()` before `CodingHarness` finalizes tools, hooks, commands, model metadata, and resource diagnostics.
7. Add an E2E test that installs a local adapter package with `opi package add -l`, starts a harness or subprocess, and proves the adapter tool/command is visible without manually passing `config.packages.paths`.

### P0 - `opi package add/remove/list/doctor` is declaration-only and below the design contract

The CLI MVP in the design is a package lifecycle surface. The implementation is closer to a TOML declaration editor.

Evidence:

- `cmd_add()` does not require the local path to exist, does not parse `package.toml`, does not write `package-lock.toml`, does not compute a manifest hash, and does not call `PackageStore::git_clone()`.
- `cmd_remove()` removes only declarations whose `source` exactly matches `name_or_source`; it does not remove by manifest name and cannot detect ambiguous names.
- `resolve_scope()` makes `list` and `doctor` always use project scope, while the design says they should list or validate global and project packages.
- `cmd_doctor()` validates only that a path exists, `package.toml` exists, and the file parses as generic TOML. It does not use `PackageManifest::from_toml()`, does not check `opi_version`, resource containment, lock drift, git commit state, adapter command resolution, adapter protocol, or handshake behavior.

Impact:

- The README example `opi package remove todo` is misleading unless the literal source string is `todo`.
- Git-backed packages cannot be installed through the CLI even though git source parsing and clone primitives exist.
- `doctor` can report success for manifests that Phase 5 manifest parsing would reject.
- `doctor` does not meet success criterion 10: explaining source, lock, manifest, resource, and adapter failures.

Recommendation:

Promote package CLI operations from declaration editing to lifecycle operations:

- `add`: resolve source, validate manifest with `PackageManifest::from_toml()`, clone git sources, pin commit, compute `manifest_sha256`, and write/update lock entries.
- `remove`: support source identity and manifest name removal, with ambiguity diagnostics.
- `list`: merge global and project declarations and include state, scope, source, package name, version, resolved root, adapter command, and diagnostics.
- `doctor`: run the same resolver as startup plus deeper checks for lock drift, resource containment, opi version advisory diagnostics, adapter command resolution, and optional handshake-only adapter startup.

### P1 - Adapter state does not survive restart through session persistence

The adapter bridge implements `serialize_state()` and `restore_state()`, and tests prove isolated registry round-trips. Production session persistence does not call those methods.

Evidence:

- `ExtensionRegistry::serialize_states()` and `restore_states()` exist in `opi-agent`.
- `ProcessAdapter` implements `serialize_state()` and `restore_state()` via `state_serialize` and `state_restore` adapter messages.
- `SessionEntry` contains only `Message`, `Compaction`, and `Leaf`.
- `SessionCoordinator` persists LLM messages, compaction entries, and leaf pointers only.
- No production call site invokes `serialize_states()` or `restore_states()` during session write, shutdown, resume, fork, or clone.

Impact:

- Adapter state can be manually serialized in tests, but it cannot survive an `opi` restart through the documented session/state path.
- Phase 5 success criterion 9 is not met.
- Stateful examples such as todo, permission-gate audit logs, and protected-paths audit logs do not persist across normal session resume unless manually bridged by a caller outside the current production path.

Recommendation:

Define a session persistence shape for extension state, then wire it into `SessionCoordinator` and resume:

- Add a session entry or metadata payload for extension state snapshots.
- Serialize extension state at deterministic boundaries such as turn end and graceful shutdown.
- Restore extension state after adapter startup and before agent turns resume.
- Add tests that start an adapter, mutate state, persist a session, resume it with a fresh adapter process, and verify restored state.

### P1 - Adapter hook coverage is narrower than the Phase 5 design

The design includes `before_tool_call`, `after_tool_call`, `prepare_next_turn`, and `transform_context` hook mappings. The adapter bridge implements only the first two plus event observation.

Evidence:

- `ProcessAdapter` implements `on_before_tool_call()`, `on_after_tool_call()`, `on_event()`, `on_command()`, and state methods.
- `ProcessAdapter` does not implement `Extension::prepare_next_turn()`.
- There is no `ProcessAdapterHooks` implementation in production code.
- `ExtensionRegistry::CompositeHooks::transform_context()` delegates only to the base hooks; extensions have no transform-context hook surface.
- `docs/superpowers/specs/...productized-extensions-package-ecosystem-design.md` explicitly lists `prepare_next_turn` and `transform_context` in the Phase 5 adapter bridge.

Impact:

- Packages cannot inject next-turn messages through adapter hooks.
- Packages cannot transform app-level context before provider conversion.
- Phase 5 task 5.6 and the design's runtime adapter bridge are overstated unless the intended MVP scope is narrowed.

Recommendation:

Either implement the missing mappings or amend the Phase 5 ledger/docs to state that adapter hooks are limited to before/after tool hooks and events in this slice. If implemented, add protocol payload/response conversion tests for:

- `hook = "prepare_next_turn"` returning extra messages.
- `hook = "transform_context"` returning transformed `AgentMessage` values.
- Skip behavior when those hooks are undeclared.

### P1 - Adapter process diagnostics are not surfaced consistently

The host implements timeouts and request correlation, but several diagnostic promises from the design are not yet visible to users.

Evidence:

- `AdapterHost::send_event()` silently drops event writes on timeout or failure.
- The design says dropped events should record diagnostics.
- `AdapterHost::shutdown_inner()` sends a shutdown message and then immediately kills and waits for the child, rather than allowing a bounded graceful exit window.
- Startup diagnostics from `start_adapters_from_packages()` are returned as a vector, but production startup does not call this function, so users never see those diagnostics for installed packages.

Impact:

- Event observer failures are invisible.
- Adapter shutdown semantics are harsher than documented and may prevent adapter cleanup logic from running.
- Adapter startup failures are useful in tests but not in the normal product path.

Recommendation:

Add a diagnostics sink shared by package discovery, adapter startup, event delivery, and doctor. Let `shutdown_inner()` wait briefly for natural process exit after sending shutdown before killing. Expose diagnostics in `package doctor`, startup metadata, and RPC session info.

### P2 - Source identity and path handling need hardening

Several accepted evaluator notes remain real product risks.

Evidence:

- `PackageSource::identity_key()` returns raw local paths, but the design says local identity should be a canonical absolute path.
- `PackageSource::parse()` uses the last `@` to split git refs, so `git:ssh://git@github.com/user/repo` without an explicit ref can be misparsed.
- `resolve_adapter_command()` joins relative commands to the package root but does not normalize or reject `..` escapes before spawn.
- `cmd_doctor()` does not report resolved adapter executable paths.

Impact:

- Duplicate local declarations can bypass identity if written with different relative spellings.
- SSH git URLs without refs can resolve incorrectly.
- A relative adapter command can escape the package root, which is surprising even under the trusted-package security model.
- Users cannot inspect the exact adapter command path through doctor/list as required by the design.

Recommendation:

Canonicalize local package identities during declaration resolution. Replace ad hoc git source splitting with URL-aware parsing or require/refuse ambiguous SSH no-ref forms explicitly. Normalize adapter command paths and reject relative path escapes unless the command is absolute or a bare PATH lookup. Include resolved executable paths in list/doctor output.

## Success Criteria Trace

| Design criterion | Audit result | Notes |
|---|---|---|
| Add local package globally or per project | Partial | Declaration is written; package is not resolved, validated, locked, or loaded from that declaration. |
| Add git package globally or per project | Not met | Git source parses and `git_clone()` has tests, but CLI add does not clone or lock git packages. |
| Restarting opi loads declared resources and adapters | Not met | Harness startup does not read installed package declarations or call adapter startup. |
| Adapter tools can be called by the agent | Partial | Works for directly registered adapters in tests, not for packages installed through CLI. |
| Adapter commands dispatch through interactive/RPC paths | Partial | Registry/RPC dispatch exists; installed adapter startup is missing. |
| Before-tool hooks can block calls | Partial | Direct registry tests pass; installed package path is missing. |
| Event observers are nonblocking | Partial | Fire-and-forget exists; dropped event diagnostics are missing. |
| Cancelling adapter-backed tools sends cancel | Partial | Cancellation bridge exists; pending-map race is acknowledged future risk. |
| Adapter state survives restart | Not met | No production session persistence integration. |
| `package doctor` explains common failures | Not met | Doctor checks only path existence and generic TOML parse. |
| Static resource-only packages still work | Partial | Works through configured package paths and package directories; not through installed declarations. |
| Workflow-heavy features stay outside core | Met | MCP, sub-agent, plan mode, todo, and permission-gate remain examples/packages. |

## Test Coverage Assessment

Strong coverage:

- Manifest V2 parsing and compatibility checks.
- Package resource composition from package directories.
- Adapter protocol serde round-trips.
- Adapter host handshake, request correlation, timeout, crash, cancel, state, and shutdown primitives.
- Direct adapter-to-extension bridge behavior.
- Example adapter package behavior when `PackageResource` values are supplied directly.
- Documentation guards that prevent overstating npm, marketplace, hot reload, provider streaming adapters, custom TUI adapters, or package permission enforcement.

Coverage gaps:

- No E2E test from `opi package add` to harness startup.
- No test proves `packages.toml` declarations are loaded into `CodingHarness`.
- No test proves `package-lock.toml` is written or consumed by the CLI lifecycle.
- No test proves a git package can be installed through the CLI.
- No test proves `package doctor` uses manifest V2 validation or adapter handshake diagnostics.
- No test proves extension/adapter state persists through session JSONL resume.
- No test covers adapter `prepare_next_turn` or `transform_context`.

## Recommended Remediation Order

1. Build the installed-package resolver and use it in production harness startup.
2. Upgrade `opi package add/remove/list/doctor` to operate on resolved packages and lock state.
3. Wire `start_adapters_from_packages()` into interactive, non-interactive, and RPC harness construction.
4. Persist and restore extension state through sessions.
5. Either implement adapter `prepare_next_turn` and `transform_context` or narrow the Phase 5 docs/ledger.
6. Add diagnostics for event drops, adapter command resolution, lock drift, and adapter startup.
7. Harden local identity canonicalization, SSH git parsing, and relative adapter command containment.

## Exit Recommendation

Do not mark Phase 5 as fully exited yet. Mark it as **substrate complete, product loop incomplete** until at least these gates pass:

- `opi package add ./examples/todo -l` writes declaration and lock state.
- A fresh `opi` harness startup from the same workspace discovers that installed package without `config.packages.paths`.
- The installed adapter registers its command/tool/hook into the runtime.
- `opi package doctor --json` reports manifest V2, resource, lock, and adapter diagnostics.
- Adapter state persists across a session resume with a fresh adapter process.

