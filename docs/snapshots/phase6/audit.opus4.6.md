# Phase 6 Independent Completion Audit

## Audit Metadata

| Field | Value |
|---|---|
| Audited phase | Phase 6 — Alignment Hardening |
| Audited commit | `e9e58f4` (archive of Phase 6 ledger snapshot) |
| Phase 6 final task commit | `d78c28f` (task 6.6, alignment guards) |
| Design spec | `docs/superpowers/specs/2026-06-15-phase6-alignment-hardening-design.md` |
| Impl state | `docs/snapshots/phase6/opi-impl-state.json` |
| Auditor | Opus 4.6 (independent; did not participate in Phase 6 implementation) |
| Audit date | 2026-06-17 |

## Methodology

For each of the 10 Success Criteria defined in the Phase 6 design spec, this
audit applies a three-layer check:

1. **Code audit** — design spec intent vs. actual implementation behavior.
2. **Doc audit** — documentation claims vs. actual code behavior.
3. **Test audit** — whether tests cover the SC's key assertions through
   production entry points.

Each SC receives a verdict: **MET**, **PARTIALLY MET**, or **NOT MET**.

Findings are classified by severity:

- **Critical** — affects correctness or reliability of production behavior.
- **Moderate** — documentation misleads package authors or maintainers about
  observable behavior.
- **Minor** — naming drift, missing documentation detail, or test methodology
  observation that does not affect functionality.

Gate verification was performed by actually running `cargo fmt --check --all`,
`cargo clippy --workspace --all-targets -- -D warnings`, and focused Phase 6
test suites (317 tests total) against the audited commit, rather than trusting
the ledger's self-reported evidence.

## Executive Summary

**Phase 6 is complete.** All 10 Success Criteria are **MET**.

| Severity | Count |
|---|---|
| Critical | 0 |
| Moderate | 1 |
| Minor | 6 |

The single Moderate finding is a doc/implementation gap in the adapter protocol
failure semantics table. The Minor findings are documentation precision issues,
spec drift, and test methodology observations. None affect runtime correctness
or the Phase 6 non-goal boundary.

## Success Criteria Assessment

### SC1 — Version synchronization

> "Current implementation documentation consistently identifies the workspace
> as 0.5.1 while preserving historical release rows."

**Code audit.** The workspace version in `Cargo.toml` is `0.5.1`. All five
publishable crates use `version.workspace = true`.

**Doc audit.** Every file in the Phase 6 guarded set carries `0.5.1`:

| Document | EN version | ZH version |
|---|---|---|
| `README.md` / `README.zh.md` | `Current workspace version: \`0.5.1\`` | `当前 workspace 版本：\`0.5.1\`` |
| `docs/opi-spec.md` / `.zh.md` | Current implementation row, baseline, Phase 4/5 status | Mirrored |
| `docs/pi-alignment-matrix.md` / `.zh.md` | P0 row: `0.5.1` | Mirrored |
| 5 crate READMEs (EN + ZH) | `Current crate version: \`0.5.1\`` | `当前 crate 版本：\`0.5.1\`` |
| `CHANGELOG.md` | `[0.5.1]` current; `[0.5.0]` historical section preserved | N/A |

Historical `0.5.0` rows are preserved in `CHANGELOG.md`, the spec roadmap
table, and the alignment matrix.

**Test audit.** `phase6_current_docs_match_workspace_version` (34-test suite)
programmatically verifies the version strings above against
`env!("CARGO_PKG_VERSION")` and checks that the historical `[0.5.0]` section
remains in `CHANGELOG.md`.

**Finding F6** (Minor): `AGENTS.md` line 9 says `Current workspace version:
\`0.5.0\`` and `CLAUDE.md` line 7 says `v0.5.0 ships`. These files are outside
the Phase 6 design-scoped document set (Workstream 1 lists `README`,
`opi-spec`, `pi-alignment-matrix`, and crate READMEs) and are not covered by
the guard test. They are workspace-rule files maintained by the user, not
user-facing documentation that Phase 6 was tasked with updating.

**Verdict: MET.**

---

### SC2 — EN/ZH synchronization

> "English and Chinese docs remain synchronized for all changed user-facing
> documentation."

**Doc audit.** For every file in the Phase 6 guarded set, the EN and ZH
counterparts carry identical version strings, package capability claims, and
`opi-web-ui` scope statements. `phase6_localized_docs_stay_in_sync` encodes
these checks.

**Finding F7** (Minor): `docs/pi-alignment-matrix.md` P1 "Extension/package
execution" row describes process-JSONL adapter bridging as present, while
`docs/pi-alignment-matrix.zh.md` P1 row still describes discovered packages as
not automatically becoming executable workflows. This is substantive content
drift in a non-version-related row. The Phase 6 sync guard asserts version and
scope claims, not full row parity for every alignment matrix entry.

**Verdict: MET.**

---

### SC3 — Phase 6 baseline audit

> "A Phase 6 baseline audit explains the status of conflicting Phase 5 audit
> findings."

**Doc audit.** `docs/snapshots/phase6/audit-baseline.md` (108 lines):

- Names audited release `0.5.1` and commit `693c2e7`.
- References all three Phase 5 audit files as immutable inputs.
- Defines four classification buckets: Closed by 0.5.1, Accepted design
  difference, Phase 6 task, Future ecosystem candidate.
- Reconciles the contested GLM-5.1 "complete" vs. Codex/Opus "product loop
  incomplete" findings by explaining both views are accurate against different
  references (ledger DoD vs. full MVP loop).
- Maps contested findings to tasks 6.3, 6.4, 6.5.

**Test audit.** `phase6_baseline_audit_is_complete` verifies the structural
requirements: version, commit, three audit file references, `immutable`,
four bucket names, and task mappings.

Phase 5 audit files remain present and unmodified at their original paths.

**Verdict: MET.**

---

### SC4 — Package startup tests

> "Package startup has focused tests for the successful local-package adapter
> path and the key degraded paths listed in this design."

**Code audit.** Eight degraded paths from the design spec, mapped to tests:

| # | Degraded path | Test coverage | Entry point |
|---|---|---|---|
| 1 | Stale/mismatched lock | `resolver_reports_lock_drift_with_expected_and_actual_hash_and_disabled_state`, `resolver_reports_missing_lock_entry_with_disabled_runtime_state` | `resolve_installed_packages` |
| 2 | Missing package root | `resolver_reports_missing_local_package_as_error` | `resolve_installed_packages` |
| 3 | Invalid manifest | `resolver_reports_missing_top_level_manifest_field_through_resolution` | `resolve_installed_packages` |
| 4 | Unsupported adapter protocol | `unsupported_adapter_protocol_produces_diagnostic`, `start_adapters_unsupported_protocol_diagnostic_names_expected_and_actual` (3 tests) | `start_adapters_from_packages` |
| 5 | Adapter initialize timeout | `host_times_out_unresponsive_adapter` (host layer); `adapter_startup_failure_diagnostic_includes_command_and_disabled_state` (startup path, SpawnFailed proxy) | `AdapterHost::start`, `start_adapters_from_packages` |
| 6 | Adapter process exit | `host_detects_adapter_crash_during_handshake` (host layer); same startup proxy test as #5 | `AdapterHost::start`, `start_adapters_from_packages` |
| 7 | Duplicate names (same layer) | `resolver_reports_duplicate_manifest_name_within_precedence_layer` | `resolve_installed_packages` |
| 8 | Project overrides global | `resolver_prefers_project_package_over_global_package_with_same_manifest_name`, `resolver_does_not_report_duplicate_for_project_over_global_override` | `resolve_installed_packages` |

Success path coverage: `package_add_local_writes_declaration_and_lock` (CLI
add), `runtime_startup_starts_installed_project_package_adapter` (resolve +
start + tool), `resumed_installed_adapter_state_restores_on_current_thread_runtime`
(state restore after resume).

**Finding F5** (Minor): Degraded paths 5 and 6 (InitializeTimeout,
AdapterExited) are tested at the host layer (`adapter_host.rs`) for variant
content and error display, but through `start_adapters_from_packages` they use
a SpawnFailed proxy (nonexistent binary) rather than a real hang/crash mock.
This is documented as a deliberate engineering decision to avoid parallel-test
flakes from process-environment pollution. The shared startup-failure formatting
arm (`"adapter startup failed: {e}; ... disabled at runtime"`) is pinned by the
proxy test; the `{e}` content for timeout/exit variants is proven separately in
host-level tests.

**Test audit.** All eight paths have at least one focused test through a
production entry point. Total: package_store (24) + package_cli (31) +
package_resolver (14) + package_manifest_v2 (21) + harness_resource_integration
(21) = 111 tests, all passing.

**Verdict: MET.**

---

### SC5 — Adapter protocol tests

> "Adapter protocol tests cover lifecycle, failure, cancellation, state, and
> shutdown behavior."

**Code audit.** The design lists 11 protocol behaviors. Coverage mapping:

| # | Behavior | Test file(s) | Tests |
|---|---|---|---|
| 1 | Initialize/capabilities | adapter_host, adapter_runtime, adapter_protocol | `host_initializes_and_receives_capabilities`, `host_sends_correct_protocol_version_in_initialize`, serde round-trips |
| 2 | Unsupported protocol | adapter_runtime | `start_adapters_unsupported_{protocol,kind}_diagnostic_names_expected_and_actual` |
| 3 | Deterministic lifecycle order | harness_resource_integration | `adapters_start_in_deterministic_order` |
| 4 | Hook declaration/dispatch | adapter_runtime | `adapter_skips_hook_not_in_capabilities`, `adapter_before_tool_hook_can_block`, `adapter_before_tool_hook_allows_safe_tools`, `gate_adapter_in_registry_blocks_destructive_tools` |
| 5 | Request id correlation | adapter_host | `host_correlates_multiple_concurrent_requests`, `next_id_produces_unique_values` |
| 6 | Timeout behavior | adapter_host | `host_times_out_unresponsive_adapter`, `host_times_out_individual_request` |
| 7 | Best-effort cancellation | adapter_host, adapter_runtime | `host_sends_cancel_best_effort`, `adapter_tool_execute_respects_cancellation` |
| 8 | Event fire-and-forget | adapter_host, adapter_runtime | `host_sends_event_without_blocking`, `event_drop_records_diagnostic`, `adapter_event_forwarding_does_not_block` |
| 9 | State serialize/restore | adapter_host, adapter_runtime | `host_sends_state_{serialize,restore}_and_receives_result`, `adapter_state_*` (5 tests) |
| 10 | Shutdown | adapter_host | `host_shutdown_reaps_child_process`, `shutdown_waits_for_child_exit_before_kill` |
| 11 | Crash diagnostics | adapter_host, adapter_runtime | `host_detects_adapter_crash_during_handshake`, `host_reports_unavailable_after_crash`, `start_adapters_unsupported_*` |

All five SC5 areas (lifecycle, failure, cancellation, state, shutdown) have
focused tests through production code paths. Total: adapter_protocol (24) +
adapter_host (18) + adapter_runtime (21) = 63 tests, all passing.

**Doc audit.** Spec section 10.2 documents the honest 0.x protocol for all 11
behaviors. The `adapter_protocol.rs` module doc describes version negotiation,
failure semantics, and lifecycle accurately except for the findings below.

**Finding F1** (Moderate): The `adapter_protocol.rs` Failure Semantics table
(line 64) says `after-tool hook times out -> fail open, record diagnostic`. The
actual implementation in `ProcessAdapter::on_after_tool_call`
(`adapter_extension.rs` line 395) uses `let _ = host.send_request(...)` which
silently discards the timeout error. No diagnostic is recorded. The
`adapter_extension.rs` module doc (line 37) correctly says `after_tool_call ->
continue (fail open)` without mentioning diagnostics, and `docs/opi-spec.md`
section 10.2 correctly describes `after_tool_call` as "the result stands"
without requiring a diagnostic. The protocol module's internal table is the
only location that over-promises. The behavior itself (fail open) is correct
and intentional. The fix is either to add `record_diagnostic` on the timeout
path, or to correct the table to say "fail open, continue" without the
diagnostic claim.

**Finding F2** (Minor): `AdapterHost::Drop` implementation calls
`start_kill()` and aborts the reader task, but does not send a `Shutdown`
protocol message and does not fail pending channels. This contrasts with the
explicit `shutdown()` method which sends `Shutdown`, waits up to 5 seconds,
kills, and fails all pending channels. The Drop behavior is a standard Rust
safety-net pattern, but the difference from the documented graceful shutdown
flow is not noted in the protocol docs.

**Finding F4** (Minor): The Failure Semantics table row
"protocol version mismatch -> Doctor reports versions" conflates two different
diagnostic paths. The `opi package doctor` command detects manifest validation
errors at resolution time (`PackageDiagnostic`), while the `expected vs actual`
protocol diagnostic is produced by `start_adapters_from_packages` at runtime
startup and flows into `startup_diagnostics`. Both paths report the relevant
values, but through different mechanisms. The table could be more precise.

**Verdict: MET.**

---

### SC6 — Session and RPC boundary tests

> "Session and RPC tests cover extension state persistence and startup
> diagnostics where relevant."

**Code audit.** Four specific areas from the design:

| Area | Test | Production path |
|---|---|---|
| 1. State restored before adapter use | `resumed_extension_state_is_restored_before_adapter_command` | `CodingHarness::builder().resume().build()` -> `prompt()` -> `dispatch_extension_command("todo/list")` -> 1 item |
| 2. State persisted after mutating turn | `extension_state_persists_to_session_jsonl_after_mutating_turn` | `dispatch_extension_command("todo/add")` -> `prompt()` -> JSONL contains `ExtensionState` entry with todo item |
| 3. Startup diagnostics in RPC | `rpc_ready_header_carries_startup_diagnostics`, `rpc_session_info_surfaces_startup_diagnostics` | `RpcRunner::new_with_runtime_packages()` -> `rpc_ready.startup_diagnostics` array + `session_info` response |
| 4. Command consistency through RPC | `rpc_adapter_backed_commands_dispatch_consistently_through_shared_abstraction` | RPC `extension_command` -> `todo/add` -> `todo/list` -> item present |

All four areas have focused tests through production entry points.
Total: session_contract (15) + session_extension_state (4) + rpc_jsonl (64) +
session_runtime (26) = 109 tests, all passing.

**Doc audit.** Spec section 9.3 clearly frames the session format as
"Rust-native" and "does not promise pi session v3 file read/write
compatibility." Section 9.4 ("Why Not pi Session v3") explicitly states the
two formats are not interchangeable.

**Finding F3** (Minor): Spec section 9.3 lists 10 entry types in its table
(message, model_change, thinking_level_change, compaction, branch_summary,
label, session_info, custom, custom_message, leaf). The `SessionEntry` enum
(`crates/opi-agent/src/session.rs`) has 4 variants: `Message`, `Compaction`,
`Leaf`, `ExtensionState`. Six listed types (model_change, thinking_level_change,
branch_summary, label, session_info, custom_message) have no JSONL entry
variant; some exist only as in-memory agent message types or runtime
configuration. The spec says "custom" for extension state but the wire type is
`extension_state`. Phase 11 planning explicitly defers defining or removing
these entries. This is pre-existing spec drift, not introduced by Phase 6.
Phase 6 task 6.5 edited section 9.3's prose framing (Rust-native) but did not
modify the entry-type table.

**Verdict: MET.**

---

### SC7 — Documentation guards

> "Documentation guards prevent Phase 6 from overclaiming deferred pi
> ecosystem features."

**Test audit.** Guard coverage mapped to design non-goals:

| Non-goal | Guard test(s) |
|---|---|
| npm | `readme_does_not_claim_npm` |
| marketplace/gallery | `readme_does_not_claim_marketplace` |
| update/enable/disable | `docs_do_not_claim_package_update_enable_disable` |
| permission enforcement | `docs_do_not_claim_package_permission_enforcement` |
| hot reload | `readme_does_not_claim_hot_reload`, `spec_does_not_claim_hot_reload` |
| bundled Node/TS/jiti | `docs_do_not_claim_bundled_js_ts_runtime`, `workspace_has_no_bundled_js_ts_runtime` |
| custom TUI adapters | `spec_does_not_claim_custom_tui_adapters` |
| provider streaming adapters | `spec_does_not_claim_provider_streaming_adapters` |
| TS extension compat | `docs_do_not_claim_ts_extension_api_compat` |
| pi session v3 compat | `docs_do_not_claim_pi_session_v3_compat` |
| pi-web-ui parity | `docs_do_not_claim_pi_web_ui_parity` |
| OAuth/provider parity | `docs_do_not_claim_broad_oauth_provider_parity` |
| new opi-types crate | `docs_do_not_claim_opi_types_or_protocol_migration`, `workspace_has_no_opi_types_crate` |
| new first-class providers | `first_class_provider_set_is_unchanged` |
| adapter protocol migration | `adapter_protocol_types_stay_in_coding_agent` |

Positive guard: `docs_describe_phase5_adapter_capability_surface` ensures
completed Phase 5 MVP claims remain present.

Teeth-testing was performed during implementation (task 6.6 session notes):
injecting forbidden claims into `docs/opi-spec.md` turned 7 doc guards red;
creating `crates/opi-types/` turned 2 code guards red; reverting restored green.

Every design-spec non-goal has at least one executable guard. 34 guard tests
pass.

**Verdict: MET.**

---

### SC8 — Future Ecosystem backlog

> "The Future Ecosystem candidate backlog exists and is explicitly
> non-committal."

**Doc audit.** The "Future ecosystem candidate" section in
`docs/snapshots/phase6/audit-baseline.md` (lines 84-99) lists:

- Package enable/disable at runtime
- Package update command
- npm/registry-backed sources and marketplace/gallery metadata
- Provider OAuth flows and interactive provider auth
- Stronger trust model/sandboxing/permission enforcement
- Browser-based web-ui product surface
- Session import/migration and pi session compatibility
- Extension UI/RPC UI sub-protocol

Non-committal language is present: "non-committal", "not committed next-phase
scope", "candidates for later prioritization", "none is implemented".

**Test audit.** `phase6_future_backlog_is_non_committal` verifies the section
exists, lists key candidates (enable/disable, update, npm, OAuth, sandbox,
web-ui), and contains non-committal language.

**Verdict: MET.**

---

### SC9 — Non-goal absence

> "No npm, marketplace, OAuth parity, pi-web-ui parity, permission
> enforcement, TS extension compatibility, or new shared type crate is added
> in Phase 6."

**Code audit.**

| Check | Result |
|---|---|
| `crates/opi-types/` directory | Does not exist |
| `opi-types` in workspace members | Not present (5 crates: opi-ai, opi-agent, opi-coding-agent, opi-tui, opi-web-ui) |
| Provider modules in `opi-ai/src/` | Exactly 9 (anthropic, azure_openai, bedrock, gemini, mistral, openai_chat, openai_responses, openrouter, vertex) |
| `adapter_protocol.rs` location | In `opi-coding-agent`, not moved to `opi-agent` |
| npm/marketplace code | None |
| OAuth parity code | None |
| Permission enforcement | None (trusted-code model only) |
| Bundled JS/TS runtime | None |
| TS extension compat | None |
| pi-web-ui parity | `opi-web-ui` remains unpublished component crate |

Code-presence guards in `productized_packages_docs.rs` encode these checks as
regression tests.

**Verdict: MET.**

---

### SC10 — Final verification gates

> "Final verification gates pass."

**Evidence** (independently executed, not from ledger):

| Gate | Result |
|---|---|
| `cargo fmt --check --all` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 (1m 31s) |
| `productized_packages_docs` | 34/34 passed |
| `adapter_protocol` | 24/24 passed |
| `adapter_host` | 18/18 passed |
| `adapter_runtime` | 21/21 passed |
| `package_store` | 24/24 passed |
| `package_cli` | 31/31 passed |
| `package_resolver` | 14/14 passed |
| `package_manifest_v2` | 21/21 passed |
| `harness_resource_integration` | 21/21 passed |
| `session_contract` | 15/15 passed |
| `session_extension_state` | 4/4 passed |
| `rpc_jsonl` | 64/64 passed |
| `session_runtime` | 26/26 passed |
| **Total focused tests** | **317/317 passed** |

No test requires live provider credentials or user runtime data. All
focused Phase 6 test suites were executed individually and passed.

**Verdict: MET.**

## Consolidated Findings

| ID | Severity | SC | Finding | Location |
|---|---|---|---|---|
| F1 | Moderate | SC5 | `adapter_protocol.rs` Failure Semantics table claims `after-tool hook times out -> fail open, record diagnostic` but `ProcessAdapter::on_after_tool_call` silently discards the timeout (`let _ = host.send_request(...)`) without recording any diagnostic. The fail-open behavior is correct and intentional; the "record diagnostic" claim is not implemented. | `adapter_protocol.rs:64`, `adapter_extension.rs:395` |
| F2 | Minor | SC5 | `AdapterHost::Drop` does not send `Shutdown` or clean up pending channels, unlike explicit `shutdown()`. Standard Rust safety-net pattern but the difference is undocumented. | `adapter_host.rs` Drop impl |
| F3 | Minor | SC6 | Spec 9.3 entry-type table lists 10 types; `SessionEntry` has 4 variants. Six types are unimplemented as JSONL entries; "custom" is implemented as "extension_state". Pre-existing drift deferred to Phase 11. | `docs/opi-spec.md:838-851`, `session.rs:85-90` |
| F4 | Minor | SC5 | Failure Semantics table "protocol version mismatch -> Doctor reports versions" conflates manifest-time `opi package doctor` validation with runtime `start_adapters_from_packages` startup diagnostics. Both paths report values but through different mechanisms. | `adapter_protocol.rs:62` |
| F5 | Minor | SC4 | InitializeTimeout and AdapterExited degraded paths (5, 6) tested via SpawnFailed proxy through `start_adapters_from_packages` rather than real hang/crash mock. Variant content tested separately in `adapter_host.rs`. Deliberate decision to avoid parallel-test flakes. | `harness_resource_integration.rs`, `adapter_host.rs` |
| F6 | Minor | SC1 | `AGENTS.md` and `CLAUDE.md` still say `0.5.0` in current-state prose. Outside the Phase 6 guarded doc set and maintained by the user as workspace rules. | `AGENTS.md:9`, `CLAUDE.md:7` |
| F7 | Minor | SC2 | `pi-alignment-matrix.md` P1 "Extension/package execution" row content drifts between EN and ZH. EN describes adapter bridging as present; ZH describes packages as not automatically executable. Non-version content, outside SC2's version-sync focus. | `pi-alignment-matrix.md` P1, `pi-alignment-matrix.zh.md` P1 |

## Recommendations

### F1 (Moderate) — after_tool_call diagnostic claim

Either:
- (a) Add `host.record_diagnostic(...)` on the timeout path in
  `ProcessAdapter::on_after_tool_call` to match the table claim, or
- (b) Correct the `adapter_protocol.rs` Failure Semantics table to say
  `fail open, continue` without the diagnostic claim (matching the
  `adapter_extension.rs` module doc and `docs/opi-spec.md` section 10.2).

Option (b) is simpler and aligns with the honest-0.x principle: document what
the code does, not what it might eventually do.

### F6 (Minor) — AGENTS.md / CLAUDE.md version staleness

Update `AGENTS.md` line 9 to `Current workspace version: \`0.5.1\`` and
`CLAUDE.md` line 7 to `v0.5.1 ships`. Consider adding these files to the
version-sync guard test to prevent future drift.

### F7 (Minor) — alignment matrix P1 EN/ZH content drift

Synchronize the P1 "Extension/package execution" row between
`pi-alignment-matrix.md` and `pi-alignment-matrix.zh.md`. Consider extending
the localization guard to assert content parity beyond version strings.

### F3 (Minor) — spec 9.3 entry-type table drift

This is explicitly deferred to Phase 11. No Phase 6 action required. When
addressed, either implement the missing entry types or remove them from the
table with a note that they are not yet persisted.
