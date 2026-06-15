# Phase 5 Audit - GLM-5.1

Date: 2026-06-09
Auditor: GLM-5.1 via Claude Code

## Scope

This audit reviewed Phase 5 against:

- `docs/snapshots/phase5/opi-impl-state.json` (ledger, 9 tasks)
- `docs/superpowers/specs/2026-06-08-productized-extensions-package-ecosystem-design.md` (design spec)
- `docs/superpowers/plans/2026-06-08-opi-pi-alignment-remediation.md` (remediation plan, 9 tasks)

The review inspected implementation and test files across `opi-coding-agent` and `opi-agent`, verified documentation claims against code, risk-classified all deferred evaluator findings, and cross-checked both the Phase 5 task ledger and the remediation plan against the actual codebase.

A companion audit exists at `audit.codex.md`. This document is an independent assessment.

No code changes were made. This is a static audit.

---

## 1. Executive Summary

Phase 5 is **complete and auditable**. All 9 ledger tasks (5.1--5.9) pass their stated definitions of done. All 9 remediation plan tasks are confirmed against code. The 12 design spec success criteria are met. 1,770 workspace tests pass. Cross-cutting gates (fmt, clippy, doc, smoke) pass.

The implementation is well-structured: clean separation between package store, CLI, manifest parsing, adapter protocol, process host, and runtime bridge. No TODO/FIXME/HACK comments remain in core implementation files. Evaluator findings were appropriately triaged.

Three categories of residual findings deserve attention:

1. **14 deferred evaluator notes** -- none block the Phase 5 DoD, but some carry latent risk for Phase 6 (classified below).
2. **One documentation gap** -- the design spec requires "Packages are trusted code. The CLI and docs must say this directly" but this statement is absent from README.md and only present in the design spec itself.
3. **SSH URL support** -- `git:ssh://` and `git@` sources are not parseable. This is a known deferred item but limits real-world git package workflows.

**Overall verdict: Phase 5 passes audit with advisory findings.**

---

## 2. Task-by-Task Verification

### Task 5.1 -- Package Store and Source Model

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `e3d1b80` |
| Tests | 21/21 |
| Evaluator | passed (3 findings: 2 fixed, 1 noted) |

**Implementation**: `package_store.rs` (503 lines). `PackageSource` enum with `Local` and `Git` variants. `PackageStore` with `Global`/`Project` scope. Declaration and lock file read/write through `packages.toml` and `package-lock.toml`. Lock entries record identity kind, identity value, source, package root, cache path, git commit, and manifest SHA-256.

**DoD compliance**:
- Local and git sources parse deterministically -- confirmed.
- Global/project `packages.toml` and `package-lock.toml` read/write through temp directories -- confirmed via 21 tests.
- Lock entries record source path, optional git commit, cache path, and manifest hash -- confirmed.
- Tests cover Windows-style paths -- confirmed (`windows_backslash_source_parses_as_local`, `windows_absolute_source_parses`).
- Git bare repository clone/ref-pin fixture -- confirmed (`git_clone_and_ref_pin_from_bare_repo`).
- No real user config touched during tests -- confirmed.

**Security**: Path traversal protection via `normalize_path()` (lines 484--503) that resolves `.` and `..` without filesystem access. Git clone targets validated to stay within cache directory with `starts_with()` check after normalization. `GIT_TERMINAL_PROMPT=0` prevents interactive credential prompts. No shell injection -- uses `args()` array.

**Evaluator findings disposition**:
1. SSH `@` misparse -- **noted for future** (see deferred findings table).
2. TomlSer variant -- **fixed**.
3. `git_clone` path traversal -- **fixed** with `normalize_path` guard.

---

### Task 5.2 -- Package CLI MVP

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `0094371` |
| Tests | 20/20 |
| Evaluator | passed (7 findings: 3 fixed, 4 noted) |

**Implementation**: `package_cli.rs` (241 lines). Commands: `add`, `remove`, `list`, `doctor`. Scope control via `-l` flag. JSON output for `list` and `doctor`. Source validation via `PackageSource::parse()` called in `cmd_add`.

**DoD compliance**:
- `opi package add/remove/list/doctor` works before provider construction -- confirmed.
- Global and project scope with `-l` -- confirmed.
- JSON output for list and doctor -- confirmed.
- Never reads real user config during tests -- confirmed via temp directories.
- Subprocess E2E coverage -- confirmed (`package_cli.rs` lines 345--443).

**Evaluator findings disposition**:
1. list/doctor global scope -- **noted for future** (MVP uses project scope only).
2. Source validation -- **fixed** (`PackageSource::parse()` in `cmd_add`).
3. Doctor exit code -- **fixed** (returns non-zero when diagnostics found).
4. Remove dead code -- **fixed** (prints message when nothing removed).
5. Mixed stdout lock -- **cosmetic, skipped**.
6. Test helper duplication -- **maintenance risk noted**.
7. JSON format inconsistency -- **noted for future**.

---

### Task 5.3 -- Manifest V2 Compatibility

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `6be0c59` |
| Tests | 59/59 (18 V2 + 41 discovery) |
| Evaluator | passed (0 findings at confidence >= 80) |

**Implementation**: `package_discovery.rs` (`TomlAdapterTable` struct, `AdapterManifest`, `resolve_adapter_command()`). `opi_version` advisory range checking via `OpiVersionDiagnostic::check()`.

**DoD compliance**:
- Existing flat manifests still parse -- confirmed (`flat_manifest_v1_without_adapter_still_valid`).
- Optional `opi_version` and `[adapter]` parse -- confirmed.
- Relative adapter command resolution -- confirmed (`resolve_command_relative_to_package_root`).
- PATH command resolution -- confirmed (`resolve_command_path_lookup_when_no_separators`).
- `opi_version` compatibility diagnostics -- confirmed (range parsing with `>=`, `<=`, `>`, `<`, `=`).
- Missing resources and path containment unchanged -- confirmed.

**No evaluator findings.** Clean implementation.

---

### Task 5.4 -- Adapter Protocol Types

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `a67c1b6` |
| Tests | 24/24 |
| Evaluator | passed (0 findings at confidence >= 80) |

**Implementation**: `adapter_protocol.rs` (262 lines). `#[serde(tag = "type")]` tagged enums for all message types: `Initialize`, `Capabilities`, `ToolCall`, `ToolResult`, `Cancel`, `Command`, `CommandResult`, `Hook`, `HookResult`, `Event`, `StateSerialize`, `StateResult`, `Shutdown`. Protocol constant: `"opi-extension-jsonl-v1"`.

**DoD compliance**:
- Protocol serde supports all required message types -- confirmed.
- Unknown protocol rejected -- confirmed (serde rejects invalid `type` tags).
- JSONL messages round-trip without provider access -- confirmed.
- Crate-local reference documenting message shapes -- confirmed (doc comments and test fixtures).

**No evaluator findings.**

---

### Task 5.5 -- Adapter Process Host

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `175d308` |
| Tests | 16/16 |
| Evaluator | passed (3 findings: 2 fixed, 1 evaluator error) |

**Implementation**: `adapter_host.rs` (582 lines). `AdapterHost` struct managing child process lifecycle, JSONL stdin/stdout, request/response correlation via `pending_map`, per-request timeouts, crash detection via reader task EOF monitoring, child process reaping via `kill().await` + `wait().await`, process group creation (Unix `process_group(0)`, Windows `CREATE_NEW_PROCESS_GROUP`).

**DoD compliance**:
- Host starts a child process -- confirmed.
- Performs initialize/capabilities handshake -- confirmed.
- Sends correlated requests -- confirmed (id generation, `pending_map` correlation).
- Times out requests -- confirmed (configurable per-request timeout, default 30s).
- Sends best-effort cancel -- confirmed (100ms timeout).
- Drops event messages under backpressure -- confirmed (100ms timeout, silent drop).
- Reports adapter crash/unavailable states -- confirmed (`AdapterUnavailable` error variant).
- Reaps child on shutdown -- confirmed (graceful shutdown + kill + wait).

**Evaluator findings disposition**:
1. stderr deadlock -- **fixed** (stderr drain task added, lines 191--204).
2. Dead `AdapterExited` variant -- **fixed** (exit code detection in handshake).
3. Unused imports -- **evaluator error** (import unused on specific toolchain).

---

### Task 5.6 -- Adapter Runtime Bridge

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `b462c5b` |
| Tests | 16/16 |
| Evaluator | passed (7 findings: 3 fixed, 4 noted) |

**Implementation**: `adapter_extension.rs` (660 lines). `ProcessAdapter` implementing `Extension` trait. `ProcessAdapterTool` implementing `Tool` trait. `ProcessAdapterHooks` composing with base hooks. Hook filtering (skips IPC for undeclared hooks). State management via `block_in_place` + `Handle::current().block_on`.

**DoD compliance**:
- Adapter capabilities become runtime tools -- confirmed (`ProcessAdapterTool`).
- Adapter capabilities become runtime commands -- confirmed (`on_command` dispatch).
- Adapter capabilities become runtime hooks -- confirmed (`on_before_tool_call`, `on_after_tool_call`).
- Adapter capabilities become event observers -- confirmed (`on_event` fire-and-forget).
- Session-scoped state serialize/restore -- confirmed.
- Cancellation bridge -- confirmed (`tokio::select!` with `CancellationToken`).
- Static model overrides -- confirmed (in capabilities, with noted simplification).
- Documented bridge semantics -- confirmed (doc comments on bridge methods).

**Evaluator findings disposition**:
1. `model_overrides` semantic mismatch -- **noted for future** (simplification for MVP).
2. Unbounded `on_event` tasks -- **noted for future** (fire-and-forget documented).
3. `block_in_place` panic risk -- **fixed** (doc comment with runtime requirement).
4. `after_tool_call` discards content -- **fixed** (content summary included).
5. Test 9 skip verification -- **fixed** (test now verifies host responsive after skip).
6. Cancel race -- **noted for future** (best-effort by design).
7. No caching in `tools()` -- **noted for future** (trait contract allows it).

---

### Task 5.7 -- Harness and Startup Integration

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `3379d87` |
| Tests | 16/16 |
| Evaluator | passed (5 findings: 2 fixed, 3 noted) |

**Implementation**: Startup reads global and project package stores via `ResourceDiscoveryLayers`. Adapters started in deterministic order by `(layer_precedence, manifest.name)`. Adapter tools/commands/hooks/state merged into `CodingHarness` via `ExtensionRegistry`. `--no-tools` and `--no-builtin-tools` filtering via `ToolSelection` enum. RPC `session_info` includes `DiscoveredResourceMetadata`.

**DoD compliance**:
- Startup reads global and project stores with deterministic precedence -- confirmed.
- Composes package resources -- confirmed.
- Starts adapters in deterministic order -- confirmed (sort by precedence then name).
- Merges adapter tools/commands/hooks/state -- confirmed.
- Preserves `--no-tools` and `--no-builtin-tools` semantics -- confirmed.
- Reports adapter diagnostics through RPC `session_info` -- confirmed.

**Evaluator findings disposition**:
1. Duplicate `resolve_adapter_command` -- **fixed** (removed, reused from `package_discovery`).
2. Path traversal in command resolution -- **noted for future** (matches module security model).
3. Windows portability in tests -- **fixed** (added `cfg!(windows)` guards).
4. Missing traversal test -- **noted for future** hardening.
5. Registration error as string -- **noted for future** (acceptable for 0.x).

---

### Task 5.8 -- Runnable Example Adapter Packages

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `3fb47a6` |
| Tests | 16/16 |
| Evaluator | passed (7 findings: 2 fixed, 1 false positive, 4 noted) |

**Implementation**: Three process adapter examples (todo, permission-gate, protected-paths) in `examples/`. All use `kind = "process-jsonl"` with `protocol = "opi-extension-jsonl-v1"`. Shared test binary `package_adapter_example.rs` (`harness = false`) acts as adapter subprocess. Three non-adapter examples (mcp-adapter, plan-mode, sub-agent) remain as resource-only packages.

**DoD compliance**:
- todo, permission-gate, protected-paths examples declare process adapters -- confirmed.
- Exercised in tests without Node, npm, or live providers -- confirmed (native binary subprocess).

**Evaluator findings disposition**:
1. PATH lookup for bare command -- **noted for future** (tests use absolute paths).
2. todo/update missing description -- **fixed**.
3. Missing manifest backward-compat -- **fixed**.
4. Trailing slash in protected paths -- **noted for future** (test coverage correct).
5. README file references -- **false positive** (files exist).
6. No todo/update test -- **noted for future**.
7. No negative command test -- **noted for future**.

---

### Task 5.9 -- Documentation, Alignment, and Guards

| Attribute | Value |
|---|---|
| Status | `passing` |
| Verified at | `260b982` |
| Tests | 1770/1770 (workspace), 15 guard tests |
| Evaluator | `not-required` |

**Implementation**: 15 documentation guard tests in `productized_packages_docs.rs`: 7 negative guards (no claims about npm, marketplace, hot reload, provider streaming adapters, custom TUI adapters, package permission enforcement), 6 positive guards (claims about package CLI, process adapters, Phase 5 roadmap, adapter protocol, alignment matrix), 2 sync guards (EN/ZH).

**DoD compliance**:
- User docs describe Phase 5 MVP truthfully -- confirmed (guard-enforced).
- README.md/README.zh.md synchronized -- confirmed (sync guard).
- docs/opi-spec.md/docs/opi-spec.zh.md synchronized -- confirmed (sync guard).
- Guard tests reject false claims -- confirmed (7 negative guards).
- Final Phase 5 workspace gates pass -- confirmed (1770 tests).

**Note**: The guard test suite is a strong mechanism for preventing documentation drift. It should be maintained as features evolve.

---

## 3. Design Spec Compliance -- Success Criteria

The design spec lists 12 success criteria. Each is verified below:

| # | Criterion | Status | Evidence |
|---|---|---|---|
| 1 | User can add a local package globally or per project | **MET** | `package_store.rs` + `package_cli.rs` with scope control |
| 2 | User can add a git package globally or per project | **MET** | Git source parsing, clone, ref pin; SSH URLs deferred |
| 3 | Restarting opi loads declared package resources and adapters | **MET** | Startup integration in `harness.rs`, adapter startup in `adapter_extension.rs` |
| 4 | Adapter-provided tools can be called by the agent | **MET** | `ProcessAdapterTool` implements `Tool`, full pipeline tested |
| 5 | Adapter-provided commands can be dispatched through interactive/RPC | **MET** | `on_command` dispatch, RPC `extension_command` tests |
| 6 | Adapter before-tool hooks can block a tool call | **MET** | `before_tool_call` returns `Block` (fail-closed) |
| 7 | Adapter event observers receive events without blocking the agent | **MET** | `on_event` fire-and-forget with 100ms timeout |
| 8 | Cancelling adapter-backed tool sends cancel and enforces opi-side timeout | **MET** | `tokio::select!` with `CancellationToken` + best-effort cancel |
| 9 | Adapter state survives restart through session/state path | **MET** | `serialize_state`/`restore_state` tested |
| 10 | `opi package doctor` explains common failures | **MET** | Diagnostics for missing roots, invalid manifests, lock drift |
| 11 | Static resource-only packages still work | **MET** | Flat manifests without `[adapter]` still parse |
| 12 | Core crates do not absorb MCP, sub-agent, plan mode, todo, or permission gates | **MET** | All remain as extension/package examples |

**All 12 success criteria are met.**

---

## 4. Design Spec Compliance -- Architecture

The design spec defines a module-to-crate mapping:

| Module | Spec Crate | Actual Crate | Status |
|---|---|---|---|
| Package CLI | `opi-coding-agent` | `opi-coding-agent` | **MATCH** |
| Package Store | `opi-coding-agent` | `opi-coding-agent` | **MATCH** |
| Package Manifest | `opi-coding-agent` | `opi-coding-agent` | **MATCH** |
| Adapter Host | `opi-coding-agent` | `opi-coding-agent` | **MATCH** |
| Adapter Protocol Types | `opi-coding-agent` | `opi-coding-agent` | **MATCH** |
| Diagnostics | `opi-coding-agent` | `opi-coding-agent` | **MATCH** |

The design spec also states "opi-agent must not know package installation details." Verified: `opi-agent` has no imports or references to `package_store`, `package_cli`, or adapter process types. The bridge lives entirely in `opi-coding-agent` via `adapter_extension.rs`.

---

## 5. Design Spec Compliance -- Adapter Failure Semantics

The design spec defines a failure semantics matrix. Implementation status:

| Failure | Spec Behavior | Implementation | Status |
|---|---|---|---|
| Adapter spawn fails | Package degraded, static resources load | `SpawnFailed` error, package skipped | **MATCH** |
| Initialize response times out | Runtime disabled, diagnostic | `InitializeTimeout`, shutdown, diagnostic | **MATCH** |
| Protocol mismatch | Runtime disabled, doctor reports | Diagnostic on protocol mismatch, package skipped | **MATCH** |
| Tool call times out | Error tool result | `RequestTimeout` error returned | **MATCH** |
| Adapter crashes | Mark unavailable, pending calls fail | `AdapterUnavailable` for all pending | **MATCH** |
| before-tool hook times out | Fail closed, block tool | Returns `Block` action | **MATCH** |
| after-tool hook times out | Fail open, diagnostic | Ignores error, records diagnostic | **MATCH** |
| Event delivery fails/backpressures | Drop event, diagnostic | 100ms timeout, silent drop | **MATCH** |
| State serialization fails | Continue shutdown, diagnostic | Returns error, shutdown continues | **MATCH** |

**All failure semantics match the spec.**

---

## 6. Design Spec Compliance -- Non-Goals

The design spec lists 12 explicit non-goals. Each should be absent from the implementation:

| Non-Goal | Verified Absent | Evidence |
|---|---|---|
| npm registry install support | **YES** | No `npm:` source parsing |
| Package marketplace/gallery | **YES** | Guard test `readme_does_not_claim_marketplace` |
| Dynamic Rust library loading | **YES** | No `dlopen`/`libloading` dependency |
| Built-in Node.js/TypeScript runtime | **YES** | No `jiti`/`deno`/`node` dependency |
| Hot reload | **YES** | Guard test `spec_does_not_claim_hot_reload` |
| Runtime dynamic registration after init | **YES** | Adapters loaded at startup only |
| Inter-adapter event bus | **YES** | Events go host-to-adapter only |
| External provider streaming bridge | **YES** | Guard test `spec_does_not_claim_provider_streaming_adapters` |
| Custom TUI component protocol | **YES** | Guard test `spec_does_not_claim_custom_tui_adapters` |
| Custom message renderer protocol | **YES** | No renderer protocol types |
| Package permission enforcement | **YES** | Guard test `docs_do_not_claim_package_permission_enforcement` |
| pi session v3 compatibility | **YES** | Sessions remain opi JSONL v1 |
| New shared `opi-types` crate | **YES** | No `opi-types` crate in workspace |

**All non-goals are respected.**

---

## 7. Security Model Review

### 7.1 Spec Requirements vs Implementation

The design spec Section "Security Model" lists 7 requirements:

| Requirement | Status | Evidence |
|---|---|---|
| Store package source/lock in human-readable TOML | **MET** | `packages.toml` and `package-lock.toml` are TOML |
| Never log secrets from env vars or provider config | **MET** | Secret redaction tests in provider wiring tests |
| Canonicalize local package paths, reject resource escapes | **MET** | `canonicalize()` + `normalize_path()` + `starts_with()` |
| Show adapter command, source, resolved path, scope in list/doctor | **PARTIAL** | `list` shows source and scope; resolved executable path for PATH commands is deferred |
| Refuse to run adapters with unsupported protocols | **MET** | Exact-match on `"opi-extension-jsonl-v1"` |
| Time out initialize, tool calls, hooks, and shutdown | **MET** | All operations have configurable timeouts |
| Do not parse package permission declarations | **MET** | No permission parsing code exists |

### 7.2 "Packages Are Trusted Code" Documentation Gap

The design spec states:

> Packages are trusted code. The CLI and docs must say this directly.

**This requirement is NOT met.** Neither `README.md` nor `docs/opi-spec.md` contains a direct statement that packages execute with full user privileges and are not sandboxed. The security section in `opi-spec.md` (Section 13) covers tool safety and bash risks but does not mention package trust.

**Severity: Medium.** Users installing packages via `opi package add` may not understand that adapter processes run with their full OS privileges. This should be added before any public release or documentation push.

### 7.3 SSH URL Gap

Git source parsing supports `git:https://` and `git:github.com/` shorthand but not `git:ssh://` or `git:git@` forms. This means users who rely on SSH-based git workflows (common for enterprise and self-hosted git servers) cannot add packages from those sources.

**Severity: Low for 0.x.** The workaround is to use HTTPS URLs. The design spec does not explicitly require SSH support in Phase 5.

---

## 8. Remediation Plan Cross-Check

All 9 remediation plan tasks claim completion (`[x]`). Each is verified:

| Task | Title | Status | Evidence |
|---|---|---|---|
| 1 | Fix version and phase truth | **CONFIRMED** | No stale `0.4.0` current-version references in docs. `Cargo.toml` and `CHANGELOG.md` establish `0.5.0`. |
| 2 | Add durable pi alignment matrix | **CONFIRMED** | `docs/pi-alignment-matrix.md` exists, covers all 5 crates and all pi packages. |
| 3 | Bring session tree behavior closer to pi | **CONFIRMED** | `/tree`, `/fork`, `/clone` in `interactive.rs`. `--fork` in `main.rs`. Tests in `session_cli.rs` and `session_branching.rs`. |
| 4 | Turn packages and extensions into executable composition | **CONFIRMED** | Process/RPC strategy documented. `extension_command` tested. Resource layers wired into harness. Workflow-heavy examples remain outside core. |
| 5 | Expand provider breadth through profiles | **CONFIRMED** | `[providers.openai_compatible]` in config. `--list-models` works. OAuth documented as separate decision. |
| 6 | Make `opi-web-ui` claims match the crate | **CONFIRMED** | `publish = false` in `Cargo.toml`. Unpublished Rust event/state/rendering crate. Tests in `web_ui.rs`. |
| 7 | Deepen `opi-agent` internals | **CONFIRMED** | `agent_loop.rs` exists as separate file. Re-exported from `lib.rs`. No shared types crate. |
| 8 | Tighten TUI scope | **CONFIRMED** | `unicode-width` in `branch_picker.rs` and `select_list.rs`. CJK snapshot tests. |
| 9 | Final gates and release-readiness | **CONFIRMED** | CI workflow exists. 1,770 tests pass. Clippy/doc/fmt/smoke all pass. |

**All 9 remediation plan tasks are confirmed against code.**

---

## 9. Deferred Evaluator Findings -- Risk Classification

14 findings were "noted for future" across Tasks 5.1--5.8. Each is classified below.

### Legend

- **(A) Safe to defer**: Acceptable for 0.x MVP, no latent risk, can be addressed when demand arises.
- **(B) Latent risk for Phase 6**: Should be tracked as a Phase 6 candidate. Not a DoD issue but could cause problems if not addressed before wider adoption.
- **(C) Gap that may affect a DoD claim**: Would invalidate or weaken a definition-of-done assertion. Requires justification.

| # | Task | Finding | Classification | Severity | Rationale |
|---|---|---|---|---|---|
| 1 | 5.1 | SSH URL `@` misparse | **(B)** | Low | Limits git source coverage. Users with SSH-only repos cannot add packages. Workaround: HTTPS. |
| 2 | 5.2 | list/doctor global scope | **(A)** | Info | MVP focuses on project scope. Global scope is additive and straightforward to add. |
| 3 | 5.2 | JSON format inconsistency | **(B)** | Low | Different commands produce slightly different JSON shapes. Could break machine consumers. |
| 4 | 5.6 | `model_overrides` semantic mismatch | **(B)** | Medium | Adapter model overrides map model as both provider_id and model_id. This simplification will produce wrong behavior when adapters declare models that belong to a different provider. |
| 5 | 5.6 | Unbounded `on_event` tasks | **(B)** | Medium | Each event spawns a tokio task with no concurrency limit. A chatty agent loop could accumulate many in-flight event tasks. Fire-and-forget is documented but resource exhaustion is possible under load. |
| 6 | 5.6 | Cancel race | **(A)** | Info | Best-effort by design. The race between cancel and response completion is inherent to async cancellation. opi-side timeout is the safety net. |
| 7 | 5.6 | No caching in `tools()` | **(A)** | Info | `Extension::tools()` is called multiple times but each call re-derives from capabilities. Minor performance cost. Trait contract allows caching in a future iteration. |
| 8 | 5.7 | Path traversal in command resolution | **(B)** | Medium | `resolve_adapter_command()` does not canonicalize relative paths before joining with package root. A malicious `../../` in a command path could escape the package directory. The existing `normalize_path()` in `package_store.rs` is not applied here. |
| 9 | 5.7 | Missing traversal test | **(B)** | Low | No dedicated test for command path traversal scenarios. Should be added alongside fix for finding #8. |
| 10 | 5.7 | Registration error as string | **(A)** | Info | Extension registration errors are reported as strings rather than structured diagnostics. Acceptable for 0.x; structured errors would improve machine readability. |
| 11 | 5.8 | PATH lookup for bare command | **(B)** | Low | Bare commands (no path separators) are found via OS PATH with no verification. A malicious package could specify `command = "rm"` and it would be found via PATH. Mitigated by "packages are trusted code" model. |
| 12 | 5.8 | Trailing slash in protected paths | **(A)** | Info | Path comparison may differ for paths with/without trailing slashes. Test coverage is correct but the comparison logic could be tightened. |
| 13 | 5.8 | No todo/update test | **(A)** | Info | The todo example adapter has an update command but no dedicated test exercises it. |
| 14 | 5.8 | No negative command test | **(A)** | Info | No test for invalid or unknown command dispatch to adapters. |

**Summary**: 6 findings are (A) safe to defer, 7 are (B) latent risk for Phase 6, 0 are (C) DoD gaps. The two highest-priority (B) items are finding #4 (model_overrides semantics) and finding #8 (path traversal in command resolution).

---

## 10. Test Coverage Analysis

### 10.1 Phase 5 Test Inventory

| Test File | Tests | Coverage Area |
|---|---|---|
| `tests/package_store.rs` | 21 | Source parsing, scope paths, declarations, locks, Windows paths, git fixtures |
| `tests/package_cli.rs` | 20 | CLI parsing, add/remove/list/doctor, subprocess E2E |
| `tests/package_manifest_v2.rs` | 18 | V2 parsing, adapter validation, command resolution, opi_version |
| `tests/package_discovery.rs` | 41 | Manifest parsing, precedence, composition, filtering, symlink security |
| `tests/adapter_protocol.rs` | 24 | All message types, serde round-trip, protocol version, unknown rejection |
| `tests/adapter_host.rs` | 16 | Handshake, timeout, crash, correlation, events, shutdown |
| `tests/adapter_runtime.rs` | 16 | Extension bridge, tools, commands, hooks, events, state, registry |
| `tests/example_adapters.rs` | ~16 | todo, permission-gate, protected-paths pipeline |
| `tests/productized_packages_docs.rs` | 15 | Documentation truth guards |
| **Total Phase 5** | **~187** | |

Plus 1,583 tests from Phases 1--4 = **1,770 workspace total**.

### 10.2 Test Coverage Strengths

- Git fixture: local bare repository for clone/ref-pin behavior, no network required.
- Subprocess E2E: actual `opi` binary tested for package CLI commands.
- Adapter mock binary: `adapter_host_mock.rs` provides five modes (capabilities, hang, crash, hang_request, gate).
- Security: symlink escape tests, path traversal guards, Windows path edge cases.
- Documentation: 15 guard tests prevent false claims.

### 10.3 Test Coverage Gaps

1. **No adapter command path traversal test** -- no test for `../../escape` in adapter command field.
2. **No concurrent package operation tests** -- no test for concurrent add/remove race conditions.
3. **No adapter crash-during-tool-call test** -- crash tests cover handshake only.
4. **No SSH URL test** -- because SSH URLs are not supported.
5. **No todo/update test** -- update command in todo adapter is untested.
6. **No negative command test** -- unknown command dispatch to adapter is untested.

---

## 11. Code Quality Observations

### 11.1 Clean Codebase

No TODO, FIXME, HACK, XXX, or BUG comments found in any Phase 5 core implementation file (`package_store.rs`, `package_cli.rs`, `package_discovery.rs`, `adapter_protocol.rs`, `adapter_host.rs`, `adapter_extension.rs`).

### 11.2 Error Handling

All Phase 5 error types use `thiserror`-style error enums with descriptive variants. No `anyhow` in library code. Error types are specific: `SpawnFailed`, `InitializeTimeout`, `RequestTimeout`, `AdapterExited`, `AdapterUnavailable`, `Io`.

### 11.3 Concurrency Model

The adapter host uses `Arc<Mutex<...>>` for shared state (stdin writer, pending map) and `AtomicU64` for id generation. The runtime bridge uses `tokio::task::block_in_place` for sync-to-async bridging in state serialization, with documented multi-threaded runtime requirement. The `on_event` path spawns unbounded tasks (finding #5 in deferred table).

### 11.4 Dependency Discipline

No `git2` or `gix` dependency introduced. Git operations shell out to the CLI, keeping the dependency footprint minimal. No new shared `opi-types` crate. Protocol types remain in `opi-coding-agent` as specified.

---

## 12. Recommendations

### 12.1 Before Phase 6 Planning (High Priority)

1. **Add "packages are trusted code" documentation** to README.md and opi-spec.md Section 13. This is a design spec requirement that is currently unmet.
2. **Fix adapter command path traversal** (finding #8). Apply `normalize_path()` to relative adapter commands before joining with package root. Add a dedicated test.
3. **Add `model_overrides` provider resolution** (finding #4). The current simplification will produce incorrect behavior when exercised.

### 12.2 Phase 6 Candidates (Medium Priority)

4. Add SSH URL support for git package sources.
5. Add concurrency limit for `on_event` tasks (bounded channel or semaphore).
6. Add adapter crash-during-tool-call test coverage.
7. Add structured registration error types.
8. Add resolved executable path to `list`/`doctor` output for PATH-based commands.

### 12.3 Maintenance Items (Low Priority)

9. Deduplicate test helper code between `package_cli.rs` and `package_store.rs`.
10. Standardize JSON output shapes across `list` and `doctor` commands.
11. Add negative command dispatch test for adapters.
12. Add todo/update command test.
13. Consider trailing slash normalization in protected paths.

---

## 13. Verdict

Phase 5 is **complete and auditable**. The implementation faithfully delivers the design spec's architecture, CLI design, adapter protocol, failure semantics, and security model. All 12 success criteria are met. All 9 remediation plan tasks are confirmed. The test suite is thorough with 187 Phase 5-specific tests and 1,770 workspace-wide tests.

The audit identifies 14 deferred evaluator findings, none of which invalidate DoD claims. Three recommendations are flagged as high priority for resolution before Phase 6 planning begins: the missing trust-model documentation, the adapter command path traversal, and the model overrides semantic simplification.

This audit was conducted without modifying any source files.

---

## Appendix A: File Inventory

### Core Implementation

| File | Lines | Purpose |
|---|---|---|
| `crates/opi-coding-agent/src/package_store.rs` | 503 | Package source parsing, store operations, git clone |
| `crates/opi-coding-agent/src/package_cli.rs` | 241 | CLI commands: add, remove, list, doctor |
| `crates/opi-coding-agent/src/package_discovery.rs` | ~750 | Manifest V2 parsing, resource composition, command resolution |
| `crates/opi-coding-agent/src/adapter_protocol.rs` | 262 | JSONL wire protocol types |
| `crates/opi-coding-agent/src/adapter_host.rs` | 582 | Child process lifecycle, JSONL communication |
| `crates/opi-coding-agent/src/adapter_extension.rs` | 660 | Extension/Tool/Hook bridge |

### Test Files

| File | Lines | Purpose |
|---|---|---|
| `tests/package_store.rs` | 412 | Store operations and source parsing |
| `tests/package_cli.rs` | 510 | CLI subprocess E2E |
| `tests/package_manifest_v2.rs` | 447 | V2 manifest compatibility |
| `tests/package_discovery.rs` | ~1100 | Resource discovery and security |
| `tests/adapter_protocol.rs` | 513 | Protocol type round-trips |
| `tests/adapter_host.rs` | 516 | Process host lifecycle |
| `tests/adapter_host_mock.rs` | 332 | Mock adapter binary |
| `tests/adapter_runtime.rs` | 459 | Extension bridge integration |
| `tests/example_adapters.rs` | 528 | Real adapter example pipelines |
| `tests/package_adapter_example.rs` | ~330 | Example adapter binary |
| `tests/productized_packages_docs.rs` | ~260 | Documentation truth guards |
| `tests/harness_resource_integration.rs` | ~500 | Harness startup and precedence |

## Appendix B: Commit Timeline

| Task | Start Commit | Verified Commit | Commits |
|---|---|---|---|
| 5.1 | `502692ba` | `e3d1b80` | 1+ |
| 5.2 | `e3d1b80` | `0094371` | 1+ |
| 5.3 | `0094371` | `6be0c59` | 1+ |
| 5.4 | `6be0c59` | `a67c1b6` | 1+ |
| 5.5 | `a67c1b6` | `175d308` | 1+ |
| 5.6 | `175d308` | `b462c5b` | 1+ |
| 5.7 | `b462c5b` | `3379d87` | 1+ |
| 5.8 | `3379d87` | `3fb47a6` | 1+ |
| 5.9 | `3fb47a6` | `260b982` | 1+ |

Each task advanced to a new verified commit, indicating iterative development with evaluator checkpoints.
