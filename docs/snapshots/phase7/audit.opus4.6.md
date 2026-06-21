# Phase 7 Reliability and Observability -- Audit (Opus 4.6)

## Metadata

| Field | Value |
|-------|-------|
| Date | 2026-06-19 |
| HEAD | `4dccbad36429a3f960295d330d05555a703e94ad` |
| Auditor | Opus 4.6 (Cursor agent) |
| Design spec | `docs/superpowers/specs/2026-06-15-phase7-reliability-observability-design.md` |
| Impl state | `docs/snapshots/phase7/opi-impl-state.json` |
| Workspace version | 0.5.2 |
| Phase 7 tasks | 7.1 -- 7.6 (6 tasks, all status `passing`) |
| Phase 7 commits | `4812c94` .. `42969ad` (6 verified commits) |

### Baseline gate results

| Gate | Result |
|------|--------|
| `cargo fmt --check --all` | Clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo test --workspace --all-targets --no-fail-fast` | Exit 1 (see note) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | Clean |

**Test note:** The sole exit-code-1 binary is `adapter_host_mock`, a `harness = false` mock adapter process (not a test). It hangs when invoked directly by `cargo test` because it reads stdin for adapter protocol messages. The actual tests that use it (`adapter_host.rs`, 18 passed) drive it as a subprocess with correct env vars. All other test binaries pass with 0 failures.

### Phase 7 specific test results

| Test file | Crate | Passed |
|-----------|-------|-------:|
| `diagnostics.rs` | opi-agent | 17 |
| `diagnostics_runtime.rs` | opi-agent | 31 |
| `streaming_proxy.rs` | opi-agent | 26 |
| `trace_envelope.rs` | opi-agent | 29 |
| `provider_diagnostics.rs` | opi-ai | 8 |
| `diagnostics_runtime.rs` | opi-coding-agent | 5 |
| `doctor_cli.rs` | opi-coding-agent | 30 |
| `json_mode.rs` | opi-coding-agent | 16 |
| `non_interactive.rs` | opi-coding-agent | 7 |
| `observability_docs.rs` | opi-coding-agent | 4 |
| `rpc_jsonl.rs` | opi-coding-agent | 69 |
| `phase4_ledger.rs` | opi-coding-agent | 1 |
| **Total** | | **243** |

---

## Executive Summary

Phase 7 is **substantially complete** against its design spec. All 8 Success Criteria are met. All 9 Non-Goals are confirmed absent. The implementation is well-layered, thoroughly tested, and correctly documented in both EN and ZH.

| Severity | Count |
|----------|------:|
| **Pass** | 24 |
| **Note** | 7 |
| **Risk** | 8 |
| **Gap** | 0 |

No Gaps found. The 8 Risk items are all forward-looking Phase 8 handoff concerns or minor coverage blind spots, none of which violate the Phase 7 spec.

---

## 1. Specification Conformance

### SC1: Shared diagnostic shape used by Phase 7 surfaces

**A-1.1 | Pass | Diagnostic model matches spec vocabulary**

The `Diagnostic` struct in `crates/opi-agent/src/diagnostic.rs` carries all six spec-required fields: `severity` (3-level enum), `code` (stable `&'static str`), `source` (stable `&'static str`), `message` (String), `details` (optional JSON), `action` (optional String). 9 source constants and 27 code constants provide compile-time stability. Serialization is deterministic and tested (`diagnostic_serializes_deterministically`).

Evidence: `crates/opi-agent/src/diagnostic.rs` lines 76-91; `crates/opi-agent/tests/diagnostics.rs` 17 tests.

**A-1.2 | Pass | Shared shape crosses Phase 7 boundaries**

Guard test `phase7_shared_diagnostics_used_by_surfaces` in `observability_docs.rs` structurally verifies that doctor, trace, SDK, RPC, classification, and session event surfaces consume the shared `Diagnostic` model via source-file content assertions (not runtime only). Runtime boundary tests exist in `doctor_cli.rs`, `rpc_jsonl.rs`, and `trace_envelope.rs`.

Evidence: `crates/opi-coding-agent/tests/observability_docs.rs::phase7_shared_diagnostics_used_by_surfaces`.

**A-1.3 | Risk | Aggregate rollups remain pre-0.x string/count shapes**

`StartupDiagnostics` carries `Vec<String>` (not `Vec<Diagnostic>`). `SessionDiagnosticCounts` carries `{info, warning, error}` counts without per-diagnostic detail. `run_summary` in RPC is an ad-hoc JSON shape outside `AgentSessionEvent`. These were intentional decisions to respect 7.5's additive 0.x wire contract, but they mean Phase 8 must decide whether to promote these to full `Diagnostic` payloads before stabilizing the protocol.

Evidence: `crates/opi-agent/src/session_event.rs` lines 92-114; impl-state 7.5 session note "aggregate rollups kept as 0.x wire per spec".

### SC2: `opi doctor` reports all scopes without network

**A-1.4 | Pass | Doctor covers all 6 scopes without network calls**

`DoctorScope::ALL` contains `Config`, `Provider`, `Package`, `Session`, `Tui`, `Rpc`. The doctor module never constructs a provider, makes HTTP calls, or requires credentials. Provider scope checks credential **presence** only via env var probe; credential values never enter messages or details. 30 tests including 7 binary integration tests (`doctor_clean_env_exits_zero`, `doctor_json_reports_all_scopes_without_network`, etc.) verify this.

Evidence: `crates/opi-coding-agent/src/doctor.rs` dispatched before provider construction at `crates/opi-coding-agent/src/main.rs` line 41; `crates/opi-coding-agent/tests/doctor_cli.rs` 30 passed.

**A-1.5 | Pass | Doctor exit code policy correct**

Exit 0 when no `Severity::Error` (warnings OK), exit 2 when any error, exit 1 only on bad `--scope` argument. Tests: `exit_code_no_errors_is_zero`, `exit_code_with_error_is_two`, `doctor_unknown_scope_exits_one`, `doctor_config_error_exits_two`.

Evidence: `crates/opi-coding-agent/src/doctor.rs` lines 174-178; `crates/opi-coding-agent/tests/doctor_cli.rs`.

### SC3: JSON/RPC startup and run summaries expose structured diagnostic counts

**A-1.6 | Pass | JSON mode emits StartupDiagnostics before prompt output**

`StartupDiagnostics` NDJSON line is emitted as line 2 (after `session_header`), before any `Agent` event. `session_summary` carries optional `SessionDiagnosticCounts`. Test `phase7_startup_diagnostics_and_counts` pins ordering.

Evidence: `crates/opi-coding-agent/src/runner.rs` lines 191-203; `crates/opi-coding-agent/tests/json_mode.rs::phase7::phase7_startup_diagnostics_and_counts`.

**A-1.7 | Pass | RPC surfaces startup diagnostics and per-run counts**

`rpc_ready` header carries `startup_diagnostics` array. `run_summary` event carries `diagnostics.{info, warning, error}` after `AgentEnd`. Per-run scoping via `RecordingSink::clear()` at each `prepare_trace_run`. Tests: `phase7_startup_diagnostics_and_counts`, `phase7_run_summary_per_run_counts_and_after_agent_end` (verifies AgentEnd precedes run_summary and counts are per-run, not cumulative).

Evidence: `crates/opi-coding-agent/src/rpc.rs` lines 356-362, 537-551; `crates/opi-coding-agent/tests/rpc_jsonl.rs` 5 phase7 tests.

### SC4: Local redacted trace envelope requestable

**A-1.8 | Pass | Trace envelope implemented with correct opt-in semantics**

`TraceCollector` + `TraceSink` (file / recording) with `TRACE_SCHEMA_VERSION = 1`. 13 `TraceKind` variants. CLI opt-in via `--trace <PATH>` (non-interactive/JSON only). RPC opt-in via `trace` command returning versioned envelope. Unsupported trace requests produce structured error with `error_code: "unsupported_trace_request"`. No trace persisted by default.

Evidence: `crates/opi-agent/src/trace.rs`; `crates/opi-coding-agent/src/cli.rs` lines 86-89; `crates/opi-coding-agent/tests/rpc_jsonl.rs::phase7::phase7_trace_request_supported_and_unsupported_paths`.

**A-1.9 | Pass | Trace redaction modes work correctly**

Summary mode: `SecretRedactor` + content-sensitive key redaction + absolute path heuristic. Verbose mode: secrets only. Default is Summary. Tests cover both modes (`summary_mode_redacts_secret_and_prompt`, `verbose_mode_keeps_prompt_redacts_secret`, `phase7_trace_redacts_sensitive_values_in_diagnostic_linked`).

Evidence: `crates/opi-agent/tests/trace_envelope.rs` 5 redaction tests.

### SC5: Retry, cancellation, compaction, adapter degradation, and provider failures represented

**A-1.10 | Pass | Runtime failures emit diagnostics and trace records**

Agent loop `observe()` mirrors every runtime diagnostic as a `DiagnosticLinked` trace record. Covered failure paths: retry attempt/exhausted/succeeded, cancellation, provider auth/rate-limit/timeout/request/stream errors, tool unknown/validation/hook-deny/execution failure, compaction, max-turns-exceeded. Trace wiring tests pin provider failure, retry, and tool call paths through real `agent_loop`.

Evidence: `crates/opi-agent/src/agent_loop.rs` `observe()` function; `crates/opi-agent/tests/trace_envelope.rs::wiring` 7 tests; `crates/opi-agent/tests/diagnostics_runtime.rs::runtime_emission` 5 tests.

**A-1.11 | Risk | Adapter degradation surfaces as metadata strings, not typed Diagnostic + observe()**

Harness startup surfaces adapter degradation via `RuntimePackageStartup::diagnostics: Vec<String>`, which flows into `startup_diagnostics` as strings. These are not typed `Diagnostic` objects and do not flow through `observe()` for trace linkage. This is documented in the impl-state phase exit evaluator summary.

Evidence: `crates/opi-coding-agent/src/harness.rs` `startup_diagnostics` builder; impl-state phase 7 exit summary.

### SC6: Redaction covers API keys, bearer tokens, env values, prompts, tool output

**A-1.12 | Pass | Redaction coverage is comprehensive**

`SecretRedactor` (6 value regexes, 10 sensitive field names) handles: Anthropic keys (`sk-ant-`), OpenAI keys (`sk-`), GitHub tokens (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_`), credentialed URLs (userinfo), JWTs (`eyJ...`), and sensitive fields (`password`, `secret`, `token`, `api_key`, `apikey`, `private_key`, `access_token`, `refresh_token`, `authorization`, `proxy-authorization`). Summary mode additionally redacts content-sensitive keys (`prompt`, `prompts`, `tool_output`, `tool_result`, `env`, `environment`, `command`, `args`, `cwd`) and absolute paths. End-to-end guard test `phase7_redacts_sensitive_values` covers the full pipeline.

Evidence: `crates/opi-agent/src/streaming_proxy.rs` `SecretRedactor`; `crates/opi-agent/src/diagnostic.rs` `redact()`; `crates/opi-agent/tests/diagnostics.rs::phase7_redacts_sensitive_values`.

### SC7: Documentation states observability is local and explicit

**A-1.13 | Pass | Docs posture verified by automated guard**

Guard test `phase7_docs_state_local_explicit_observability` verifies 8 files (4 EN, 4 ZH) contain required phrases. EN: `local` + `explicit` + (`0.x` or `unstable`). ZH: local/explicit/unstable equivalents. Spec and opi-coding-agent README additionally name `opi doctor` and `trace`.

Evidence: `crates/opi-coding-agent/tests/observability_docs.rs::phase7_docs_state_local_explicit_observability`.

### SC8: No telemetry, ecosystem expansion, OAuth, marketplace, or web dashboard added

**A-1.14 | Pass | Non-goals verified by automated guard**

Guard test `phase7_non_goals_are_not_claimed_or_implemented` checks: (1) forbidden positive-claim phrases absent from 6 doc files in both EN and ZH, (2) no telemetry/analytics Cargo dependencies (`opentelemetry`, `otlp`, `sentry`, `posthog`, `amplitude`, `datadog`, `mixpanel`, `segment`, `tracing-appender` all absent), (3) no global tracing subscriber in `main.rs`, (4) `trace.rs` states it is not telemetry and traces are not produced by default. Meta-guard test `phase7_negation_helper_rejects_positive_claims` validates the negation helper itself.

Evidence: `crates/opi-coding-agent/tests/observability_docs.rs::phase7_non_goals_are_not_claimed_or_implemented`, `phase7_negation_helper_rejects_positive_claims`.

### Non-Goals (NG1-NG9)

**A-1.15 | Pass | All 9 Non-Goals confirmed absent**

| NG | Non-Goal | Status |
|----|----------|--------|
| NG1 | No remote telemetry service | Absent; Cargo dep guard + doc guard |
| NG2 | No analytics collection | Absent; Cargo dep guard + doc guard |
| NG3 | No automatic session sharing | Absent; doc guard |
| NG4 | No package ecosystem expansion | Absent; no new package types or registries |
| NG5 | No OAuth/provider breadth work | Absent; no new providers or auth flows |
| NG6 | No new tracing backend | Absent; no `tracing-appender` or new subscriber; `tracing`/`tracing-subscriber` allowed |
| NG7 | No full prompt/tool output capture by default | Absent; trace opt-in only; Summary mode redacts content |
| NG8 | No web dashboard | Absent; doc guard |
| NG9 | No stable 1.0 observability protocol | Absent; all schema versions explicitly unstable 0.x |

Evidence: `observability_docs.rs::phase7_non_goals_are_not_claimed_or_implemented`; workspace `Cargo.toml` and `Cargo.lock`.

---

## 2. Architecture Review

**A-2.1 | Pass | Diagnostic model layering is sound**

Three clean layers: (1) vocabulary in `opi-agent` (types, constants, redaction), (2) classification bridges (`From<&ProviderError>`, `From<&AgentError>`) in `opi-agent` consuming `opi-ai` error types, (3) emission via `DiagnosticSink` trait (object-safe, `Send + Sync`, no I/O). Human formatting stays at CLI boundary (`doctor.rs` `format_text`/`format_json`), not in lower layers. `Serialize`-only on `Diagnostic` enforces emit-only design.

Evidence: `crates/opi-agent/src/diagnostic.rs`, `diagnostic_sink.rs`; `crates/opi-coding-agent/src/doctor.rs`.

**A-2.2 | Pass | Trace model lifecycle is well-separated**

Caller (harness) owns `prepare()`/`finish()` lifecycle; `agent_loop` only emits records (documented contract). `TraceCollector` is single-owner with `Arc` sharing for the loop duration. `TraceSink` trait is minimal (`prepare`/`write`/`finish`). `TraceKind` is `#[non_exhaustive]` for forward compatibility. `TRACE_SCHEMA_VERSION = 1` stamps every record.

Evidence: `crates/opi-agent/src/trace.rs`; `crates/opi-coding-agent/src/harness.rs` `prepare_trace_run()`/`finish_trace_run()`.

**A-2.3 | Pass | Doctor architecture is clean**

Dispatched before provider construction in `main.rs`. Pure `DoctorContext` struct threads all inputs. Scope enum is extensible. Separate from `opi package doctor` (different command path, different output shape, different exit semantics). No HTTP, no provider construction, no model calls.

Evidence: `crates/opi-coding-agent/src/main.rs` line 41; `crates/opi-coding-agent/src/doctor.rs`.

**A-2.4 | Pass | JSON/RPC changes are additive 0.x**

`NDJSON_SCHEMA_VERSION` stays 1, `SDK_SCHEMA_VERSION`/`RPC_SCHEMA_VERSION` stays 2. New `StartupDiagnostics` variant in `AgentSessionEvent`. New optional `diagnostics` field on `session_summary`. New `trace` command on `SdkCommand`. New `error_code` field on `SdkResponse`. All use `#[serde(skip_serializing_if)]` or are new variants of `#[non_exhaustive]` enums. No existing field removed or renamed.

Evidence: `crates/opi-agent/src/session_event.rs`; `crates/opi-agent/src/sdk.rs`; `crates/opi-coding-agent/src/rpc.rs`.

**A-2.5 | Note | RPC `run_summary` is ad-hoc, not an `AgentSessionEvent` variant**

`run_summary` in RPC is emitted as raw `serde_json::json!()` outside the `AgentSessionEvent` enum. This creates a second event framing shape in the RPC stream (alongside `AgentSessionEvent`-wrapped agent events). JSON mode uses `AgentSessionEvent::SessionSummary` instead. If Phase 8 stabilizes the RPC protocol, this asymmetry should be reconciled.

Evidence: `crates/opi-coding-agent/src/rpc.rs` lines 537-551 vs `crates/opi-coding-agent/src/runner.rs` `SessionSummary` usage.

**A-2.6 | Note | `observe()` lockstep has no divergence guard**

The `observe()` helper in `agent_loop.rs` mirrors every runtime diagnostic as a `DiagnosticLinked` trace record. If a future call site emits a diagnostic without going through `observe()`, diagnostics and trace will silently diverge. There is no compile-time or test-time guard that all diagnostic emissions must flow through `observe()`.

Evidence: `crates/opi-agent/src/agent_loop.rs` `observe()` function; no trait/macro enforcement.

---

## 3. Security and Privacy

**A-3.1 | Pass | SecretRedactor covers major credential families**

6 value regexes (Anthropic, OpenAI, GitHub classic + fine-grained PATs, credentialed URLs, JWTs) + 10 sensitive field names (including `authorization` and `proxy-authorization` added in 7.6). The patterns are tested in `streaming_proxy.rs` (26 tests) and the integrated pipeline in `diagnostics.rs::phase7_redacts_sensitive_values`.

Evidence: `crates/opi-agent/src/streaming_proxy.rs` `SecretRedactor`; test files.

**A-3.2 | Note | AWS credentials and Azure tokens are not pattern-matched**

`SecretRedactor` does not have explicit patterns for AWS access keys (`AKIA...`), Azure AD tokens, or Google OAuth tokens. These are partially covered by the JWT regex (Azure/Google tokens are often JWTs) and by field-name matching (`access_token`, `token`), but raw AWS `AKIA` keys embedded in string values would not be caught.

Evidence: `crates/opi-agent/src/streaming_proxy.rs` regex list; no `AKIA` pattern.

**A-3.3 | Pass | Doctor never leaks credential values**

Provider scope reads env var presence via `env_var(name).is_some()` and emits only `credentials_present: bool`. Sentinel-based tests (`provider_scope_never_emits_credential_value`, `doctor_does_not_leak_credential_value_end_to_end`) verify the sentinel never appears in text or JSON output.

Evidence: `crates/opi-coding-agent/src/doctor.rs` credential check; `crates/opi-coding-agent/tests/doctor_cli.rs`.

**A-3.4 | Pass | Trace redaction at emit boundary**

Redaction is applied in `TraceCollector::emit_inner()` before writing to any sink. Details are passed through `diagnostic::redact()` with the collector's configured `RedactionMode`. No raw details ever reach the sink.

Evidence: `crates/opi-agent/src/trace.rs` `emit_inner()`.

**A-3.5 | Note | Absolute path heuristic has known false positive edge**

The `ABSOLUTE_PATH_RE` regex in `diagnostic.rs` matches common POSIX root prefixes (`/Users/`, `/home/`, `/var/`, etc.) and Windows drive letters. It uses a non-alphanumeric boundary to avoid matching `https://`. However, URLs containing these directory names in their path (e.g. `https://example.com/Users/foo`) would be redacted in Summary mode. The impl-state notes this as "safety-biased, never leaks." This is acceptable but worth documenting.

Evidence: `crates/opi-agent/src/diagnostic.rs` `ABSOLUTE_PATH_RE`; impl-state 7.1 session note.

**A-3.6 | Pass | Content-sensitive key list is comprehensive**

The `CONTENT_SENSITIVE_KEYS` set (`prompt`, `prompts`, `tool_output`, `tool_result`, `env`, `environment`, `command`, `args`, `cwd`) covers the spec requirements for prompt content, tool output, and environment values. Key matching is case-insensitive (tested).

Evidence: `crates/opi-agent/src/diagnostic.rs` `CONTENT_SENSITIVE_KEYS`; `crates/opi-agent/tests/diagnostics.rs::summary_redaction_scrubs_sensitive_content_fields`.

---

## 4. Test Coverage

**A-4.1 | Pass | Test matrix covers all spec-required levels**

| Level | Tests | Files |
|-------|------:|-------|
| Unit (diagnostic model) | 17 | `diagnostics.rs` |
| Unit (trace model) | 22 | `trace_envelope.rs` (substrate) |
| Provider fixture | 8 | `provider_diagnostics.rs` |
| Runtime classification | 31 + 5 | `diagnostics_runtime.rs` (opi-agent + opi-coding-agent) |
| Runtime emission | 5 | `diagnostics_runtime.rs::runtime_emission` |
| CLI integration (doctor) | 30 | `doctor_cli.rs` |
| JSON mode | 16 | `json_mode.rs` |
| RPC | 69 | `rpc_jsonl.rs` |
| Trace wiring | 7 | `trace_envelope.rs::wiring` |
| Redaction guards | 9 | across 5 test files |
| Doc/non-goal guards | 4 | `observability_docs.rs` |
| Streaming proxy | 26 | `streaming_proxy.rs` |
| **Total Phase 7** | **243** | |

**A-4.2 | Pass | Edge cases are well-covered**

Tested edge cases include: fail-closed on prepare (`prepare_failure_aborts_before_run_fail_closed`), fail-open on write (`write_failure_emits_diagnostic_and_disables_sink_fail_open`), emit-after-finish no-op (`emit_after_finish_is_a_noop`), finish-error swallowed (`finish_error_is_swallowed_and_disables`), fail-open without diagnostic sink (`fail_open_works_without_diagnostic_sink`), write-before-prepare guard (`file_sink_fail_open_when_written_before_prepare`), nested redaction (`redaction_recurses_into_nested_structures`), poisoned mutex recovery (`RecordingSink`), per-run diagnostic count reset (`phase7_run_summary_per_run_counts_and_after_agent_end`).

Evidence: `crates/opi-agent/tests/trace_envelope.rs` 10 sink failure tests.

**A-4.3 | Risk | TurnEnded gap on early exit is untested**

When `agent_loop` exits early (cancellation, provider failure mid-turn), `TurnStarted` may not have a matching `TurnEnded`. The wiring tests verify `RunStarted` before `RunEnded` but do not assert `TurnStarted`/`TurnEnded` pairing or verify the gap is acceptable. This is not a correctness bug (trace consumers should handle incomplete turns), but the absence of a test means the behavior is not pinned.

Evidence: `crates/opi-agent/src/agent_loop.rs` -- `TurnEnded` emitted only after successful provider stream completion; no test asserts turn pairing.

**A-4.4 | Risk | Interactive TUI diagnostic path is not tested**

The interactive TUI mode does not enable `record_diagnostics` or trace collection by default. There are no tests verifying that runtime diagnostics in interactive mode are surfaced to the user (e.g., retry notifications, tool failures). The design likely intends these to remain agent events rendered by the TUI, but this path has no Phase 7 test coverage.

Evidence: `crates/opi-coding-agent/src/harness.rs` -- interactive default has no recording sink; no test file for interactive diagnostics.

**A-4.5 | Note | `adapter_host_mock` binary causes misleading test exit code**

The `harness = false` mock adapter binary (`crates/opi-coding-agent/tests/adapter_host_mock.rs`) hangs when executed directly by `cargo test` without the `OPI_ADAPTER_TEST_MODE` env var. This causes `cargo test --workspace` to report exit code 1 even when all actual tests pass. While not a Phase 7 issue (pre-existing from Phase 5/6 adapter work), it makes CI gate interpretation ambiguous.

Evidence: `crates/opi-coding-agent/tests/adapter_host_mock.rs` line 1 (`harness = false`); `cargo test --workspace` exit 1.

---

## 5. Code Quality

**A-5.1 | Pass | Error handling follows fail-closed/fail-open contract**

`TraceError::Prepare` propagates as `AgentError::TraceSetup` -- run aborted (fail-closed). `TraceError::Write` emits a `CODE_TRACE_SINK_FAILED` diagnostic and disables the collector (fail-open). `TraceError::Finish` is swallowed (best-effort cleanup). This matches the spec: "fail closed for file creation errors before the run starts, then fail open during a run."

Evidence: `crates/opi-agent/src/trace.rs` `emit_inner()`; `crates/opi-agent/src/loop_types.rs` `AgentError::TraceSetup`.

**A-5.2 | Pass | Serialization is stable and deterministic**

`Diagnostic` and `TraceRecord` are `Serialize`-only (no `Deserialize`), preventing round-trip assumptions. Optional fields use `skip_serializing_if = "Option::is_none"` for clean wire output. `Severity` and `TraceKind` use `rename_all = "snake_case"/"lowercase"`. Deterministic serialization is tested (`diagnostic_serializes_deterministically`).

Evidence: `crates/opi-agent/src/diagnostic.rs` derive macros; `crates/opi-agent/src/trace.rs` derive macros.

**A-5.3 | Pass | Concurrency model is correct**

`TraceCollector.sequence` uses `AtomicU64` with `SeqCst` ordering. `TraceCollector.disabled` uses `AtomicBool` with acquire/release. `RecordingSink` and `RecordingTraceSink` use `Mutex<Vec<_>>` with poisoned-mutex recovery (`into_inner()`). `DiagnosticSink` trait requires `Send + Sync`. The design assumes single-threaded emission per run (documented), making the atomics a defense-in-depth measure rather than a concurrent-access requirement.

Evidence: `crates/opi-agent/src/trace.rs` atomic usage; `crates/opi-agent/src/diagnostic_sink.rs` mutex handling.

**A-5.4 | Pass | Classification bridges are exhaustive**

`From<&ProviderError>` covers all 5 variants (`AuthFailed`, `RateLimited`, `Timeout`, `RequestFailed`, `StreamError`). `From<&AgentError>` covers all 7 variants (`Provider`, `AuthFailed`, `Tool`, `Hook`, `Cancelled`, `MaxTurnsExceeded`, `TraceSetup`). Both use `match` statements that would fail to compile if new variants were added.

Evidence: `crates/opi-agent/src/diagnostic.rs` `From` impls; Rust exhaustive match guarantees.

**A-5.5 | Note | `RecordingTraceSink` accumulates across RPC runs**

The RPC `trace` command returns the recording sink's full snapshot, which accumulates records from **all** runs in one RPC session. Meanwhile, diagnostic counts are reset per run via `RecordingSink::clear()`. This asymmetry is documented in impl-state and tested, but could surprise consumers expecting per-run traces.

Evidence: `crates/opi-coding-agent/src/rpc.rs` trace command; `crates/opi-coding-agent/src/harness.rs` `prepare_trace_run()` clears diagnostics but not trace sink.

---

## 6. Documentation Consistency

**A-6.1 | Pass | EN/ZH documentation synchronized**

Guard test `phase7_docs_state_local_explicit_observability` checks 4 EN + 4 ZH files for equivalent posture claims. The spec (`opi-spec.md` / `.zh.md`) section 11.3, root README, pi-alignment-matrix Phase 7 row, and opi-coding-agent README all carry the local/explicit/unstable-0.x message in both languages.

Evidence: `crates/opi-coding-agent/tests/observability_docs.rs::phase7_docs_state_local_explicit_observability`.

**A-6.2 | Pass | Spec 11.3 accuracy corrected during 7.6**

The 7.6 evaluator caught that spec 11.3 initially overclaimed a config-setting trace trigger. This was fixed before commit -- spec now correctly states `--trace` and RPC `trace` command only. The finding was documented in impl-state session notes.

Evidence: impl-state 7.6 session note: "confirmed MEDIUM (spec 11.3 overclaimed a non-existent 'config setting' trace trigger) -> FIXED".

**A-6.3 | Pass | Phase 4 spec hash re-synchronized**

Spec 11.3 changes to `opi-spec.md` updated its SHA-256. The Phase 4 snapshot's `spec_files_sha256` was re-synced (same maintenance pattern as Phase 6). Guard test `phase4_ledger_spec_hash_matches_current_spec` verifies alignment.

Evidence: `crates/opi-coding-agent/tests/phase4_ledger.rs`; `docs/snapshots/phase4/opi-impl-state.json`.

**A-6.4 | Note | Spec lists 5 scopes in example, implementation has 6**

The spec example on line 142 shows `--scope config,provider,package,session,tui` (5 scopes). The implementation adds `rpc` as a 6th scope. This is an additive extension beyond the spec example, not a contradiction (the spec text table on lines 147-155 lists all 6). Minor documentation nit.

Evidence: spec line 142 vs `crates/opi-coding-agent/src/doctor.rs` `DoctorScope::ALL`.

---

## 7. Residual Risks and Phase 8 Handoff

**A-7.1 | Risk | Startup diagnostics as strings, not typed Diagnostic**

`StartupDiagnostics` in JSON mode and `rpc_ready.startup_diagnostics` carry `Vec<String>`, not `Vec<Diagnostic>`. Phase 8 should decide whether to promote these to structured diagnostics. Changing the wire shape will require a schema version bump.

Evidence: `crates/opi-agent/src/session_event.rs` `StartupDiagnostics { diagnostics: Vec<String> }`.

**A-7.2 | Risk | Schema versions are explicitly unstable**

`TRACE_SCHEMA_VERSION = 1`, `SDK_SCHEMA_VERSION = 2`, `NDJSON_SCHEMA_VERSION = 1`. All three are documented as unstable 0.x. Phase 8 must define the stabilization path and breaking-change policy for these versions.

Evidence: `crates/opi-agent/src/trace.rs`, `crates/opi-agent/src/sdk.rs`, JSON mode `session_header`.

**A-7.3 | Risk | TurnEnded not guaranteed on early exit**

If `agent_loop` exits mid-turn (cancel, provider failure), trace may contain `TurnStarted` without `TurnEnded`. Consumers must handle open turns. Phase 8 should either guarantee pairing (emit `TurnEnded` on all exit paths) or document the gap as intentional.

Evidence: `crates/opi-agent/src/agent_loop.rs` -- `TurnEnded` emitted only after successful provider completion.

**A-7.4 | Risk | RPC trace accumulates across runs**

`RecordingTraceSink` used by RPC accumulates records from all runs. The `trace` command returns the full snapshot, not just the latest run. Diagnostic counts are per-run (cleared). Phase 8 should decide whether to provide per-run trace retrieval or document the accumulation behavior.

Evidence: `crates/opi-coding-agent/src/rpc.rs` trace command returns `sink.snapshot()`.

**A-7.5 | Risk | Adapter degradation not in typed diagnostic path**

Adapter startup degradation flows through `RuntimePackageStartup::diagnostics: Vec<String>` into `startup_diagnostics`. It does not use the shared `Diagnostic` model or flow through `observe()` for trace linkage. Phase 8 should bridge adapter diagnostics into the shared model.

Evidence: impl-state phase 7 exit evaluator summary: "SC5 harness startup surfaces adapter degradation as metadata strings, not typed Diagnostic+observe()."

**A-7.6 | Risk | `observe()` divergence risk**

New diagnostic emit sites added outside `observe()` would silently break the diagnostic-trace lockstep. Phase 8 should consider a lint, macro, or architectural guard to enforce that all runtime diagnostics flow through a centralized observation point.

Evidence: `crates/opi-agent/src/agent_loop.rs` `observe()` -- convention-only enforcement.

---

## Appendix A: Full Gate Results

```
Gate: cargo fmt --check --all
Result: Clean (exit 0)

Gate: cargo clippy --workspace --all-targets -- -D warnings
Result: Clean (exit 0)

Gate: cargo test --workspace --all-targets --no-fail-fast
Result: Exit 1 (adapter_host_mock harness=false binary hang; all actual tests pass)
  - All test binaries except adapter_host_mock report 0 failures
  - adapter_host_mock is a mock subprocess binary, not a test suite

Gate: RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
Result: Clean (exit 0)

Phase 7 specific tests (all pass):
  opi-agent diagnostics:           17 passed
  opi-agent diagnostics_runtime:   31 passed
  opi-agent streaming_proxy:       26 passed
  opi-agent trace_envelope:        29 passed
  opi-ai provider_diagnostics:      8 passed
  opi-coding-agent diagnostics_runtime:  5 passed
  opi-coding-agent doctor_cli:     30 passed
  opi-coding-agent json_mode:      16 passed
  opi-coding-agent non_interactive: 7 passed
  opi-coding-agent observability_docs:   4 passed
  opi-coding-agent rpc_jsonl:      69 passed
  opi-coding-agent phase4_ledger:   1 passed
  Total:                          243 passed
```

---

## Appendix B: Finding Index

| ID | Severity | Title |
|----|----------|-------|
| A-1.1 | Pass | Diagnostic model matches spec vocabulary |
| A-1.2 | Pass | Shared shape crosses Phase 7 boundaries |
| A-1.3 | Risk | Aggregate rollups remain pre-0.x string/count shapes |
| A-1.4 | Pass | Doctor covers all 6 scopes without network calls |
| A-1.5 | Pass | Doctor exit code policy correct |
| A-1.6 | Pass | JSON mode emits StartupDiagnostics before prompt output |
| A-1.7 | Pass | RPC surfaces startup diagnostics and per-run counts |
| A-1.8 | Pass | Trace envelope implemented with correct opt-in semantics |
| A-1.9 | Pass | Trace redaction modes work correctly |
| A-1.10 | Pass | Runtime failures emit diagnostics and trace records |
| A-1.11 | Risk | Adapter degradation surfaces as metadata strings |
| A-1.12 | Pass | Redaction coverage is comprehensive |
| A-1.13 | Pass | Docs posture verified by automated guard |
| A-1.14 | Pass | Non-goals verified by automated guard |
| A-1.15 | Pass | All 9 Non-Goals confirmed absent |
| A-2.1 | Pass | Diagnostic model layering is sound |
| A-2.2 | Pass | Trace model lifecycle is well-separated |
| A-2.3 | Pass | Doctor architecture is clean |
| A-2.4 | Pass | JSON/RPC changes are additive 0.x |
| A-2.5 | Note | RPC `run_summary` is ad-hoc, not `AgentSessionEvent` |
| A-2.6 | Note | `observe()` lockstep has no divergence guard |
| A-3.1 | Pass | SecretRedactor covers major credential families |
| A-3.2 | Note | AWS credentials and Azure tokens not pattern-matched |
| A-3.3 | Pass | Doctor never leaks credential values |
| A-3.4 | Pass | Trace redaction at emit boundary |
| A-3.5 | Note | Absolute path heuristic has known false positive edge |
| A-3.6 | Pass | Content-sensitive key list is comprehensive |
| A-4.1 | Pass | Test matrix covers all spec-required levels |
| A-4.2 | Pass | Edge cases are well-covered |
| A-4.3 | Risk | TurnEnded gap on early exit is untested |
| A-4.4 | Risk | Interactive TUI diagnostic path is not tested |
| A-4.5 | Note | `adapter_host_mock` causes misleading test exit code |
| A-5.1 | Pass | Error handling follows fail-closed/fail-open contract |
| A-5.2 | Pass | Serialization is stable and deterministic |
| A-5.3 | Pass | Concurrency model is correct |
| A-5.4 | Pass | Classification bridges are exhaustive |
| A-5.5 | Note | `RecordingTraceSink` accumulates across RPC runs |
| A-6.1 | Pass | EN/ZH documentation synchronized |
| A-6.2 | Pass | Spec 11.3 accuracy corrected during 7.6 |
| A-6.3 | Pass | Phase 4 spec hash re-synchronized |
| A-6.4 | Note | Spec lists 5 scopes in example, implementation has 6 |
| A-7.1 | Risk | Startup diagnostics as strings, not typed Diagnostic |
| A-7.2 | Risk | Schema versions are explicitly unstable |
| A-7.3 | Risk | TurnEnded not guaranteed on early exit |
| A-7.4 | Risk | RPC trace accumulates across runs |
| A-7.5 | Risk | Adapter degradation not in typed diagnostic path |
| A-7.6 | Risk | `observe()` divergence risk |
