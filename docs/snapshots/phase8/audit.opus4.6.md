# Phase 8 Agent Runtime Stabilization -- Audit (Opus 4.6)

## Metadata

| Field | Value |
|-------|-------|
| Date | 2026-06-24 |
| HEAD | `ef3424bab054995d7c57db3e903c38a54b8732ad` |
| Auditor | Opus 4.6 (Cursor agent) |
| Design spec | `docs/superpowers/specs/2026-06-15-phase8-agent-runtime-stabilization-design.md` |
| Impl state | `docs/snapshots/phase8/opi-impl-state.json` |
| Workspace version | 0.5.3 |
| Phase 8 tasks | 8.1 -- 8.7 (7 tasks, all status `passing`) |
| Phase 8 commits | `8816d32` .. `1f270b8` (7 verified commits) |

### Baseline gate results

| Gate | Result |
|------|--------|
| `cargo fmt --check --all` | Clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo test --workspace --all-targets` | Exit 1 (see note) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | Clean |

**Test note:** Identical to Phase 7: the sole exit-code-1 binary is `adapter_host_mock`, a `harness = false` mock adapter process that hangs when invoked directly by `cargo test` because it reads stdin. All 53 completed test suites report 0 failures. The impl-state at Phase 8 exit commit (`1f270b8`) reports 2005 passed / 0 failed across 133 binaries.

### Phase 8 specific test results

| Test file | Crate | phase8_* count |
|-----------|-------|---------------:|
| `agent_loop_semantics.rs` | opi-agent | 13 |
| `hooks_queues.rs` | opi-agent | 7 |
| `extensions.rs` | opi-agent | 2 |
| `tool_validation.rs` | opi-agent | 1 |
| `trace_envelope.rs` | opi-agent | 4 |
| `diagnostics.rs` | opi-agent | 1 |
| `diagnostics_runtime.rs` | opi-agent | 1 |
| `sdk_embedding.rs` | opi-agent | 3 |
| `rpc_jsonl.rs` | opi-coding-agent | 8 |
| `adapter_runtime.rs` | opi-coding-agent | 3 |
| `interactive_mock.rs` | opi-coding-agent | 1 |
| `session_runtime.rs` | opi-coding-agent | 2 |
| `json_mode.rs` | opi-coding-agent | 1 |
| `doctor_cli.rs` | opi-coding-agent | 1 |
| `sdk_embedding.rs` | opi-coding-agent | 4 |
| `runtime_contract_docs.rs` | opi-coding-agent | 3 |
| **Total** | | **55** |

---

## Executive Summary

Phase 8 is **complete** against its design spec. All 8 Success Criteria are met. All 8 Non-Goals are confirmed absent. The implementation is primarily characterization tests and documentation (55 new contract tests, comprehensive README additions in EN and ZH) with three targeted production changes.

| Severity | Count |
|----------|------:|
| **Pass** | 28 |
| **Note** | 6 |
| **Risk** | 5 |
| **Gap** | 0 |

No Gaps found. The 5 Risk items are forward-looking Phase 9 handoff concerns or inherited Phase 7 residuals, none of which violate the Phase 8 spec.

---

## 1. Specification Conformance

### SC1: Runtime event order documented and contract-tested

**A-1.1 | Pass | Event order is documented and tested**

The opi-agent README (EN and ZH) documents the full runtime event order from `agent_start` through `agent_end`, including per-turn cancel check, hook dispatch points, tool execution brackets, `should_stop_after_turn`, `prepare_next_turn`, and steering/follow-up queue boundaries. Four contract tests pin the documented order.

Representative test `phase8_event_order_no_tool_run` uses `RecordingProvider` + real `agent_loop` and asserts: `AgentStart` is first, `AgentEnd` is last, exactly 1 provider call, no tool events, single `turn_start`/`turn_end`, and `turn_start < message_end < turn_end` ordering.

Evidence: `crates/opi-agent/README.md` Loop Shape section; `crates/opi-agent/tests/agent_loop_semantics.rs` 4 phase8 event-order tests.

**A-1.2 | Pass | Queue polling order documented and tested**

Two tests pin the steering-before-follow-up contract and compaction-stop-before-next-turn behavior. `phase8_queue_polling_order_steering_before_follow_up` asserts 3 provider calls with steering at call index 1 and follow-up at index 2. `phase8_queue_polling_order_compaction_stop_before_next_turn` asserts only 1 provider call when `should_stop_after_turn` returns true, with follow-up not delivered and `prepare_next_turn` not run.

Evidence: `crates/opi-agent/tests/hooks_queues.rs` 2 phase8 queue-polling tests.

### SC2: Hook order and failure semantics documented and tested

**A-1.3 | Pass | Hook order and effects are documented and tested**

The README documents six hook methods with their order and effects. Five hook-contract tests cover: hook ordering within a turn (`transform -> convert -> before -> after -> should_stop -> prepare`), `before_tool_call` blocking after validation, `after_tool_call` replacement appearing in events, `prepare_next_turn` injection reaching the next provider request, and terminal stop skipping prepare.

Representative test `phase8_hook_contract_before_call_after_validation` runs two cases: (1) invalid args -> `before_tool_call` and `execute` both do not run, error tool result persisted; (2) hook returns `Deny` -> `execute` does not run, error result contains deny reason.

Evidence: `crates/opi-agent/README.md` Hook Semantics section; `crates/opi-agent/tests/hooks_queues.rs` 5 phase8 hook-contract tests.

**A-1.4 | Pass | Extension hook composition tested**

Two tests verify `ExtensionRegistry::wrap_hooks` composition: base hook runs first, then extensions in registration order; a `Block` from one extension stops the chain.

Evidence: `crates/opi-agent/tests/extensions.rs` 2 phase8 hook-composition tests.

**A-1.5 | Pass | Skipped adapter hooks visible in trace**

`phase8_skipped_adapter_hooks_trace` verifies that undeclared adapter hooks produce `TraceKind::HookSkipped` records with adapter name in details. This is backed by production code: `Extension::set_trace_collector` trait method, `ProcessAdapter::record_hook_skip`, and harness `prepare_trace_run`/`finish_trace_run` lifecycle.

Evidence: `crates/opi-coding-agent/tests/adapter_runtime.rs::phase8_skipped_adapter_hooks_trace`.

### SC3: Tool scheduling and termination tested

**A-1.6 | Pass | Tool scheduling covers parallel, sequential, mixed, and termination**

Seven tool-scheduling tests and one tool-validation test cover: parallel batch executes all, sequential batch runs serially (`tool_a:end < tool_b:start`), mixed batch forces sequential, persisted results in source order (even when completion order differs), completion events one per tool, all-terminate stops early (1 provider call), partial-terminate continues (2 provider calls), and validation failure as normal runtime outcome (not loop error).

Representative test `phase8_tool_scheduling_persisted_results_in_source_order` uses a slow `tool_a` and fast `tool_b` in a parallel batch, asserts completion order `b` before `a`, and persisted tool result ids are `["c1","c2"]` (source order).

Evidence: `crates/opi-agent/tests/agent_loop_semantics.rs` 7 phase8 tool-scheduling tests; `crates/opi-agent/tests/tool_validation.rs` 1 phase8 test.

### SC4: Cancellation has a consistent observable contract

**A-1.7 | Pass | Cancellation is tested across all documented paths**

Six cancellation tests cover: before-turn cancel (no provider call, `Err(Cancelled)`, `AgentEnd` with seed messages only), mid-stream cancel (partial discarded, no `message_end`), RPC abort (success + idle transition + new prompt accepted), adapter cancel (best-effort, `Err(ToolError::Cancelled)`, marker file), interactive abort/shutdown (`Err(Cancelled)`, `reset_cancel_if_cancelled` restores idle), and session persistence (cancelled turn writes no partial state).

Representative test `phase8_cancel_persists_only_finalized_state` completes turn 1, cancels turn 2, asserts JSONL contains exactly 2 message entries (turn 1 user + assistant), no "partial" or "second" text.

Evidence: `crates/opi-agent/tests/agent_loop_semantics.rs` 2 phase8 cancellation tests; `crates/opi-coding-agent/tests/rpc_jsonl.rs`, `adapter_runtime.rs`, `interactive_mock.rs`, `session_runtime.rs` 4 additional phase8 cancellation tests.

### SC5: SDK/RPC command behavior documented, versioned, and tested

**A-1.8 | Pass | SDK/RPC command-state contract is comprehensive**

The README documents the full command-state table (Idle vs While-running acceptance/rejection for each command). Twelve tests across three files cover: all 12 documented commands parse and name correctly, unsupported response carries `error_code`, `AgentControl` routes steer/follow_up/abort, `set_model` validation with structured errors, `set_thinking_level` validation, extension dispatch when unhandled, compact no-op structured diagnostic, second prompt rejected while busy, continue rejected while busy, abort while idle is no-op, follow_up queued while running, and mutating commands (compact/session_info/extension_command) rejected while busy.

Evidence: `crates/opi-agent/README.md` SDK and RPC Command Contract section; `crates/opi-agent/tests/sdk_embedding.rs` 3 + `crates/opi-coding-agent/tests/sdk_embedding.rs` 4 + `crates/opi-coding-agent/tests/rpc_jsonl.rs` 5 phase8 command-contract tests.

**A-1.9 | Note | SDK_SCHEMA_VERSION was already 3 before Phase 8**

The README states `SDK_SCHEMA_VERSION = 3`. The impl-state Phase 8 session notes say "no schema bump" for task 8.6. The version was bumped from 2 to 3 during Phase 7 (additive `error_code` field). Phase 8 did not bump any schema version.

Evidence: `crates/opi-agent/src/sdk.rs` line 42; CHANGELOG Phase 8 entries.

### SC6: Public API surfaces classified

**A-1.10 | Pass | API surface classification is documented and guard-tested**

The README classifies 12 named surfaces into three tiers: 5 supported 0.x (`Agent`, `agent_loop`, `AgentHooks`, `Tool`, `AgentEvent`), 7 unstable internal (`AgentSessionEvent`, `SessionEntry`, `Extension`, `ExtensionRegistry`, `SdkCommand`, `SdkResponse`, `StreamingProxy`), and 0 candidate removal. The guard test `phase8_api_surface_classification` asserts: classification section exists in EN and ZH, all three tiers documented, every surface bound to its tier on a single doc line in both languages, no stable 1.0 promise, `#[non_exhaustive]` documented, all three schema versions documented, and `lib.rs` pub use re-export pins verified.

Evidence: `crates/opi-agent/README.md` API Surface Classification section; `crates/opi-coding-agent/tests/runtime_contract_docs.rs::phase8_api_surface_classification`.

**A-1.11 | Pass | Classification matches actual crate visibility**

The `lib.rs` re-exports match the documentation: `Agent`, `agent_loop`, `AgentHooks`, `Tool`, `AgentEvent` are re-exported at the crate root (supported 0.x). `AgentSessionEvent`, `SdkCommand`, `SdkResponse`, `Extension`, `ExtensionRegistry`, and streaming proxy types are re-exported at the crate root but documented as unstable internal. `SessionEntry` is confirmed module-path-only (not re-exported at the crate root), matching its unstable internal classification.

Evidence: `crates/opi-agent/src/lib.rs` lines 27-48; guard test re-export pins.

### SC7: Phase 7 diagnostics and traces cover runtime contract failures

**A-1.12 | Pass | Runtime contract failures produce diagnostics and trace records**

Thirteen tests across eight files cover: tool validation trace (`ToolCallFailed` + `DiagnosticLinked` with `CODE_TOOL_VALIDATION_FAILED`), execute error trace, hook deny trace (goes through `CODE_TOOL_EXECUTION_FAILED`), cancellation trace (`CODE_AGENT_CANCELLED`, Info severity), real-format redaction contract (Anthropic/OpenAI keys, credentialed URLs, absolute paths), session/compaction diagnostics reaching in-process sink, RPC runtime contract failure diagnostics (error_code wire values), RPC trace production reachability and per-run scoping, startup diagnostics structured and redacted, adapter degradation diagnostic contract, and doctor public diagnostic message redaction.

Representative test `phase8_rpc_trace_production_reachable_and_scoped` uses the production `new_with_runtime_packages` constructor (not test-only `new_with_trace`), runs two sequential prompts, requests a trace, and asserts: trace success, `schema_version == TRACE_SCHEMA_VERSION`, all records share a single `run_id` (per-run scoped), and at least one `run_started`/`run_ended`.

Evidence: 8 test files with 13 phase8 diagnostic/trace tests; `crates/opi-coding-agent/src/rpc.rs` 5 error_code constants.

**A-1.13 | Pass | Session-recovery diagnostics wired to in-process sink**

Task 8.6 production change: `harness.rs` lines 988-995 iterate resumed session diagnostics through `record_harness_diagnostic`, writing them to `RecordingSink` and optionally to the trace collector. This closes the Phase 7 residual where session-recovery diagnostics bypassed the in-process sink.

Evidence: `crates/opi-coding-agent/src/harness.rs` lines 988-995; `crates/opi-coding-agent/tests/session_runtime.rs::phase8_session_recovery_diagnostics_reach_in_process_sink`.

### SC8: No ecosystem expansion or workflow feature enters core

**A-1.14 | Pass | Non-goals verified by automated guard**

Guard test `phase8_non_goals_not_claimed_or_implemented` verifies: (1) eight doc files contain no forbidden positive-claim phrases about stable 1.0, TypeScript extension API, package ecosystem, new adapter kind, web UI, OAuth, in-core workflow, or MCP runtime; (2) no `opi-types` or `opi-web-ui` crate exists; (3) no `oauth2`/`openidconnect`/`tame-oauth` Cargo dependencies; (4) no in-core `web_ui`/`oauth`/`mcp`/`plan_mode`/`sub_agent`/`todo`/`permission_popup` modules; (5) `agent_loop` entry point is intact; (6) adapter protocol is `opi-extension-jsonl-v1` only.

Meta-guard test `phase8_negation_helper_rejects_positive_claims` validates the assertion helper itself.

Evidence: `crates/opi-coding-agent/tests/runtime_contract_docs.rs` 2 phase8 non-goal tests.

### Non-Goals (NG1-NG8)

**A-1.15 | Pass | All 8 Non-Goals confirmed absent**

| NG | Non-Goal | Status |
|----|----------|--------|
| NG1 | No stable 1.0 public API promise | Absent; doc guard + API classification states 0.x |
| NG2 | No TypeScript extension API compatibility | Absent; doc guard + code scan |
| NG3 | No package ecosystem expansion | Absent; doc guard + code scan |
| NG4 | No new adapter kind | Absent; `opi-extension-jsonl-v1` only |
| NG5 | No web UI product work | Absent; doc guard + no `opi-web-ui` crate |
| NG6 | No provider OAuth work | Absent; no `oauth2`/`openidconnect`/`tame-oauth` deps |
| NG7 | No in-core plan/sub-agent/todo/permission/MCP runtime | Absent; code scan |
| NG8 | No whole-loop rewrite | Absent; `agent_loop` entry intact; no structural change |

---

## 2. Architecture Review

**A-2.1 | Pass | Contract tests use production `agent_loop` path**

All 55 phase8_* tests route through production code paths: `agent_loop_semantics.rs` tests call `opi_agent::agent_loop` with `RecordingProvider` or custom mock providers; RPC tests use `RpcRunner` with production constructors; harness tests use `CodingHarness`. No test bypasses the production loop or simulates behavior without exercising real dispatch.

Evidence: `crates/opi-agent/tests/agent_loop_semantics.rs` line 861 (`opi_agent::agent_loop`); `crates/opi-coding-agent/tests/rpc_jsonl.rs` line 4528 (`RpcRunner::new_with_runtime_packages`).

**A-2.2 | Pass | Production changes are minimal and necessary**

Phase 8 made three production changes:

1. **Task 8.2:** `TraceKind::HookSkipped` variant + `Extension::set_trace_collector` trait method + `ProcessAdapter::record_hook_skip` + harness `prepare_trace_run`/`finish_trace_run` trace lifecycle. Required to make the "adapter implements only a subset of hooks" case visible (spec Hook Semantics requirement).

2. **Task 8.6:** `harness.rs` session-recovery diagnostics wired to `record_harness_diagnostic` (6 lines). Required to close Phase 7 residual A-7.1.

3. **Task 8.6:** `rpc.rs` five `error_code` constants (`agent_busy`, `harness_unavailable`, `compaction_failed`, `extension_command_not_handled`, `unsupported_trace_request`) + `response_error_with_code` helper + const-pin unit test. Required to stabilize the RPC error wire format (spec SDK and RPC Contract requirement).

All other tasks (8.1, 8.3, 8.4, 8.5, 8.7) are characterization tests and documentation only. No production code was changed for them.

Evidence: impl-state session notes; `crates/opi-agent/src/trace.rs` lines 79-83; `crates/opi-coding-agent/src/harness.rs` lines 988-995; `crates/opi-coding-agent/src/rpc.rs` lines 90-95, 946-952, 983-990.

**A-2.3 | Pass | No schema version was bumped during Phase 8**

`SDK_SCHEMA_VERSION = 3`, `NDJSON_SCHEMA_VERSION = 2`, `TRACE_SCHEMA_VERSION = 1`, `RPC_SCHEMA_VERSION = 3`. All four values are unchanged from their Phase 7 exit state. The five RPC `error_code` values use the existing additive `SdkResponse::error_code` field introduced in Phase 7, so no wire-breaking change occurred.

Evidence: `crates/opi-agent/src/sdk.rs` line 42; `crates/opi-coding-agent/src/runner.rs` line 29; `crates/opi-agent/src/trace.rs` line 54.

**A-2.4 | Pass | API surface classification tiers are reasonable**

The five supported 0.x surfaces (`Agent`, `agent_loop`, `AgentHooks`, `Tool`, `AgentEvent`) are the core runtime abstractions that embedders must use. The seven unstable internal surfaces are either wire protocol types (`AgentSessionEvent`, `SdkCommand`, `SdkResponse`), extension infrastructure (`Extension`, `ExtensionRegistry`), storage internals (`SessionEntry`), or proxy plumbing (`StreamingProxy`). This split is defensible: the core runtime contract is tested and documented, while the wire/extension/storage shapes remain free to evolve.

Evidence: `crates/opi-agent/README.md` API Surface Classification table.

**A-2.5 | Note | `docs/agent-runtime-contracts.md` was not created**

The task 8.7 `task_owned_paths` listed `docs/agent-runtime-contracts.md` and `docs/agent-runtime-contracts.zh.md`, but these files were not created. All runtime contract documentation was placed in `crates/opi-agent/README.md` (EN and ZH) instead. This is a reasonable consolidation -- the README is the natural home for crate-level API documentation -- but the task_owned_paths are inaccurate.

Evidence: file system search; `crates/opi-agent/README.md` contains all contract sections.

---

## 3. Security and Privacy

**A-3.1 | Pass | Real-format redaction contract tested**

`phase8_real_format_redaction_contract` tests redaction against realistic credential formats: Anthropic API keys (`sk-ant-api03-...`), OpenAI keys (`sk-proj-...`, `sk-live-...`, `sk-svcacct-...`), credentialed URLs (`https://user:pass@host`), and absolute paths (`/Users/alice/.config/opi/config.toml`). Summary mode redacts all; verbose mode redacts secret patterns but keeps paths. Benign values (`model-name`, `https://api.example.com`) are preserved.

Evidence: `crates/opi-agent/tests/diagnostics.rs::phase8_real_format_redaction_contract`.

**A-3.2 | Pass | Doctor public diagnostic message redaction tested**

`phase8_public_diagnostic_message_redaction` verifies that both JSON and text doctor output formats redact API keys, passwords, and usernames, replacing them with `[REDACTED]`.

Evidence: `crates/opi-coding-agent/tests/doctor_cli.rs::phase8_public_diagnostic_message_redaction`.

**A-3.3 | Pass | Startup diagnostics are structured and redacted**

`phase8_startup_diagnostics_are_structured_and_redacted` verifies that NDJSON `StartupDiagnostics` (now `Vec<DiagnosticPayload>` instead of `Vec<String>`) does not contain seeded secrets in serialized output, contains `[REDACTED]`, and preserves `code`/`source`/`severity` fields.

Evidence: `crates/opi-coding-agent/tests/json_mode.rs::phase8_startup_diagnostics_are_structured_and_redacted`.

**A-3.4 | Note | AWS credentials and Azure tokens still not pattern-matched**

Inherited from Phase 7 (A-3.2): `SecretRedactor` does not have explicit patterns for AWS access keys (`AKIA...`), Azure AD tokens, or Google OAuth tokens. Partial coverage exists through the JWT regex and sensitive field-name matching. Phase 8 did not add new credential patterns.

Evidence: `crates/opi-agent/src/streaming_proxy.rs` regex list.

---

## 4. Test Coverage

**A-4.1 | Pass | 55 phase8_* tests cover all 7 tasks and all 8 Success Criteria**

| Task | SC | Test count | Files |
|------|----|--------:|-------|
| 8.1 | SC1 | 6 | `agent_loop_semantics.rs`, `hooks_queues.rs` |
| 8.2 | SC2 | 8 | `hooks_queues.rs`, `extensions.rs`, `adapter_runtime.rs` |
| 8.3 | SC3 | 8 | `agent_loop_semantics.rs`, `tool_validation.rs` |
| 8.4 | SC4 | 6 | `agent_loop_semantics.rs`, `rpc_jsonl.rs`, `adapter_runtime.rs`, `interactive_mock.rs`, `session_runtime.rs` |
| 8.5 | SC5 | 12 | `sdk_embedding.rs` (x2), `rpc_jsonl.rs` |
| 8.6 | SC7 | 12 | `trace_envelope.rs`, `diagnostics.rs`, `diagnostics_runtime.rs`, `rpc_jsonl.rs`, `json_mode.rs`, `adapter_runtime.rs`, `doctor_cli.rs`, `session_runtime.rs` |
| 8.7 | SC6, SC8 | 3 | `runtime_contract_docs.rs` |
| **Total** | | **55** | **16 files** |

**A-4.2 | Pass | Assertion quality is strong in sampled tests**

Sampled tests demonstrate: concrete event sequence assertions (not just "no error"), source-order vs completion-order verification, negative assertions (e.g., "no tool events", "no message_end"), cardinality assertions (exactly N provider calls, exactly 1 `agent_end`), and state persistence assertions (JSONL content after cancellation). Mock setup uses `RecordingProvider` with pre-defined response sequences, providing deterministic replay without network access.

**A-4.3 | Risk | Cancel-path TurnStarted/TurnEnded pairing is not pinned**

Phase 8 pins the provider-failure TurnEnded gap via `provider_failure_trace_may_leave_turn_open` (non-phase8 test). The phase8 cancellation tests assert `Err(Cancelled)` and `agent_end` but do not assert whether `TurnStarted` has a matching `TurnEnded` in cancel scenarios. The gap behavior is documented in the README cancellation section but not explicitly pinned by a phase8 test.

Evidence: `crates/opi-agent/tests/agent_loop_semantics.rs::phase8_cancellation_contract_during_stream` -- asserts no `message_end` but does not assert TurnStarted/TurnEnded pairing.

**A-4.4 | Note | Interactive TUI diagnostic path remains untested**

Inherited from Phase 7 (A-4.4). The interactive TUI mode does not enable `record_diagnostics` or trace collection by default. There are no tests verifying runtime diagnostics in interactive mode are surfaced to the user. Phase 8 `interactive_mock.rs` tests abort/shutdown but not diagnostic rendering.

---

## 5. Code Quality

**A-5.1 | Pass | Production changes are clean and well-integrated**

The `harness.rs` session-recovery wiring (6 lines) follows the existing `record_harness_diagnostic` pattern used by compaction. The `rpc.rs` error_code changes define four `const` values, a `response_error_with_code` helper that delegates to `SdkResponse::error_with_code`, and a const-pin unit test. The fifth code (`unsupported_trace_request`) uses a string literal at its single call site, pinned by an e2e RPC test.

Evidence: `crates/opi-coding-agent/src/rpc.rs` lines 90-95, 983-990.

**A-5.2 | Pass | Contract tests follow consistent patterns**

Tests use a shared helper infrastructure: `RecordingProvider` for deterministic replay, `recording_sink` / `agent_end_sink` for event capture, `make_context` for context construction, `MinimalHooks` / `StopAfterNHooks` for configurable hook behavior. This promotes consistency and reduces per-test boilerplate.

**A-5.3 | Note | Guard test `no_positive_claim` has a documented substring limitation**

The `no_positive_claim` helper uses per-line co-occurrence of negation words and positive claim substrings. A deceptive line placing a negation word next to a positive claim on the same line can bypass the guard. This limitation is shared with Phase 6/7 guards and is documented in the helper's doc-comment. The Phase 8 session notes state the helper was kept at the narrow Phase 7 baseline with the limitation documented.

Evidence: impl-state Phase 8 `audit_notes`; `crates/opi-coding-agent/tests/runtime_contract_docs.rs`.

**A-5.4 | Pass | Five RPC error_code values are const-pinned**

`error_code_constants_pin_documented_wire_values` (unit test in `rpc.rs`) asserts the four `ERR_*` constants match their documented wire strings. The fifth code (`unsupported_trace_request`) is covered by the e2e test `phase8_rpc_trace_production_reachable_and_scoped`.

Evidence: `crates/opi-coding-agent/src/rpc.rs` lines 983-990.

---

## 6. Documentation Consistency

**A-6.1 | Pass | EN/ZH README synchronized**

Guard test `phase8_api_surface_classification` verifies EN and ZH READMEs in parallel: classification section exists, all three tiers documented, each of 12 surfaces bound to its tier on a single line, no stable 1.0 promise, `#[non_exhaustive]` documented, and all three schema versions documented. Guard test `phase8_non_goals_not_claimed_or_implemented` checks 8 doc files (4 EN, 4 ZH) for forbidden positive claims.

Evidence: `crates/opi-coding-agent/tests/runtime_contract_docs.rs` 3 guard tests.

**A-6.2 | Pass | pi-alignment-matrix Phase 8 row is accurate**

Both EN and ZH alignment matrices contain a Phase 8 row documenting: feature family (agent runtime stabilization), crates (`opi-agent`, `opi-coding-agent`), pi manifestation (`pi-agent-core` runtime contracts), level (Partial), current state (runtime contracts documented and contract-tested), and next action (keep 0.x, explicit non-goals list).

Evidence: `docs/pi-alignment-matrix.md` line 68; `docs/pi-alignment-matrix.zh.md` line 68.

**A-6.3 | Pass | CHANGELOG entries are accurate**

Phase 8 entries under `## [Unreleased]`:
- **Added:** API surface classification (EN/ZH), pi-alignment-matrix Phase 8 row, guard tests.
- **Changed:** RPC JSONL rejection responses carry stable `error_code`; SDK_SCHEMA_VERSION remains 3.
- **Fixed:** Resumed session-recovery diagnostics wired to in-process diagnostic sink.

No Phase 8 entries appear in released version sections. All entries are under `[Unreleased]`.

Evidence: `CHANGELOG.md` lines 10-12 (Added), 14-15 (Changed).

**A-6.4 | Note | CHANGELOG says "SDK_SCHEMA_VERSION remains 3" but does not mention the bump was Phase 7**

The CHANGELOG Changed section for Phase 8 states `SDK_SCHEMA_VERSION` "remains" at 3, which is accurate. However, the phrasing could mislead a reader into thinking Phase 8 considered bumping it. The version was bumped from 2 to 3 during Phase 7 for the additive `error_code` field.

Evidence: `CHANGELOG.md`.

---

## 7. Phase 7 Handoff Item Closure Status

### A-1.3 / A-7.1: StartupDiagnostics shape

**A-7.1 | Partially closed**

`StartupDiagnostics` upgraded from `Vec<String>` to `Vec<DiagnosticPayload>` with `code`/`source`/`severity` fields. This is a material improvement. However, `SessionDiagnosticCounts` remains `{info, warning, error}` counts without per-diagnostic detail, and `run_summary` in RPC remains an ad-hoc JSON shape (see A-7.5).

Evidence: `crates/opi-agent/src/session_event.rs` lines 104-106; `crates/opi-coding-agent/tests/json_mode.rs::phase8_startup_diagnostics_are_structured_and_redacted`.

### A-7.2: Schema versions stabilization path

**A-7.2 | Partially closed**

Phase 8 documented the 0.x stability mechanism (API Surface Classification, `#[non_exhaustive]`, module `# Unstable` prose) but did not define a 1.0 stabilization path or breaking-change policy for schema versions. This is consistent with NG1 (no stable 1.0 promise) and is an intentional deferral.

Evidence: `crates/opi-agent/README.md` API Surface Classification section.

### A-7.3: TurnEnded not guaranteed on early exit

**A-7.3 | Partially closed**

Behavior unchanged: early exit from provider failure or cancellation may leave `TurnStarted` without `TurnEnded`. Provider failure path is now pinned by test `provider_failure_trace_may_leave_turn_open`. Cancel path is not explicitly pinned for turn pairing (see A-4.3). The README Cancellation section documents the behavior: partial streaming content is discarded, `AgentEnd` is always emitted.

Evidence: `crates/opi-agent/tests/trace_envelope.rs::provider_failure_trace_may_leave_turn_open`.

### A-7.4: RPC trace accumulates across runs

**A-7.4 | Closed**

`RecordingTraceSink::prepare()` calls `clear()` before each run, resetting the trace buffer. `prepare_trace_run()` triggers this through `TraceCollector::prepare()`. Test `phase8_rpc_trace_production_reachable_and_scoped` runs two sequential prompts and asserts all trace records share a single `run_id`, confirming per-run scoping.

Evidence: `crates/opi-agent/src/trace.rs` lines 350-353; `crates/opi-coding-agent/tests/rpc_jsonl.rs::phase8_rpc_trace_production_reachable_and_scoped`.

### A-7.5 / A-1.11: Adapter degradation as metadata strings

**A-7.5 | Partially closed**

`RuntimePackageStartup.diagnostics` is now `Vec<Diagnostic>` (typed, not strings). `diagnostic_bridge.rs` produces `SOURCE_ADAPTER` typed diagnostics. Test `phase8_adapter_degradation_diagnostic_contract` asserts `source`/`code`/structured `details`. However, adapter startup diagnostics enter `resource_metadata.diagnostics` and do not flow through `agent_loop`'s `observe()`, so they do not produce `DiagnosticLinked` trace records. This is because startup diagnostics are collected before the trace run begins.

Evidence: `crates/opi-coding-agent/tests/adapter_runtime.rs::phase8_adapter_degradation_diagnostic_contract`.

### A-7.6 / A-2.6: observe() divergence risk

**A-7.6 | Still open**

`observe()` in `agent_loop.rs` remains convention-enforced (comment + centralized function). No macro, lint, or compile-time guard ensures all diagnostic emissions flow through it. Phase 8 added characterization tests that verify specific paths produce `DiagnosticLinked` trace records, which provides regression coverage but not structural prevention.

Evidence: `crates/opi-agent/src/agent_loop.rs` `observe()` function.

### A-2.5: RPC run_summary ad-hoc shape

**A-7.7 | Still open**

`run_summary` in RPC is still emitted as raw `serde_json::json!()` outside `AgentSessionEvent`. JSON mode uses `AgentSessionEvent::SessionSummary`. This asymmetry persists from Phase 7.

Evidence: `crates/opi-coding-agent/src/rpc.rs` lines 594-601.

### A-4.3: TurnEnded gap untested

**A-7.8 | Partially closed**

Provider failure path is now pinned by test `provider_failure_trace_may_leave_turn_open`. Cancel path turn pairing is not explicitly tested (see A-4.3).

### A-4.4: Interactive TUI diagnostic path

**A-7.9 | Still open**

No Phase 8 test covers interactive TUI runtime diagnostic rendering. `interactive_mock.rs` tests abort/shutdown hooks, not diagnostic surfacing.

### A-4.5: adapter_host_mock exit code

**A-7.10 | Still open**

`adapter_host_mock.rs` remains `harness = false`. It still hangs when invoked directly by `cargo test`. The impl-state reports 2005 passed / 0 failed, suggesting CI runs this differently, but `cargo test --workspace --all-targets` still produces exit 1 in environments that do not kill the hung binary.

Evidence: `crates/opi-coding-agent/Cargo.toml` `harness = false`; audit gate run.

### A-3.2: AWS/Azure credential patterns

**A-7.11 | Still open**

No new credential patterns were added to `SecretRedactor`. AWS `AKIA` keys, Azure AD tokens, and Google OAuth tokens remain partially covered by JWT regex and field-name matching only.

### Summary table

| # | Phase 7 item | Phase 8 status |
|---|-------------|----------------|
| 1 | A-1.3/A-7.1 StartupDiagnostics shape | **Partially closed** |
| 2 | A-7.2 Schema versions unstable | **Partially closed** (intentional) |
| 3 | A-7.3 TurnEnded early exit | **Partially closed** |
| 4 | A-7.4 RPC trace per-run scope | **Closed** |
| 5 | A-7.5/A-1.11 Adapter degradation typed | **Partially closed** |
| 6 | A-7.6/A-2.6 observe() guard | **Still open** |
| 7 | A-2.5 run_summary ad-hoc | **Still open** |
| 8 | A-4.3 TurnEnded gap untested | **Partially closed** |
| 9 | A-4.4 Interactive TUI diagnostics | **Still open** |
| 10 | A-4.5 adapter_host_mock exit | **Still open** |
| 11 | A-3.2 AWS/Azure redaction | **Still open** |

---

## 8. Residual Risks and Phase 9 Handoff

### Phase 8 residuals

**A-8.1 | Risk | `no_positive_claim` substring co-occurrence limitation persists**

The Phase 6/7/8 guard tests share a per-line substring co-occurrence limitation. A deceptive line could bypass the guard. This is documented but not structurally resolved. Fully closing it requires clause-level parsing.

**A-8.2 | Risk | Cancel-path trace turn pairing is not pinned**

While provider failure TurnEnded gap is pinned, the cancel-path equivalent is not explicitly tested. A future change that emits or omits TurnEnded on cancel could go undetected.

**A-8.3 | Risk | RPC `run_summary` and `SessionDiagnosticCounts` remain ad-hoc**

These wire shapes are outside `AgentSessionEvent` and outside the Phase 8 API classification. A future SDK/RPC stabilization pass should decide whether to promote them to typed variants or remove them.

**A-8.4 | Risk | `observe()` has no structural divergence guard**

New diagnostic emit sites added outside `observe()` will silently break diagnostic-trace lockstep. This is a maintenance hazard that grows with codebase size.

**A-8.5 | Risk | `adapter_host_mock` exit code remains misleading**

`cargo test --workspace --all-targets` reports exit 1 in standard invocations. This makes CI gate interpretation ambiguous and may mask real failures.

### Phase 9 constraints from Phase 8 contracts

Phase 9 is expected to improve built-in tool correctness. Any tool change must respect:

1. **Tool scheduling contract:** Default `Parallel` execution mode; `Sequential` override forces entire batch; persisted results in source order; all-terminate required for early stop.

2. **Hook execution order:** `before_tool_call` runs after JSON Schema validation and before `execute`; `after_tool_call` runs after execution and before final event. Tool changes must not reorder these.

3. **Cancellation contract:** Cancelled tool execution returns `ToolError::Cancelled` or an error result, never a hang. Tool results from cancelled turns are not persisted.

4. **Validation failure contract:** Schema validation failure is a normal runtime outcome (error `ToolResult`, `is_error = true`, `terminate = false`), not a loop error. Tool schema changes must not break this.

5. **SDK/RPC wire stability:** Tool output changes visible through `ToolResult` content in RPC events must not break existing `SDK_SCHEMA_VERSION = 3` consumers. If tool output shape changes are wire-visible, bump `SDK_SCHEMA_VERSION`.

### Suggestions for Phase 9

1. **Pin cancel-path TurnEnded behavior** with an explicit contract test, consistent with the existing provider-failure pin.

2. **Add `AKIA` AWS access key pattern** to `SecretRedactor` if Bedrock tools produce AWS credential diagnostics.

3. **Isolate `adapter_host_mock`** from `cargo test` (e.g., `cfg(feature = "adapter-test-binary")` or `ignore` annotation) to restore clean exit 0 from `cargo test --workspace`.

4. **Consider a structured `observe()` guard** (trait, macro, or test that enumerates all diagnostic emit sites) to prevent diagnostic-trace divergence as tool code paths grow.

5. **If tool output shapes change** in a wire-visible way, promote `run_summary` and `SessionDiagnosticCounts` to typed `AgentSessionEvent` variants in the same SDK version bump.

---

## Appendix A: Full Gate Results

```
Gate: cargo fmt --check --all
Result: Clean (exit 0)

Gate: cargo clippy --workspace --all-targets -- -D warnings
Result: Clean (exit 0)

Gate: cargo test --workspace --all-targets
Result: Exit 1 (adapter_host_mock harness=false binary hang; all actual tests pass)
  - 53 completed test suites report 0 failures
  - adapter_host_mock is a mock subprocess binary, not a test suite
  - Impl-state at Phase 8 exit reports 2005 passed / 0 failed

Gate: RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
Result: Clean (exit 0)

Phase 8 specific tests (all pass):
  opi-agent agent_loop_semantics:   13 phase8_* passed
  opi-agent hooks_queues:            7 phase8_* passed
  opi-agent extensions:              2 phase8_* passed
  opi-agent tool_validation:         1 phase8_* passed
  opi-agent trace_envelope:          4 phase8_* passed
  opi-agent diagnostics:             1 phase8_* passed
  opi-agent diagnostics_runtime:     1 phase8_* passed
  opi-agent sdk_embedding:           3 phase8_* passed
  opi-coding-agent rpc_jsonl:        8 phase8_* passed
  opi-coding-agent adapter_runtime:  3 phase8_* passed
  opi-coding-agent interactive_mock: 1 phase8_* passed
  opi-coding-agent session_runtime:  2 phase8_* passed
  opi-coding-agent json_mode:        1 phase8_* passed
  opi-coding-agent doctor_cli:       1 phase8_* passed
  opi-coding-agent sdk_embedding:    4 phase8_* passed
  opi-coding-agent runtime_contract_docs: 3 phase8_* passed
  Total:                            55 phase8_* passed
```

---

## Appendix B: Finding Index

| ID | Severity | Title |
|----|----------|-------|
| A-1.1 | Pass | Event order is documented and tested |
| A-1.2 | Pass | Queue polling order documented and tested |
| A-1.3 | Pass | Hook order and effects are documented and tested |
| A-1.4 | Pass | Extension hook composition tested |
| A-1.5 | Pass | Skipped adapter hooks visible in trace |
| A-1.6 | Pass | Tool scheduling covers parallel, sequential, mixed, and termination |
| A-1.7 | Pass | Cancellation is tested across all documented paths |
| A-1.8 | Pass | SDK/RPC command-state contract is comprehensive |
| A-1.9 | Note | SDK_SCHEMA_VERSION was already 3 before Phase 8 |
| A-1.10 | Pass | API surface classification is documented and guard-tested |
| A-1.11 | Pass | Classification matches actual crate visibility |
| A-1.12 | Pass | Runtime contract failures produce diagnostics and trace records |
| A-1.13 | Pass | Session-recovery diagnostics wired to in-process sink |
| A-1.14 | Pass | Non-goals verified by automated guard |
| A-1.15 | Pass | All 8 Non-Goals confirmed absent |
| A-2.1 | Pass | Contract tests use production agent_loop path |
| A-2.2 | Pass | Production changes are minimal and necessary |
| A-2.3 | Pass | No schema version was bumped during Phase 8 |
| A-2.4 | Pass | API surface classification tiers are reasonable |
| A-2.5 | Note | docs/agent-runtime-contracts.md was not created |
| A-3.1 | Pass | Real-format redaction contract tested |
| A-3.2 | Pass | Doctor public diagnostic message redaction tested |
| A-3.3 | Pass | Startup diagnostics are structured and redacted |
| A-3.4 | Note | AWS credentials and Azure tokens still not pattern-matched |
| A-4.1 | Pass | 55 tests cover all 7 tasks and all 8 Success Criteria |
| A-4.2 | Pass | Assertion quality is strong in sampled tests |
| A-4.3 | Risk | Cancel-path TurnStarted/TurnEnded pairing is not pinned |
| A-4.4 | Note | Interactive TUI diagnostic path remains untested |
| A-5.1 | Pass | Production changes are clean and well-integrated |
| A-5.2 | Pass | Contract tests follow consistent patterns |
| A-5.3 | Note | Guard test no_positive_claim has a documented substring limitation |
| A-5.4 | Pass | Five RPC error_code values are const-pinned |
| A-6.1 | Pass | EN/ZH README synchronized |
| A-6.2 | Pass | pi-alignment-matrix Phase 8 row is accurate |
| A-6.3 | Pass | CHANGELOG entries are accurate |
| A-6.4 | Note | CHANGELOG phrasing about SDK_SCHEMA_VERSION |
| A-7.1 | -- | StartupDiagnostics shape: partially closed |
| A-7.2 | -- | Schema versions: partially closed (intentional) |
| A-7.3 | -- | TurnEnded early exit: partially closed |
| A-7.4 | -- | RPC trace per-run scope: closed |
| A-7.5 | -- | Adapter degradation typed: partially closed |
| A-7.6 | -- | observe() guard: still open |
| A-7.7 | -- | run_summary ad-hoc: still open |
| A-7.8 | -- | TurnEnded gap untested: partially closed |
| A-7.9 | -- | Interactive TUI diagnostics: still open |
| A-7.10 | -- | adapter_host_mock exit: still open |
| A-7.11 | -- | AWS/Azure redaction: still open |
| A-8.1 | Risk | no_positive_claim substring limitation persists |
| A-8.2 | Risk | Cancel-path trace turn pairing not pinned |
| A-8.3 | Risk | RPC run_summary and SessionDiagnosticCounts remain ad-hoc |
| A-8.4 | Risk | observe() has no structural divergence guard |
| A-8.5 | Risk | adapter_host_mock exit code remains misleading |
