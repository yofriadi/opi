# Phase 8 Agent Runtime Stabilization Audit

Date: 2026-06-24

Auditor: Codex

## Scope

Primary inputs:

- `docs/snapshots/phase8/opi-impl-state.json`
- `docs/superpowers/specs/2026-06-15-phase8-agent-runtime-stabilization-design.md`

Implementation baseline:

- Phase 8 final implementation commit: `1f270b8e3ace885bf38a1135d5e2b5e8789d552f`
- Current audited `HEAD`: `ef3424bab054995d7c57db3e903c38a54b8732ad`
- The only commit after the Phase 8 final implementation commit adds the archived Phase 8 ledger snapshot.

Implementation areas reviewed:

- `opi-agent` agent loop, hook composition, tool scheduling, cancellation, diagnostics, trace, SDK, extension, and API-surface documentation.
- `opi-coding-agent` RPC runner, harness, adapter bridge, runtime package startup, JSON mode, session recovery, doctor diagnostics, and Phase 8 documentation guards.
- Phase 8-focused tests and the Phase 7 Codex audit residuals that Phase 8 claimed to close.

This audit is document-only. It does not change implementation behavior.

## Verification Performed

Commands run:

- `cargo test -p opi-agent --test agent_loop_semantics phase8`
  - Passed: 13 tests.
- `cargo test -p opi-agent --test hooks_queues phase8`
  - Passed: 7 tests.
- `cargo test -p opi-agent --test extensions phase8`
  - Passed: 2 tests.
- `cargo test -p opi-agent --test tool_validation phase8`
  - Passed: 1 test.
- `cargo test -p opi-agent --test diagnostics --test diagnostics_runtime --test trace_envelope phase8`
  - Passed: 6 tests across the three targets.
- `cargo test -p opi-agent --test sdk_embedding phase8`
  - Passed: 3 tests.
- `cargo test -p opi-coding-agent --test runtime_contract_docs phase8`
  - Passed: 3 tests.
- `cargo test -p opi-coding-agent --test rpc_jsonl phase8`
  - Passed: 8 tests.
- `cargo test -p opi-coding-agent --test json_mode phase8`
  - Passed: 1 test.
- `cargo test -p opi-coding-agent --test adapter_runtime phase8`
  - Passed: 3 tests.
- `cargo test -p opi-coding-agent --test interactive_mock phase8`
  - Passed: 1 test.
- `cargo test -p opi-coding-agent --test session_runtime phase8`
  - Passed: 2 tests.
- `cargo test -p opi-coding-agent --test doctor_cli phase8`
  - Passed: 1 test.
- `cargo test -p opi-coding-agent --test sdk_embedding phase8`
  - Passed: 4 tests.

Not run for this audit:

- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- `scripts\opi-impl-smoke.ps1`

The omitted gates are appropriate for code-change and release gates. This audit only added this Markdown file.

## Executive Summary

Phase 8 is materially implemented. Runtime event order, hook order, extension hook composition, tool scheduling, cancellation, SDK/RPC command state, startup diagnostics, trace exposure, and non-goal guards all have focused tests, and every Phase 8 filtered test run during this audit passed.

The Phase 7 Codex audit residuals are mostly closed:

- `ToolResult { is_error: true }` now emits a tool failure diagnostic and `ToolCallFailed` trace.
- Startup/package/adapter degradation now flows as shared `Diagnostic` values and public payloads are redacted.
- Resume recovery diagnostics are carried through `ResumedSession` and startup metadata.
- Automatic compaction diagnostics are recorded through the harness diagnostic sink and trace mirror.
- Production RPC now installs a recording trace sink through `new_with_runtime_packages`.
- RPC trace records are scoped to the latest run because `RecordingTraceSink::prepare` clears the sink.
- Public diagnostic messages/actions are redacted through `Diagnostic::redacted_payload`.

This audit found no P0/P1 blockers. It found two P2 contract/guard gaps:

1. Malformed JSON tool-call arguments are silently converted to `{}` before schema validation, so permissive schemas can execute on invalid provider arguments.
2. The Phase 8 API surface classification does not classify several crate-root public re-exports, despite the design asking to classify public `opi-agent` items.

## Findings

### C8-AUD-01 - P2 - Malformed tool-call JSON can execute as empty arguments

Phase 8 requires a tool validation failure contract, and the implementation correctly validates parsed JSON before `before_tool_call` and before `Tool::execute`.

However, the production parser currently discards malformed argument strings:

- Sequential path: `serde_json::from_str(&tc.arguments).unwrap_or(json!({}))` in `crates/opi-agent/src/agent_loop.rs:178`.
- Parallel path: `serde_json::from_str(&tc.arguments).unwrap_or(json!({}))` in `crates/opi-agent/src/agent_loop.rs:228`.
- Validation only sees the fallback `args` value in `execute_tool` at `crates/opi-agent/src/agent_loop.rs:589`.

The Phase 8 validation test covers schema-invalid but syntactically valid JSON:

- `tool_call_response("call-1", "greet", r#"{}"#)` in `crates/opi-agent/tests/tool_validation.rs:459`.
- The test proves missing required fields stop hooks and execution, but it does not cover malformed JSON such as `{not-json`.

Impact:

- A tool with a permissive schema, such as `{ "type": "object" }`, can execute even though the provider emitted syntactically invalid tool arguments.
- Hooks receive `{}` rather than the actual malformed argument state, so policy logic cannot distinguish "empty object" from "unparseable arguments".
- Runtime diagnostics and trace classify the call as normal execution if the tool succeeds.

Recommended fix:

- Treat `serde_json::from_str` failure as a tool validation/runtime failure, not as `{}`.
- Persist an error `ToolResult` with a stable diagnostic such as `tool_validation_failed`.
- Do not run `before_tool_call` or `Tool::execute` when argument parsing fails.
- Add regression tests for malformed JSON against both permissive and required-field schemas.

### C8-AUD-02 - P2 - API surface classification omits public crate-root re-exports

The Phase 8 design says public `opi-agent` items should be classified as `supported 0.x`, `unstable internal`, or `candidate removal`, and Success Criterion 6 says public surfaces are classified.

The README classification table covers these rows:

- `Agent`, `agent_loop`, `AgentHooks`, `Tool`, and `AgentEvent` as supported 0.x.
- `AgentSessionEvent`, `SessionEntry`, `Extension`, `ExtensionRegistry`, `SdkCommand`, `SdkResponse`, and streaming-proxy primitives as unstable internal.

But `crates/opi-agent/src/lib.rs` publicly re-exports more crate-root API than the table classifies:

- Diagnostics: `Diagnostic`, `DiagnosticPayload`, `RedactionMode`, `Severity`, `redact`, `redact_text`.
- Diagnostic sinks: `DiagnosticSink`, `NullSink`, `RecordingSink`.
- Loop/message/state items: `AgentError`, `AgentLoopConfig`, `AgentLoopContext`, `AgentMessage`, `AgentState`.
- Tool adjuncts: `ExecutionMode`, `ToolError`, `ToolResult`, `ToolDef`.
- Trace surface: `FileTraceSink`, `RecordingTraceSink`, `TRACE_SCHEMA_VERSION`, `TraceCollector`, `TraceError`, `TraceKind`, `TraceRecord`, `TraceSink`.

The guard test only pins a selected subset of exact `pub use` lines in `crates/opi-coding-agent/tests/runtime_contract_docs.rs:197`. It does not fail when new public crate-root re-exports are added without a classification row.

Impact:

- Embedders can reasonably interpret unclassified crate-root exports as part of the public API, especially because several are re-exported for convenience.
- The current guard proves the selected table rows remain true, but it does not prove all public crate-root surfaces have an explicit tier.
- This weakens the "no stable 1.0 promise, but honest 0.x surface classification" contract.

Recommended fix:

- Add classification rows or an explicit catch-all policy for every crate-root `pub use`.
- If some re-exports are intentionally outside Phase 8 classification, say that directly in the README.
- Strengthen `runtime_contract_docs.rs` to parse or enumerate crate-root `pub use` exports and require each one to be classified or explicitly exempted.

## Non-Blocking Observations

### Cancellation Persistence Semantics Need a Sharper Name

The cancellation documentation says session persistence records only finalized state. The actual harness behavior is stricter: if a run returns `Err(AgentError::Cancelled)`, the whole turn is not persisted.

That matches the Phase 8 test `phase8_cancel_persists_only_finalized_state`, which verifies a cancelled second turn contributes no session messages. It also means finalized in-memory messages from a turn that later observes cancellation are not persisted. This is a defensible conservative policy, but the phrase "only finalized state" can be read as "persist all finalized messages before cancellation."

Recommended follow-up:

- Rephrase docs to "cancelled turns are not persisted; successful turns persist only finalized `AgentMessage::Llm` entries."
- Add a test if the team wants to preserve or reject finalized tool results produced before a later cancellation observation.

## Success Criteria Coverage

| Criterion | Audit result | Notes |
| --- | --- | --- |
| SC1: runtime event order documented and contract-tested | Met | `agent_loop_semantics phase8` and `hooks_queues phase8` passed. |
| SC2: hook order and failure semantics documented and tested | Met | Base hook order, validation-before-before-hook, after replacement, terminal prepare skip, and extension composition passed. |
| SC3: tool scheduling and termination semantics tested | Mostly met | Parallel/sequential/mixed/order/termination tests passed. Malformed JSON argument parsing remains a validation-contract gap. |
| SC4: cancellation has a consistent observable contract | Met with wording risk | Provider, RPC, adapter, interactive, and session persistence tests passed. See cancellation wording observation. |
| SC5: SDK/RPC command behavior documented, versioned, and tested | Met | SDK/RPC command-state tests passed; `SDK_SCHEMA_VERSION = 3` is pinned. |
| SC6: public `opi-agent` surfaces classified | Partially met | Required named surfaces are classified, but several crate-root re-exports remain unclassified. |
| SC7: diagnostics/traces cover runtime contract failures | Met | Phase 8 diagnostics/trace tests passed; Phase 7 residuals are largely closed. |
| SC8: no ecosystem expansion or workflow-heavy core feature | Met | No `opi-web-ui` workspace member, no shared `opi-types` crate, no new adapter protocol, and non-goal guards passed. |

## Recommended Follow-up Issues

1. Fix malformed tool-call JSON handling and add regression coverage for permissive schemas.
2. Complete or explicitly bound the `opi-agent` API surface classification for every crate-root re-export.
3. Clarify cancellation persistence wording so "finalized state" cannot be misread as "persist finalized fragments from cancelled turns."

## Final Assessment

Phase 8 can be considered functionally complete against the implementation ledger, with no release-blocking runtime failures found in this audit. The remaining issues are contract precision problems: one real tool-argument edge case and one API classification guard gap.

The phase should be closed only with those P2 items recorded for follow-up, or fixed before a stronger public runtime/API claim is made.
