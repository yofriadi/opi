# Phase 5 Audit — Opus 4.6

Date: 2026-06-09

Auditor model: Claude Opus 4.6

## Scope

This audit reviews Phase 5 against three reference documents:

- `docs/snapshots/phase5/opi-impl-state.json` — task graph, DoD, evaluator findings, and verification evidence for tasks 5.1–5.9.
- `docs/superpowers/specs/2026-06-08-productized-extensions-package-ecosystem-design.md` — the governing design spec with 12 success criteria, data flows, adapter protocol contract, and security model.
- `docs/superpowers/plans/2026-06-08-opi-pi-alignment-remediation.md` — Task 4 extension/package executable composition path and Task 9 final gates.

The review inspected production source in:

- `crates/opi-coding-agent/src/package_store.rs`
- `crates/opi-coding-agent/src/package_cli.rs`
- `crates/opi-coding-agent/src/package_discovery.rs`
- `crates/opi-coding-agent/src/adapter_protocol.rs`
- `crates/opi-coding-agent/src/adapter_host.rs`
- `crates/opi-coding-agent/src/adapter_extension.rs`
- `crates/opi-coding-agent/src/harness.rs`
- `crates/opi-coding-agent/src/main.rs`
- `crates/opi-agent/src/extension.rs`
- `crates/opi-agent/src/session.rs`
- `crates/opi-coding-agent/src/session_coordinator.rs`

and all Phase 5 test files (9 test modules, 2 test binaries).

No code changes were made. This is a static audit of implementation fidelity, test coverage, and product loop completeness.

This audit was conducted independently from `audit.codex.md`. Cross-validation with that audit appears in section 6.

## Executive Summary

Phase 5 delivers substantial infrastructure: package source parsing, declaration storage, manifest V2 with adapter declarations, a complete JSONL adapter protocol with serde round-trip coverage, an adapter host with correlated requests/timeouts/cancellation/crash handling, a `ProcessAdapter` bridge mapping adapter capabilities to `Extension` trait methods, runnable example adapters, and documentation guard tests.

The product loop is not closed. The central user path from the design spec — `opi package add <source>` causes a declaration to be written; restarting `opi` discovers that declaration, resolves the package, starts the adapter, and registers its tools/commands/hooks — does not execute in production. The pieces exist independently and pass their isolated task gates, but the wiring between `PackageStore` declarations and `CodingHarness` resource discovery is absent from production `main.rs` and harness construction paths.

Additionally, two of the four design-specified adapter hook mappings (`prepare_next_turn`, `transform_context`) are not implemented, the `ProcessAdapterHooks` type from the design does not exist, adapter state cannot survive session restart through JSONL persistence, and adapter shutdown kills the child process immediately without a graceful exit window.

Phase 5 should be classified as **substrate complete, product loop incomplete**.

## Task-by-Task DoD Review

### 5.1 — Package store and source model

**Status in ledger:** passing (21/21 tests, evaluator passed)

**DoD:** "Local and git sources parse deterministically; global/project packages.toml and package-lock.toml read/write through temp directories; lock entries record source path, optional git commit, cache path, and manifest hash; tests cover Windows-style paths and a local bare git repository clone/ref-pin fixture without touching real user config."

**Audit finding: DoD is met for the store layer in isolation.**

The `PackageStore` API correctly reads/writes declarations and lock entries through temp directories. Git clone with ref pinning is tested against a bare repository fixture. Windows path handling is covered.

Evaluator notes worth tracking:

- "SSH @ misparse: noted for future" — `PackageSource::parse()` uses `rfind('@')` to split ref from URL, so `git:ssh://git@github.com/user/repo` without an explicit ref splits at the `@` in the username. This is a latent bug, not a design observation. It will produce wrong results the first time a user adds an SSH package without a ref.

**Evidence reliability:** Tests verify store I/O correctly. The git_clone test uses a real bare repo fixture. However, the DoD does not require that production code calls `read_lock()`, `write_lock()`, or `git_clone()` — and in fact no production code does. The DoD is scoped to the store primitive, which is met, but this creates a false confidence that the lock/clone machinery is ready for product use.

### 5.2 — Package CLI MVP

**Status in ledger:** passing (20/20 tests, evaluator passed)

**DoD:** "opi package add/remove/list/doctor works before provider construction, supports global and project scope with -l, supports JSON output for list and doctor, never reads real user config during tests, and subprocess E2E coverage asserts stdout, stderr, and exit code for package command behavior."

**Audit finding: DoD is partially met. Functional gap in list/doctor scope.**

- `add`/`remove` support global and project scope via `-l`. Met.
- `list` and `doctor` always use project scope (hardcoded in `resolve_scope()`). The DoD says "supports global and project scope with -l" without specifying per-command, but the design spec explicitly says `list` and `doctor` should cover "global and project packages." This is a gap.
- JSON output exists for list and doctor. Met.
- Subprocess E2E tests exist. Met.

Evaluator notes worth tracking:

- "list/doctor global scope: noted for future, not MVP scope" — the design spec lists this as MVP behavior, not future work. The evaluator's scope judgment conflicts with the design.
- "source validation: fixed" — confirmed, `PackageSource::parse()` is called in `cmd_add`.
- "JSON format inconsistency: noted for future" — `list --json` emits one object per line (`{"source": "..."}`), not a JSON array. `doctor --json` emits a JSON array. This inconsistency is minor but real.

**Evidence reliability:** Tests verify CLI parsing and declaration I/O. The subprocess E2E tests build the `opi` binary and run `package add/list/doctor/remove`. However, no test verifies that `cmd_add` produces a declaration that a subsequent `opi` run can discover — the E2E is confined to the package CLI, not the full product loop.

### 5.3 — Manifest V2 compatibility

**Status in ledger:** passing (59/59 tests, evaluator passed)

**DoD:** "Existing flat manifests still parse; optional opi_version and [adapter] parse; relative adapter command resolution, PATH command resolution, and opi_version compatibility diagnostics are specified and tested; missing resources and path containment behavior remain unchanged."

**Audit finding: DoD is met.**

The 17 new V2 tests cover adapter parsing, command resolution modes (relative, absolute, bare), opi_version constraint matching, backward compatibility with all 6 example manifests, and error cases (unsupported kind, unsupported protocol, empty command). The 41 existing discovery tests remain green.

**Evidence reliability:** Strong. Tests use `PackageManifest::from_toml()` with realistic manifest content and verify both valid and invalid cases. The backward-compatibility test reads all `examples/*/package.toml` files from the repository.

### 5.4 — Adapter protocol types

**Status in ledger:** passing (24/24 tests, evaluator passed)

**DoD:** "Protocol serde supports initialize/capabilities/tool/command/hook/event/state/cancel/shutdown messages; unknown protocol is rejected; JSONL messages round-trip without provider access; the adapter JSONL protocol has a minimal crate-local reference documenting message shapes, version negotiation, and failure semantics."

**Audit finding: DoD is met for the serde layer.**

All 9 host-to-adapter and 6 adapter-to-host message types have serde round-trip tests. Unknown message types are rejected. JSONL line-oriented round-trips are verified. Module-level documentation exists.

Design observation: the design spec lists `prepare_next_turn` and `transform_context` as Phase 5 MVP hook names that should appear in `hook` messages. The protocol types support arbitrary `hook` strings, so this is not a protocol-type gap — but the bridge never sends these hook types (see 5.6).

**Evidence reliability:** Protocol types are pure serde; the tests are deterministic and correct.

### 5.5 — Adapter process host

**Status in ledger:** passing (16/16 tests, evaluator passed)

**DoD:** "Host starts a child process, performs initialize/capabilities handshake, sends correlated requests, times out requests, sends best-effort cancel, drops event messages under backpressure, reports adapter crash/unavailable states, and reaps the child on shutdown."

**Audit finding: DoD is met with a shutdown semantics caveat.**

The `AdapterHost` correctly spawns child processes, performs handshake with timeout, correlates requests by ID, handles request timeouts, sends best-effort cancel, fires events without blocking, and handles adapter crashes. The stderr drain task prevents pipe deadlock.

Shutdown concern: `shutdown_inner()` sends a shutdown message with a 200ms write timeout, then immediately calls `child.kill().await`. The design spec says the host should "kill or reap child processes during shutdown" and lists "adapter spawn fails → package becomes degraded" — it does not explicitly require a graceful exit window. However, a 200ms write + immediate kill means an adapter's shutdown handler cannot execute cleanup logic that takes more than the time between receiving the message and being killed. The design's failure semantics table says "state serialization fails → continue shutdown but report session persistence diagnostic" — implying the host should wait for state serialization to complete before killing.

Evaluator notes worth tracking:

- "stderr deadlock: fixed" — confirmed, stderr drain task added.
- "dead AdapterExited variant: fixed" — exit code detection in handshake confirmed.

**Evidence reliability:** The `adapter_host_mock` test binary provides realistic adapter behavior (echo, hang, crash modes). Tests cover the documented host contract thoroughly.

### 5.6 — Adapter runtime bridge

**Status in ledger:** passing (16/16 tests, evaluator passed)

**DoD:** "Adapter capabilities become runtime tools, commands, selected hooks, event observers, session-scoped state serialize/restore handlers, cancellation bridge, static model overrides, and documented bridge semantics through existing extension/hook contracts."

**Audit finding: DoD is partially met. Two design-specified hook mappings are missing.**

Implemented bridge mappings:

- `capabilities.tools` → `Extension::tools()` via `ProcessAdapterTool`
- `tool_call`/`tool_result` → `Tool::execute()` with cancellation
- `command`/`command_result` → `Extension::on_command()`
- `hook`/`before_tool_call` → `Extension::on_before_tool_call()` (fail-closed)
- `hook`/`after_tool_call` → `Extension::on_after_tool_call()` (fail-open)
- `event` → `Extension::on_event()` (fire-and-forget)
- `state_serialize`/`state_restore` → `serialize_state()`/`restore_state()`
- `model_overrides` → `Extension::model_overrides()` (metadata only)
- `cancel` → `CancellationToken` bridge in tool execute

Missing bridge mappings (design spec, "Adapter Host Bridge" section):

- `prepare_next_turn` — design says "send hook, convert returned messages into turn update." `ProcessAdapter` does not override `Extension::prepare_next_turn()`, which returns `None` by default. No IPC message is sent.
- `transform_context` — design says this flows through `ProcessAdapterHooks: AgentHooks`. `ProcessAdapterHooks` does not exist in the codebase. `CompositeHooks::transform_context()` delegates only to the base hooks; extensions have no `transform_context` surface.

The DoD says "selected hooks" which could be read as a subset. However, the design spec's "Phase 5 MVP capabilities" table explicitly lists all four hooks as "Required: yes" and the bridge section maps all four. The ledger DoD also says "documented bridge semantics" — the module docs do not mention which hooks are intentionally excluded.

Evaluator notes worth tracking:

- "model_overrides semantic mismatch: noted for future" — `AdapterModelOverride.tools` is parsed but ignored; the model is registered with `model` as both `provider_id` and `ModelInfo.id`. This is acknowledged as a simplification.
- "unbounded on_event tasks: noted for future" — each event spawns a tokio task without backpressure. For a well-behaved adapter this is fine; for a misbehaving one it could leak tasks.
- "block_in_place panic risk: fixed" — doc comment added about runtime requirements.

**Evidence reliability:** Bridge tests use the mock adapter binary and verify Extension trait methods. However, no test covers the missing hooks, and no test runs an adapter tool inside `agent_loop()` to verify end-to-end tool call flow through the agent runtime.

### 5.7 — Harness and startup integration

**Status in ledger:** passing (16/16 tests, evaluator passed)

**DoD:** "Startup reads global and project package stores with deterministic precedence, composes package resources, starts adapters in deterministic order, merges adapter tools/commands/hooks/state into the harness, preserves --no-tools and --no-builtin-tools adapter filtering semantics, and reports adapter diagnostics through existing resource metadata and RPC session_info."

**Audit finding: DoD is met only in the test environment. Production startup does not execute the described flow.**

The test file `harness_resource_integration.rs` demonstrates the full flow: `start_adapters_from_packages()` → `ExtensionRegistry` → `CodingHarness::builder().extension_registry(registry)` → adapter tools visible alongside builtins. Tool filtering, deterministic ordering, diagnostics, and RPC metadata all work correctly in tests.

**The critical gap:** Production `main.rs` constructs `CodingHarness` via `new_with_hooks_and_resume_tool_config()` or `CodingHarness::builder().build()`. Neither path calls `start_adapters_from_packages()`. Neither path reads `packages.toml` declarations. `harness.rs` does not import `PackageStore`. The `discover_resources()` method reads package directories from filesystem scan paths (`~/.config/opi/packages/`, `.opi/packages/`, `config.packages.paths`) — not from `PackageStore` declarations written by `opi package add`.

This means the DoD's "startup reads global and project package stores" is only satisfied in explicitly constructed test scenarios where the caller manually calls `start_adapters_from_packages()` and injects the registry. It is not satisfied for any of the three production run modes (interactive, non-interactive, RPC).

Evaluator notes worth tracking:

- "duplicate resolve_adapter_command: fixed" — duplicate function removed.
- "Windows portability in tests: fixed" — `cfg!(windows)` guards added.
- "registration error as string: noted for future" — `PackageStoreError::Git` is used as a carrier for non-git errors. Minor type abuse.

**Evidence reliability:** The tests verify what they claim. The gap is that the tests verify an explicit manual wiring path, while the DoD reads as describing production startup behavior. This is the most significant audit finding.

### 5.8 — Runnable example adapter packages

**Status in ledger:** passing (16/16 tests, evaluator passed)

**DoD:** "todo, permission-gate, and protected-paths examples declare process adapters and can be exercised in tests without Node, npm, or live providers."

**Audit finding: DoD is met with a distribution caveat.**

All three example packages have `package.toml` manifests with `[adapter]` sections declaring `kind = "process-jsonl"` and `command = "package_adapter_example"`. The `package_adapter_example` test binary implements all three adapter modes. Tests prove tool calls, commands, hooks, state, and registry integration.

Distribution concern: `package_adapter_example` is a `[[test]]` target in `Cargo.toml`, not a `[[bin]]` target. It is built as `target/debug/deps/package_adapter_example-<hash>` during `cargo test`. The `examples/todo/package.toml` declares `command = "package_adapter_example"` which is only resolvable if the test binary is on PATH or the test helper resolves it. A user cloning the repo and running `opi package add ./examples/todo -l` followed by `opi` would get an "adapter spawn failed" error because the command is not discoverable.

This is acceptable for the Phase 5 MVP since the examples are developer-facing test demonstrations, not user-distributable packages. But it should be documented.

**Evidence reliability:** Tests build synthetic `PackageResource` values pointing to the test binary's absolute path, bypassing the manifest `command` field resolution. The test named `example_manifests_backward_compat` in `package_manifest_v2.rs` verifies that the manifests parse correctly, but does not verify that the declared command can be resolved at runtime from the example directory.

### 5.9 — Documentation, alignment, and guards

**Status in ledger:** passing (1770/1770 workspace tests, 15 guard tests, evaluator not required)

**DoD:** "User docs describe the Phase 5 MVP truthfully; README.md/README.zh.md and docs/opi-spec.md/docs/opi-spec.zh.md are synchronized; docs guard tests reject claims that npm, marketplace, hot reload, provider streaming adapters, custom TUI adapters, or package permission enforcement are complete; final Phase 5 workspace gates pass."

**Audit finding: DoD is met for the guard tests. CHANGELOG has a gap.**

Guard tests (7 negative, 6 positive, 2 sync) correctly enforce that docs do not overclaim npm, marketplace, hot reload, streaming adapters, TUI adapters, or permission enforcement. Positive guards verify that docs mention the package CLI, process adapters, Phase 5, and the adapter protocol.

CHANGELOG gap: `CHANGELOG.md` `[Unreleased]` contains entries for Phase 4 remediation work (fork/branch commands, extension_command, provider profiles, session parent_id/leaf, agent_loop module move, TUI CJK fix) but zero Phase 5 entries. None of the following Phase 5 features appear in the changelog:

- `opi package add/remove/list/doctor` CLI
- Package store with declarations and lock files
- Manifest V2 with `[adapter]` and `opi_version`
- Adapter protocol `opi-extension-jsonl-v1`
- Adapter process host
- Adapter-to-extension bridge
- Example adapter packages

This is a documentation completeness gap, not a guard test failure (no guard checks CHANGELOG).

**Evidence reliability:** Guard tests are string-presence/absence checks on documentation files. They cannot verify semantic accuracy (e.g., whether the README's description of `opi package add` matches the actual `cmd_add` behavior). The sync guards verify EN/ZH parity for package CLI and Phase 5 mention, which is valuable.

## Success Criteria Trace

The design spec lists 12 success criteria. This section traces each against the implementation.

| # | Criterion | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | A user can add a local package globally or per project. | **Partial** | `cmd_add` writes a declaration to `packages.toml` in the selected scope. However, the declaration is not resolved: the local path is not checked for existence, `package.toml` is not parsed, lock state is not written, and the package is not loaded on the next `opi` run. |
| 2 | A user can add a git package globally or per project. | **Not met** | `PackageSource::parse()` accepts `git:` sources and `PackageStore::git_clone()` is implemented. But `cmd_add` does not call `git_clone()`, does not write lock entries, and does not pin a commit. The CLI syntax accepts git sources; the behavior is declaration-only. |
| 3 | Restarting opi loads declared package resources and adapters. | **Not met** | `CodingHarness` startup (`discover_resources()`) scans filesystem directories, not `PackageStore` declarations. `start_adapters_from_packages()` is not called from `main.rs`. A package added via CLI is invisible to the next `opi` run. |
| 4 | Adapter-provided tools can be called by the agent. | **Partial** | `ProcessAdapterTool` implements `Tool::execute()` and works correctly when the adapter is registered via `ExtensionRegistry`. In tests, adapter tools appear in the harness. In production, no adapter is ever started from installed packages. |
| 5 | Adapter-provided commands dispatch through interactive/RPC paths. | **Partial** | `ProcessAdapter::on_command()` dispatches to the adapter process. `ExtensionRegistry` command dispatch works in tests and through RPC `extension_command`. In production, no adapter is registered from installed packages. |
| 6 | Adapter before-tool hooks can block a tool call. | **Partial** | `ProcessAdapter::on_before_tool_call()` sends a `hook` message and blocks on `"block"` response. `CompositeHooks` chains extension hooks after base hooks. Works correctly in tests. In production, no adapter is registered from installed packages. |
| 7 | Adapter event observers can receive agent events without blocking the agent. | **Partial** | `ProcessAdapter::on_event()` spawns an async task to call `send_event()`. Event delivery uses a 100ms write timeout and silently drops on failure. The design says dropped events should "record a diagnostic" — they do not. |
| 8 | Cancelling an adapter-backed tool sends a best-effort cancel message and still enforces an opi-side timeout. | **Met** (in isolation) | `ProcessAdapterTool::execute()` uses `tokio::select!` on the cancellation token and calls `host.cancel()`. The host sends a `cancel` message with 100ms timeout and always returns `Ok`. The adapter-side timeout is enforced by the host's `send_request` wrapper. |
| 9 | Adapter state can survive restart through the existing session/state path. | **Not met** | `ProcessAdapter` implements `serialize_state()` and `restore_state()` via adapter IPC. `ExtensionRegistry` has `serialize_states()` and `restore_states()`. But `SessionEntry` has only `Message`, `Compaction`, and `Leaf` variants — no extension state entry type. `SessionCoordinator` never calls `serialize_states()` or `restore_states()`. Adapter state cannot survive session restart. |
| 10 | `opi package doctor` explains common source, lock, manifest, resource, and adapter failures. | **Not met** | `cmd_doctor` checks: path exists → `package.toml` exists → `toml::from_str::<toml::Value>()` succeeds. It does not use `PackageManifest::from_toml()`, does not check `opi_version`, does not verify resource containment, does not read lock files, does not check lock drift, does not resolve adapter commands, does not attempt adapter handshake, and does not report resolved executable paths. |
| 11 | Static resource-only packages still work. | **Partial** | Packages in filesystem scan directories (`~/.config/opi/packages/`, `.opi/packages/`, `config.packages.paths`) work correctly — they are discovered, composed, and their resources appear in the system prompt. Packages installed via `opi package add` are not discovered by the scan. |
| 12 | Core crates do not absorb MCP, sub-agent, plan mode, todo, or permission gate product policy. | **Met** | All workflow-heavy capabilities remain as example packages under `examples/`. No built-in core command implements MCP, sub-agent, plan mode, todo, or permission policy. |

**Summary: 1 Met, 1 Met (in isolation), 5 Partial, 4 Not met, 1 Partial.**

## Findings

### P0-1: Installed packages not connected to runtime startup

**Severity: P0 — product loop does not close**

The design's "Normal startup" data flow is:

```
resolve config → load global packages.toml → load project packages.toml
→ merge declarations → read package-lock.toml → resolve installed package roots
→ parse package.toml → compose resources → start declared adapters
→ register adapter capabilities → build CodingHarness
```

Production startup (`main.rs` → `CodingHarness::new_with_*`) does:

```
resolve config → scan ~/.config/opi/packages/ directory
→ scan .opi/packages/ directory → scan config.packages.paths
→ parse package.toml → compose resources → build CodingHarness
```

The divergence: production startup scans filesystem directories for package.toml files but never reads PackageStore declarations from packages.toml files. `start_adapters_from_packages()` is defined in `adapter_extension.rs` and used exclusively by test files (`harness_resource_integration.rs`, `example_adapters.rs`).

Consequence: `opi package add ./vendor/my-adapter -l` creates `.opi/packages.toml` with a declaration. A subsequent `opi` run ignores that file entirely. The adapter never starts. Its tools, commands, and hooks never register.

### P0-2: Package CLI operates below design contract

**Severity: P0 — CLI is a TOML editor, not a package manager**

The design's "opi package add" flow is:

```
parse source → resolve package root or clone git source → parse package.toml
→ write declaration to packages.toml → write package-lock.toml entry
→ print installed package summary
```

Actual `cmd_add` flow:

```
PackageSource::parse(source) → check idempotent → push PackageDeclaration
→ write_declarations()
```

Missing operations in `cmd_add`:
- Does not check that the local path exists.
- Does not parse `package.toml` with `PackageManifest::from_toml()`.
- Does not write `package-lock.toml` (`write_lock()` is never called by CLI).
- Does not compute `manifest_sha256`.
- Does not call `git_clone()` for git sources.
- Does not print a package summary (name, version, resources, adapter status).

`cmd_remove` matches by exact source string only. `remove todo` fails unless the source was literally the string `"todo"`. Design says ambiguous names should produce an error.

`cmd_list` reads from one scope (project only). Design says list should show "global and project packages with state, source, version, and diagnostics." Current output is source strings only.

`cmd_doctor` uses `toml::from_str::<toml::Value>()` instead of `PackageManifest::from_toml()`. Invalid adapter declarations, missing resources, `opi_version` mismatches, lock drift, and unresolvable adapter commands all pass doctor silently.

### P0-3: Missing adapter hook mappings

**Severity: P0 — design-required MVP behavior absent**

The design spec's "Phase 5 MVP capabilities" table marks hooks as "Required: yes" and lists four hook surfaces:

| Hook | Design status | Implementation status |
|------|--------------|----------------------|
| `before_tool_call` | Required, blocking | Implemented in `ProcessAdapter` |
| `after_tool_call` | Required, observational | Implemented in `ProcessAdapter` |
| `prepare_next_turn` | Required, may inject messages | **Not implemented** — `ProcessAdapter` does not override `Extension::prepare_next_turn()` |
| `transform_context` | Required, via `ProcessAdapterHooks` | **Not implemented** — `ProcessAdapterHooks` does not exist |

The design's "Adapter Host Bridge" section explicitly describes:

- `ProcessAdapterHooks implements AgentHooks` — wraps base hooks for `transform_context` only.
- `Extension::prepare_next_turn()` — "send hook, convert returned messages into turn update."

Neither is implemented. `CompositeHooks::transform_context()` at `crates/opi-agent/src/extension.rs:478` delegates only to `self.base`:

```rust
fn transform_context(
    &self,
    messages: Vec<AgentMessage>,
    signal: CancellationToken,
) -> Pin<Box<dyn Future<Output = Result<Vec<AgentMessage>, AgentError>> + Send>> {
    self.base.transform_context(messages, signal)
}
```

Extensions have no `transform_context` hook surface at the `Extension` trait level.

### P1-1: Adapter state does not survive session restart

**Severity: P1 — persistence promise unfulfilled**

The bridge is wired:

- `ProcessAdapter::serialize_state()` sends `state_serialize` to the adapter, receives `state_result`.
- `ProcessAdapter::restore_state()` sends `state_restore` to the adapter.
- `ExtensionRegistry::serialize_states()` collects state from all extensions as a JSON object.
- `ExtensionRegistry::restore_states()` distributes state back by extension name.

The persistence layer is not wired:

- `SessionEntry` (at `crates/opi-agent/src/session.rs:76`) has three variants: `Message`, `Compaction`, `Leaf`. No `ExtensionState` variant.
- `SessionCoordinator` persists LLM messages, compaction summaries, and leaf pointers. It does not call `serialize_states()` at any point — not at turn end, not at shutdown, not before session write.
- Session resume reconstructs the message/compaction/leaf chain. It does not call `restore_states()`.

Tests (`adapter_runtime.rs:adapter_state_round_trip_through_registry`) prove the registry round-trip works in isolation. No test proves state survives session file write → read → resume.

### P1-2: Shutdown kills without graceful window

**Severity: P1 — adapter cleanup prevented**

`shutdown_inner()` at `adapter_host.rs:447`:

```rust
async fn shutdown_inner(&mut self, reason: &str) -> Result<(), AdapterHostError> {
    // Send shutdown message with 200ms write timeout
    // ...
    // Kill the child process
    if let Some(ref mut child) = self.child {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    // Abort the reader task
    // ...
}
```

Between the shutdown message write (best-effort, 200ms) and `child.kill()`, there is no wait. On a loaded system the adapter may not even have read the shutdown message before being killed. The design's failure semantics table says "state serialization fails → continue shutdown but report session persistence diagnostic" — implying the host expects state serialization to occur during shutdown, which requires the adapter to be alive long enough to process it.

The `Drop` implementation calls `start_kill()` — which is `child.start_kill()` (non-async), consistent with Drop semantics but equally lacking a graceful window.

### P1-3: Event drop diagnostics absent

**Severity: P1 — silent data loss**

`send_event()` at `adapter_host.rs:351`:

```rust
pub async fn send_event(&self, event: serde_json::Value) {
    let msg = AdapterHostMessage::Event { event };
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(_) => return,  // silent drop on serialization failure
    };
    let write_result = tokio::time::timeout(Duration::from_millis(100), async {
        // write to stdin...
    }).await;
    // write_result is not inspected — silent drop on timeout or write failure
}
```

The design spec states: "If an adapter's stdin backpressure would block event delivery, the host may drop event messages and record a diagnostic."

Current implementation drops events silently. No diagnostic counter, no log message, no `tracing::warn!` call. A user experiencing event loss has no visibility into the cause.

### P2-1: Local package identity not canonicalized

**Severity: P2 — duplicate declaration risk**

`PackageSource::identity_key()` returns the raw path string from the declaration for local packages. The design says "local → canonical absolute path."

If a user runs both `opi package add ./vendor/foo -l` and `opi package add vendor/foo -l`, two declarations are stored with different identity keys pointing to the same package. When the startup loop eventually reads declarations, this would produce a duplicate package error or silent shadowing, depending on implementation.

### P2-2: SSH git URL misparse

**Severity: P2 — latent parsing bug**

`PackageSource::parse()` splits git refs using `rfind('@')`. For `git:ssh://git@github.com/user/repo` (no explicit ref), this splits at the `@` in the SSH username, producing `url = "ssh://git"` and `refspec = Some("github.com/user/repo")`.

The evaluator noted this as "future work." It is a latent bug that will trigger on the first SSH git package add without a ref.

### P2-3: Relative adapter command can escape package root

**Severity: P2 — security boundary weakness**

`resolve_adapter_command()` joins relative commands to the package root but does not normalize or reject `..` path components. A manifest declaring `command = "../../bin/malicious"` would resolve outside the package directory. Under the "packages are trusted code" security model this is tolerable, but it is surprising behavior and should at minimum be documented.

### P2-4: CHANGELOG missing Phase 5 entries

**Severity: P2 — release hygiene gap**

`CHANGELOG.md` `[Unreleased]` section contains only Phase 4 remediation entries. No Phase 5 feature (package CLI, adapter protocol, adapter host, manifest V2, adapter bridge, example adapters) appears in the changelog. This will cause confusion during the next release when Phase 5 changes are not recorded.

### P2-5: Documentation-code naming drift

**Severity: P2 — specification drift**

Minor naming inconsistencies between design spec and code:

| Design spec | Code |
|-------------|------|
| `"tool"` message type | `"tool_call"` / `"tool_result"` |
| `packages/` install directory | `packages.toml` + `package-lock.toml` store files |
| `ProcessAdapterHooks` type | Does not exist |

These are individually harmless but create friction for external developers reading the spec and then inspecting the code.

## Test Coverage Assessment

### Coverage by level

| Level | Files | Tests | Assessment |
|-------|-------|-------|------------|
| Unit — source/manifest/lock parsing | `package_store.rs`, `package_manifest_v2.rs` | 36 | Strong |
| Unit — protocol serde | `adapter_protocol.rs` | 22 | Strong |
| Integration — CLI commands | `package_cli.rs` | 18 | Adequate for declaration-only behavior |
| Integration — adapter host | `adapter_host.rs` | 16 | Strong (uses mock adapter binary) |
| Integration — adapter bridge | `adapter_runtime.rs` | 16 | Good (tools, commands, hooks, events, state) |
| Integration — harness + adapters | `harness_resource_integration.rs` | 14 | Good for manual wiring path |
| Integration — example adapters | `example_adapters.rs` | 16 | Good (3 adapters, commands, hooks, state) |
| Guard — documentation truth | `productized_packages_docs.rs` | 13 | Adequate for overclaim prevention |
| E2E — CLI subprocess | `package_cli.rs` (subset) | 4 | Adequate for CLI-only testing |
| E2E — full product loop | (none) | 0 | **Critical gap** |

### Critical coverage gaps

1. **No E2E from `opi package add` to adapter visibility.** No test installs a package via CLI and then verifies it appears in a subsequent harness or subprocess run.

2. **No test proves `packages.toml` declarations are read by `CodingHarness`.** The harness reads filesystem directories, not store declarations.

3. **No test proves lock files are written or consumed by the CLI lifecycle.** `read_lock` and `write_lock` are exercised in `package_store.rs` unit tests only.

4. **No test proves a git package can be installed through the CLI.** `cmd_add` accepts git sources syntactically but performs no git operations.

5. **No test proves `package doctor` catches manifest V2 validation errors.** Doctor uses `toml::Value` parsing; a manifest with `kind = "grpc"` would pass doctor but fail `PackageManifest::from_toml()`.

6. **No test proves extension state persists through session JSONL resume.** Registry round-trip is tested; session file round-trip is not.

7. **No test covers adapter `prepare_next_turn` or `transform_context`.** These hooks are not implemented.

8. **No test covers adapter model overrides with non-empty tool routing.** `AdapterModelOverride.tools` is parsed but ignored.

### Mock binary vs real example coverage

Tests use two test binaries:

- `adapter_host_mock` — minimal echo/hang/crash modes for host-level testing.
- `package_adapter_example` — todo/permission-gate/protected-paths modes for bridge and example testing.

Both are `[[test]]` targets. Tests construct `PackageResource` or `AdapterProcessConfig` values with the absolute path to the built binary. No test uses the `command` field from `examples/*/package.toml` directly, so the manifest → command resolution → spawn path is untested end-to-end.

## Cross-Validation with Codex Audit

The `audit.codex.md` was produced independently by a different model. This section compares findings.

### Shared findings (independently confirmed)

| Finding | Codex | This audit |
|---------|-------|------------|
| Installed packages not connected to runtime startup | P0 | P0-1 |
| Package CLI is declaration-only | P0 | P0-2 |
| Adapter state does not survive restart through sessions | P1 | P1-1 |
| Adapter hook coverage narrower than design | P1 | P0-3 (upgraded) |
| Event drop diagnostics absent | P1 | P1-3 |
| Local package identity not canonicalized | P2 | P2-1 |
| SSH git URL misparse | P2 | P2-2 |
| Relative adapter command path escape | P2 | P2-3 |

### Severity differences

| Finding | Codex severity | This audit severity | Reasoning |
|---------|---------------|---------------------|-----------|
| Missing `prepare_next_turn` / `transform_context` | P1 | **P0** | The design spec marks these as "Required: yes" in the MVP capabilities table. This is not an enhancement — it is a contract violation. The Codex audit offered "either implement or amend the ledger/docs" as remediation; this audit asserts the design contract is authoritative until amended. |
| Adapter process diagnostics not surfaced | P1 | P1-3 (narrower) | The Codex audit combined multiple diagnostic gaps into one finding. This audit scopes the P1 to event drops specifically and treats startup diagnostics as a consequence of P0-1 (startup not wired). |

### Findings unique to this audit

| Finding | Severity | Notes |
|---------|----------|-------|
| P1-2: Shutdown kills without graceful window | P1 | Codex audit mentioned shutdown semantics in passing; this audit identifies the specific 200ms-then-kill sequence as preventing adapter cleanup logic. |
| P2-4: CHANGELOG missing Phase 5 entries | P2 | Not covered in Codex audit. |
| P2-5: Documentation-code naming drift | P2 | Not covered in Codex audit. |
| Evaluator scope judgment conflicts | — | Task 5.2 evaluator said list/doctor global scope is "not MVP scope" while design spec says it is MVP. This audit flags the evaluator's judgment as inconsistent with the governing design spec. |
| `cmd_doctor` error carrier type abuse | — | `PackageStoreError::Git` is used to carry non-git diagnostic errors. Minor type hygiene issue. |
| Example adapter binary distribution gap | — | `package_adapter_example` is a test binary, not a distributable artifact. Codex audit mentions this implicitly; this audit makes it explicit. |
| `model_overrides.tools` parsed but ignored | — | Serde parses the tools field; the bridge discards it. |

### Findings unique to Codex audit

| Finding | Notes |
|---------|-------|
| `list --json` vs `doctor --json` format inconsistency | This audit did not independently flag this because both outputs are valid; the inconsistency is minor. |

## Remediation Recommendations

Ordered by priority and dependency:

### Phase 5a: Close the product loop (addresses P0-1, P0-2, P0-3)

1. **Wire `PackageStore` into harness construction.**
   - In `harness.rs` or a new `package_resolver.rs`, add a function that reads global and project `packages.toml`, merges declarations by identity and precedence, resolves each to a `PackageResource`, and returns the merged set.
   - Merge declaration-resolved packages with directory-scanned packages before calling `start_adapters_from_packages()`.
   - Call `start_adapters_from_packages()` during harness construction in all three modes (interactive, non-interactive, RPC).
   - Inject the resulting `ExtensionRegistry` into `CodingHarness`.
   - Gate: `opi package add ./examples/todo -l && opi --non-interactive "list your tools"` shows the todo adapter tool.

2. **Upgrade `cmd_add` to a lifecycle operation.**
   - Resolve the package root (check existence for local, clone for git).
   - Parse `package.toml` with `PackageManifest::from_toml()`.
   - Compute `manifest_sha256` and write `package-lock.toml`.
   - For git sources, call `git_clone()` and pin the commit in the lock entry.
   - Print a summary showing package name, version, resources, and adapter status.
   - Gate: `opi package add git:github.com/user/repo@main -l` clones, locks, and prints summary.

3. **Implement `prepare_next_turn` and `transform_context` adapter hooks.**
   - Add `prepare_next_turn` override to `ProcessAdapter` that sends a `hook` message and converts the response.
   - Create `ProcessAdapterHooks` as described in the design, wrapping base hooks for `transform_context`.
   - Wire `ProcessAdapterHooks` into the harness when adapters are present.
   - Gate: test that an adapter declaring `"prepare_next_turn"` in hooks can inject messages.

4. **Upgrade `cmd_remove` to support manifest name.**
   - Resolve declarations, parse manifests, match by manifest name or source.
   - Report ambiguous matches.

5. **Upgrade `cmd_list` and `cmd_doctor` to cover global + project scope.**
   - `list` merges global and project declarations with precedence markers.
   - `doctor` uses `PackageManifest::from_toml()`, checks `opi_version`, verifies resource containment, reads lock for drift detection, resolves adapter commands, and reports resolved executable paths.

### Phase 5b: Session persistence and shutdown (addresses P1-1, P1-2)

6. **Add extension state to session persistence.**
   - Add a `SessionEntry::ExtensionState` variant (or a metadata entry) carrying serialized extension state.
   - Call `ExtensionRegistry::serialize_states()` at turn end and graceful shutdown.
   - Call `ExtensionRegistry::restore_states()` after adapter startup during session resume.
   - Gate: start adapter, mutate state, quit, resume session, verify state restored.

7. **Add graceful shutdown window.**
   - After sending the shutdown message, wait up to a configurable timeout (e.g. 5s) for natural process exit.
   - If the process does not exit within the window, kill it.
   - Gate: adapter that writes a "shutdown_complete" log line demonstrates receiving and processing shutdown.

### Phase 5c: Diagnostics and hardening (addresses P1-3, P2-*)

8. **Add event drop diagnostics.**
   - Increment a counter on `send_event` timeout/failure.
   - Log at `tracing::debug!` per drop; summarize at session end or in `package doctor`.

9. **Canonicalize local package identity.**
   - `identity_key()` for local sources should return the canonicalized absolute path.

10. **Fix SSH git URL parsing.**
    - Use URL-aware parsing or require explicit `@ref` syntax for git sources.

11. **Normalize adapter command paths.**
    - Reject or warn on `..` components in relative adapter commands.

12. **Add Phase 5 CHANGELOG entries.**
    - Record all Phase 5 features under `[Unreleased]`.

## Exit Recommendation

**Do not mark Phase 5 as fully exited.** The phase exit in `opi-impl-state.json` records `"exit_criteria_met": true`, but 4 of 12 design success criteria are not met and 5 are only partially met.

Recommended classification: **substrate complete, product loop incomplete**.

Minimum gates for Phase 5 exit:

1. `opi package add ./examples/todo -l` writes a declaration **and** lock state.
2. A subsequent `opi` run from the same workspace discovers the installed package without manual `config.packages.paths`.
3. The installed adapter's tools/commands/hooks are registered in the runtime and visible to the agent.
4. `opi package doctor --json` reports manifest V2, `opi_version`, resource, lock, and adapter diagnostics.
5. Adapter state persists across session resume with a fresh adapter process.
6. `prepare_next_turn` and `transform_context` either work through the adapter bridge or are explicitly descoped from the Phase 5 design spec with a design amendment.
7. CHANGELOG records Phase 5 features.

If criteria 1–5 and 7 are met and criterion 6 is resolved by design amendment (narrowing the spec), Phase 5 can exit. If the design spec is not amended, all 7 criteria must pass.
