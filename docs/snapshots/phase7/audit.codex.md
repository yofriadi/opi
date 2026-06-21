# Phase 7 Reliability / Observability Audit

Date: 2026-06-19

Auditor: Codex

## Scope

Primary inputs:

- `docs/snapshots/phase7/opi-impl-state.json`
- `docs/superpowers/specs/2026-06-15-phase7-reliability-observability-design.md`

Implementation areas reviewed:

- `opi-agent` diagnostic, trace, agent loop, compaction, session recovery, SDK, and tool contracts.
- `opi-coding-agent` doctor, JSON mode, RPC, harness, runtime package startup, resource discovery, adapter startup, and CLI wiring.
- Provider error classification paths in `opi-ai`.
- Phase 7-focused integration tests in `opi-agent` and `opi-coding-agent`.

This audit is document-only. It does not change implementation behavior.

## Verification Performed

Commands run:

- `cargo test -p opi-agent --test diagnostics --test diagnostics_runtime --test trace_envelope`
  - Passed: `diagnostics` 17 tests, `diagnostics_runtime` 31 tests, `trace_envelope` 29 tests.
- `cargo test -p opi-coding-agent --test doctor_cli --test json_mode --test rpc_jsonl --test observability_docs phase7`
  - Passed: `doctor_cli` 2 tests, `json_mode` 4 tests, `rpc_jsonl` 5 tests, `observability_docs` 4 tests.
  - Note: `diagnostics_runtime` was included in the first filtered command but the filter matched 0 tests for that target.
- `cargo test -q -p opi-coding-agent --test diagnostics_runtime`
  - Passed: 5 tests.

Not run for this audit:

- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`

The omitted gates are appropriate for a source change gate, but this audit did not modify Rust code.

## Executive Summary

Phase 7 is materially implemented: the shared `Diagnostic` model, redaction substrate, trace envelope, `opi doctor`, JSON/RPC startup diagnostics, diagnostic counts, and explicit local trace controls all exist and have focused tests.

However, this audit does not fully agree with the phase ledger's statement that all 8 success criteria are met without deferral. Several requirements are satisfied only at the classification or test-helper layer, while production paths still expose raw strings or miss diagnostics entirely.

The highest-risk gaps are:

1. Tool executions that return `Ok(ToolResult { is_error: true })` are traced as successful completions and do not emit diagnostics.
2. Startup/resource/package/adapter degradation still flows as raw `Vec<String>` diagnostics and can bypass structured redaction before JSON/RPC/system-prompt exposure.
3. Session recovery diagnostics are implemented in `opi-agent` but dropped by the production resume path.
4. Compaction diagnostics are partly traced but not recorded in the diagnostic sink, and manual/RPC compaction is not represented.
5. The embedded RPC runner supports trace, but the top-level `opi --rpc` path does not enable it despite documentation saying the RPC `trace` command can produce traces.

These are not cosmetic. They affect whether Phase 7's reliability/observability contract is consistently true on real user paths.

## Findings

### C7-AUD-01 - P1 - Tool result errors bypass diagnostics and are traced as success

Phase 7 requires runtime failures to be represented in diagnostics and, where stable, trace records. The `ToolResult` contract explicitly has `is_error: bool` (`crates/opi-agent/src/tool.rs:34`), and many built-in tools return `Ok(ToolResult { is_error: true })` for user-visible failures.

In `execute_tool`, only these paths emit `CODE_TOOL_EXECUTION_FAILED`:

- unknown tool
- JSON-schema validation failure
- hook-denied tool call
- `tool.execute(...)` returning `Err(e)`

When a tool returns `Ok(result)`, the code runs the `after_tool_call` hook and unconditionally emits `TraceKind::ToolCallCompleted` (`crates/opi-agent/src/agent_loop.rs:638`, `crates/opi-agent/src/agent_loop.rs:650`). It does not inspect `final_result.is_error`.

Impact:

- Built-in tool failures such as read/write/edit/bash/grep/glob/ls/find failures can be visible to the model as tool errors but invisible to diagnostic counts.
- Trace consumers see these failures as completed tool calls.
- Phase 7 tests cover the `Err(e)` path but do not pin the dominant built-in `Ok(is_error=true)` path.

Recommended fix:

- After `AfterToolCallResult`, if `final_result.is_error` is true, emit a shared diagnostic with source `tool` and a stable code such as `tool_execution_failed` or a more precise new code.
- Emit `TraceKind::ToolCallFailed`, or introduce a distinct stable trace classification if the team wants to preserve the current "execution returned normally" meaning.
- Add a regression test with a mock tool returning `Ok(ToolResult { is_error: true })`.

### C7-AUD-02 - P1 - Startup and adapter degradation diagnostics remain raw strings

The Phase 7 ledger already records a residual: "SC5 harness startup surfaces adapter degradation as metadata strings, not typed Diagnostic+observe()" (`docs/snapshots/phase7/opi-impl-state.json:1479`). The implementation confirms this is broader than adapter degradation.

Examples:

- `RuntimePackageStartup` stores `diagnostics: Vec<String>` (`crates/opi-coding-agent/src/runtime_packages.rs:12`, `crates/opi-coding-agent/src/runtime_packages.rs:15`).
- Installed package diagnostics are formatted with raw scope/source/code/message strings (`crates/opi-coding-agent/src/runtime_packages.rs:59`).
- Adapter startup pushes raw formatted strings for protocol and startup failures (`crates/opi-coding-agent/src/adapter_extension.rs:782`, `crates/opi-coding-agent/src/adapter_extension.rs:792`, `crates/opi-coding-agent/src/adapter_extension.rs:806`, `crates/opi-coding-agent/src/adapter_extension.rs:835`, `crates/opi-coding-agent/src/adapter_extension.rs:842`).
- Harness resource metadata stores `diagnostics: Vec<String>` (`crates/opi-coding-agent/src/harness.rs:93`, `crates/opi-coding-agent/src/harness.rs:99`).
- These strings are inserted into the system prompt (`crates/opi-coding-agent/src/harness.rs:147`) and exported through RPC JSON (`crates/opi-coding-agent/src/harness.rs:171`, `crates/opi-coding-agent/src/harness.rs:178`).

Impact:

- These diagnostics are not counted or classified through the shared diagnostic model.
- They bypass `Diagnostic::redacted_details(...)`.
- Raw package sources, adapter commands, absolute paths, or credentialed URLs can leak into JSON/RPC output or the system prompt if upstream strings contain them.
- This weakens success criteria 1, 3, 5, and 6.

Recommended fix:

- Replace or supplement startup `Vec<String>` diagnostics with `Vec<Diagnostic>`.
- Convert package and adapter degradation through shared diagnostic constructors.
- Redact diagnostic details at every public serialization boundary.
- Avoid injecting raw diagnostic details into the model prompt; use stable code/source/severity summaries unless the detail is explicitly safe.
- Add regression tests with a credentialed package source URL and absolute adapter command path through JSON startup diagnostics, `rpc_ready`, and resource prompt formatting.

### C7-AUD-03 - P2 - Session recovery diagnostics are classified but dropped by resume

`opi-agent` implements shared diagnostics for crash recovery (`crates/opi-agent/src/session.rs:135`). It covers truncated trailing lines and corrupt skipped entries (`crates/opi-agent/src/session.rs:139`, `crates/opi-agent/src/session.rs:145`, `crates/opi-agent/src/session.rs:154`).

The production resume path does not carry these diagnostics forward:

- `ResumedSession` stores only `skipped_entries: usize` (`crates/opi-coding-agent/src/session_cli.rs:31`, `crates/opi-coding-agent/src/session_cli.rs:38`).
- `resume_session` calls `recovery.corrupt_count()` and drops `recovery.diagnostics()` (`crates/opi-coding-agent/src/session_cli.rs:124`, `crates/opi-coding-agent/src/session_cli.rs:134`).
- The CLI prints an ad-hoc warning string for corrupt entries (`crates/opi-coding-agent/src/session_cli.rs:243`).

Impact:

- Session recovery diagnostics exist in the library but are not exposed through the new shared shape on the real resume path.
- Truncated-line recovery can be completely invisible to the CLI resume warning because `corrupt_count()` is 0 for `TruncatedLine`.
- This weakens success criterion 5 and the 7.2 runtime-failure diagnostic intent.

Recommended fix:

- Add `diagnostics: Vec<Diagnostic>` to `ResumedSession`.
- Emit or expose those diagnostics through JSON/RPC/non-interactive startup diagnostics and any relevant text-mode warning path.
- Add integration tests for corrupt-line and truncated-line resume behavior.

### C7-AUD-04 - P2 - Compaction diagnostics are not consistently emitted

`CompactionOutput::diagnostic()` exists and returns `CODE_SESSION_COMPACTED` (`crates/opi-agent/src/compaction.rs:75`, `crates/opi-agent/src/compaction.rs:78`). In production harness code, automatic compaction builds a similar diagnostic and sends it only to `trace_diagnostic(...)` (`crates/opi-coding-agent/src/harness.rs:1181`).

`trace_diagnostic(...)` mirrors to the active trace, but it does not record into the installed `RecordingSink` diagnostic sink (`crates/opi-coding-agent/src/harness.rs:1320`). Manual/RPC compaction uses a separate `compact()` path and does not emit the compaction diagnostic (`crates/opi-coding-agent/src/harness.rs:1474`).

Impact:

- Automatic compaction can appear in trace when tracing is active, but not in diagnostic counts.
- Manual/RPC compaction is not represented in diagnostics or trace.
- The shared `CompactionOutput::diagnostic()` helper is effectively test/substrate code, not the production source of truth.

Recommended fix:

- Reuse `CompactionOutput::diagnostic()` in harness production paths.
- Record compaction diagnostics into the same diagnostic sink used for run summaries.
- Decide whether manual/RPC compaction is in Phase 7 scope; if yes, trace/diagnose it, and if no, document the explicit deferral.

### C7-AUD-05 - P2 - Top-level `opi --rpc` does not enable the RPC trace path

The RPC implementation supports traces when constructed with `RpcRunner::new_with_trace(...)` (`crates/opi-coding-agent/src/rpc.rs:134`). If no trace sink is installed, the `trace` SDK command returns `unsupported_trace_request` (`crates/opi-coding-agent/src/rpc.rs:790`).

The top-level CLI RPC path calls `RpcRunner::new_with_runtime_packages(...)`, not `new_with_trace(...)` (`crates/opi-coding-agent/src/main.rs:359`, `crates/opi-coding-agent/src/main.rs:408`). The `--trace` CLI flag is documented as non-interactive/JSON only (`crates/opi-coding-agent/src/cli.rs:89`), and is not wired into `run_rpc`.

The public spec says: "A trace is produced only when explicitly requested via the `--trace` CLI flag or the RPC `trace` command" (`docs/opi-spec.md:1053`).

Impact:

- Embedded tests can exercise `new_with_trace`, but the real `opi --rpc` binary path appears to always reject `trace` requests.
- Documentation overstates the CLI/RPC capability.

Recommended fix:

- Either wire a trace sink into top-level `opi --rpc` when requested, or update documentation to say only embedded RPC runners can enable trace today.
- Add a binary-level or integration-level test for `opi --rpc` trace behavior.

### C7-AUD-06 - P3 - RPC trace responses aggregate session records without a run selector

`TraceCollector` is documented as collecting records for a single run (`crates/opi-agent/src/trace.rs:152`). RPC stores a long-lived `RecordingTraceSink`, and the `trace` command returns `sink.snapshot()` for all records currently in the sink (`crates/opi-coding-agent/src/rpc.rs:772`).

Impact:

- Multiple prompt runs in the same RPC session can produce one accumulated trace response.
- Each record includes `run_id`, so this is not data loss, but the envelope has no run selector or explicit "session trace" semantics.

Recommended fix:

- Clear the sink at run start, add a run selector to the trace command, or document the RPC trace response as a session-level collection of per-run records.
- Add a test with two prompt runs followed by one `trace` command.

### C7-AUD-07 - P3 - Dynamic diagnostic messages are not uniformly redacted

Phase 7 redaction is strong for structured details and trace output, but `Diagnostic::redacted_details(...)` does not redact `message` or `action`. Provider error constructors can include raw HTTP response bodies in error messages before classification.

Impact:

- If a provider returns a body containing sensitive content, a human-facing diagnostic or stderr path can expose it even if structured details are redacted.
- This is less direct than C7-AUD-02 because many public Phase 7 surfaces serialize redacted details correctly, but message fields remain a residual leak surface.

Recommended fix:

- Keep diagnostic messages short and static where possible.
- Move volatile provider bodies into details and redact/summarize them at output boundaries.
- Add a provider-error redaction test with a bearer token or credentialed URL inside a mocked HTTP error body.

## Success Criteria Coverage

| Criterion | Audit result | Notes |
| --- | --- | --- |
| SC1: shared diagnostic shape exists and is used by new Phase 7 surfaces | Partially met | Core shape exists and many surfaces use it. Startup/resource/adapter paths still use raw strings. |
| SC2: `opi doctor` reports configured scopes without network calls | Met | Doctor implementation and focused tests cover the requested scopes and redacted JSON details. |
| SC3: JSON/RPC startup and run summaries expose structured diagnostic counts | Partially met | Run summaries expose structured counts. Startup diagnostics remain raw strings. |
| SC4: local redacted trace envelope can be requested for a run | Partially met | Non-interactive `--trace` and embedded RPC trace are implemented. Top-level `opi --rpc` does not enable trace. |
| SC5: retry, cancellation, compaction, adapter degradation, and provider failures represented in diagnostics/trace | Partially met | Provider/retry/cancel paths are represented. Tool-result errors, startup adapter degradation, session recovery, and manual/RPC compaction have gaps. |
| SC6: redaction tests cover secrets, prompt content, and tool output | Partially met | Guard coverage exists, but raw startup strings and dynamic messages remain outside the strongest redaction boundary. |
| SC7: docs state observability is local and explicit | Met | Documentation guard tests and spec text support this. |
| SC8: no telemetry/ecosystem/OAuth/marketplace/web dashboard added | Met | No evidence found of these non-goals being implemented. |

## Recommended Follow-up Issues

1. Close the `ToolResult::is_error` diagnostic/trace gap and add a regression test.
2. Migrate startup/resource/package/adapter diagnostics from raw strings to shared `Diagnostic` values.
3. Carry `CrashRecovery::diagnostics()` through `ResumedSession` and expose it on real resume paths.
4. Route compaction diagnostics through the shared sink and decide/manual-test the RPC compaction contract.
5. Reconcile RPC trace documentation with top-level `opi --rpc` behavior.
6. Add redaction tests for startup diagnostics and provider error bodies.

## Final Assessment

Phase 7 should be treated as functionally close, but not fully audit-clean against its own success criteria. The current implementation has a solid observability substrate and good focused test coverage, but several real production paths still bypass that substrate.

The phase can be closed only if the team explicitly accepts the raw-string/startup/session/compaction/RPC-trace gaps as deferred to Phase 8. Otherwise, the P1 and P2 findings above should be fixed before calling Phase 7 complete.
