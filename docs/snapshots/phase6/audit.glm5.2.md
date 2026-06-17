# Phase 6 Audit — GLM-5.2

- **Audited workspace version:** `0.5.1`
- **Audited range:** Phase 6 task commits `693c2e7` (6.1) → `d78c28f` (6.6), plus archival commit `e9e58f4` (HEAD)
- **HEAD at audit:** `e9e58f40fb87d5715fe7277a16fef376c67acc7e`
- **Audit date:** 2026-06-17
- **Auditor:** GLM-5.2 (independent; did not read sibling audits before forming conclusions — see §9)
- **Inputs:** `docs/snapshots/phase6/opi-impl-state.json` (the ledger), `docs/superpowers/specs/2026-06-15-phase6-alignment-hardening-design.md` (the design), the workspace source/tests/docs at HEAD, and git history.
- **Verdict:** **PASS-WITH-FINDINGS — conditional.** The ledger's self-certified `exit_criteria_met: true` / "all 10 Success Criteria met" is an **overclaim**. SC3 is not met (Critical); SC2 is not fully met (High); SC1 has a High gap. The substantive runtime hardening (SC4 core, SC5, SC8), non-goal compliance (SC9), and final mechanical gates (SC10) are genuinely sound.

## 1. Executive Summary

Phase 6 set out to make the project state auditable, align docs with implementation, and harden the package→runtime→adapter path — explicitly *without* expanding scope. On the substance, it largely succeeds: the eight package degraded paths and the eleven adapter-protocol behaviors are tested through real production paths with mostly real teeth; the thirteen non-goals are genuinely absent from code, deps, and CLI; and `fmt`/`clippy -D warnings`/`doc -D warnings`/`test --workspace --all-targets` all pass green at HEAD when I run them myself.

Where Phase 6 fails is in its **own self-assigned accuracy mandate**. The phase's centerpiece deliverable — `docs/snapshots/phase6/audit-baseline.md`, whose entire purpose (SC3) is to correctly classify the contested Phase 5 audit findings against the `0.5.1` codebase — materially misclassifies findings that are already closed as "open," and labels two implemented adapter hooks as a permanent "accepted design difference." These statements are false against the very commit the baseline names as its audit point. The gate test that "verifies" SC3 has no accuracy teeth and its comment actively pressures the document toward the wrong classification. Because SC3 is a named Success Criterion and the ledger records it as `met`, the phase's `exit_criteria_met: true` is not honest as written.

Finding tally: **1 Critical, 8 High, 10 Medium, 5 Low, 2 Nit**, against a backdrop of genuinely-met core work. (Two of the Highs and one Medium — H8 command-only state loss, and the production shutdown-teardown gap upgraded from a Low — were surfaced by cross-check against `audit.codex.md` after my independent pass and confirmed by direct read; see §9.) No runtime/user-facing behavior is security- or correctness-breaking, but two production-behavior gaps (state loss on command-only quit; adapters killed rather than gracefully shut down) plus the audit-record/doc-synchronization defects mean the phase cannot be truthfully signed off as "all SCs met." Re-sign-off is appropriate after the Critical + the three documentation Highs are fixed; the remaining Highs/Mediums are hardening follow-ups.

The single most important takeaway: **an independent audit of an audit-hardening phase found that the phase's own audit artifact is inaccurate.** That is the finding that most directly bears on whether Phase 6 accomplished what it set out to do.

## 2. Scope and Methodology

**Stance — independent and adversarial.** The ledger reports every task `passing`, every acceptance scenario `met`, and includes an "independent phase-exit evaluator" summary asserting all 10 Success Criteria met. I treated all of that as unverified claim, not fact, and re-derived from source. I did not let the ledger prime my conclusions; where I cite the ledger, it is as an auditable artifact in its own right (see §6.3).

**Truth sources — inferred DoD *and* spec Success Criteria, cross-checked.** Every Phase 6 DoD in the ledger carries `definition_source: "inferred"`, so each is potentially narrower than the Success Criterion it derives from. For each task I verified the literal inferred DoD *and* mapped the parent Success Criterion to concrete evidence, flagging any place a task satisfied its weaker DoD while leaving the stronger SC under-covered. (This cross-check is what surfaced the SC1/SC2 gaps, since the 6.1 DoD enumerates a narrower file/claim list than SC1/SC2 state.)

**Ledger as a first-class auditable artifact.** I verified the recorded `spec_files_sha256` against disk, the cited `production_call_sites` symbols against source, the "file NOT touched" boundary claims against `git show --stat`, and the `verified_at_commit` chain against git topology, and assessed the repeated "reinit reconciliation" pattern as a process smell (§6.3).

**Test-quality method — read-judge + red-reasoning, working tree untouched.** For every cited test I opened the test *and* the production file it exercises and judged whether the test has teeth: would it actually go red if the production behavior were broken or removed? I flagged substring-vacuous assertions, mocked/non-production paths, tautological match arms, and tests that structurally cannot fail. I did not mutate the working tree; for the highest-risk claims I reasoned through "delete this production line — does the cited test fail?" Two independent corroboration points: (a) the 6.1 agent performed reversible mutation and reported red/green transitions I treat as credible; (b) I caught and retracted my own false positive (§6.3, the CRLF spec-hash "drift"), which is a small existence proof that the method self-corrects.

**Verdict authority — can overturn, severity-graded.** I issue an independent roll-up verdict (§8) and severity-grade every finding (Critical/High/Medium/Low/Nit/Info). The verdict can and does contradict the ledger.

**Sibling-audit policy — independent-first.** I wrote all conclusions in §1–§8 before reading `audit.codex.md` or the Phase 5 trio; §9 (cross-reference) was appended afterward and records agreement/divergence.

**Mechanical execution.** I independently ran the four final gates at HEAD (§3) rather than trusting `verified_at_commit`, and dispatched nine parallel independent verification dimensions (six per-task + non-goal compliance + doc truth/EN-ZH + ledger integrity) whose structured findings I spot-confirmed by direct file reads before grading.

## 3. Mechanical Gates at HEAD (independent)

Run at `e9e58f4` on a clean tree. These re-derive the ledger's gate claims from source rather than trusting `verified_at_commit`.

| Gate | Command | Exit | Result |
|---|---|---|---|
| Format | `cargo fmt --check --all` | 0 | clean |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0 warnings |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 0 warnings |
| Test | `cargo test --workspace --all-targets` | 0 | all binaries, 0 failures |

**SC10 (final gates) is genuinely met.** This is the cleanest part of the phase.

## 4. Success Criteria Traceability Matrix

| SC | Design text | Status | Primary evidence / finding |
|---|---|---|---|
| 1 | Docs consistently identify workspace as 0.5.1 | **Partially met** | Core docs are 0.5.1; `AGENTS.md:9` + `CLAUDE.md:7` still say 0.5.0 (H1) |
| 2 | EN/ZH docs synchronized | **Not fully met** | `pi-alignment-matrix.zh.md` missing Phase 5 row + all adapter content (H2) |
| 3 | Phase 6 baseline reconciles Phase 5 findings | **Not met** | Baseline stamps closed findings "open" + false accepted-limitation (C1) |
| 4 | Package startup success + degraded-path tests | **Mostly met** | Success + 6/8 degraded paths strong; (e)/(f) weak (H4); doctor blind to dup (H5) |
| 5 | Adapter protocol lifecycle/failure/cancel/state/shutdown tests | **Met with gap** | 11 behaviors through production path; cancellation test weak (L1); **graceful shutdown unreachable in production (kill-on-drop)** (M10) |
| 6 | Session/RPC extension-state + startup diagnostics | **Partially met** | RPC diagnostics + restore/persist-after-turn genuine; **command-only RPC state lost on quit** (H8); "interactive" half vacuous (M3); entry-type overclaim (M4) |
| 7 | Docs guards prevent overclaiming | **Partially met** | Guards pass but several have real false-negative surface (H6, H7, M5–M8) |
| 8 | Future Ecosystem backlog non-committal | **Met** | Backlog present, non-committal, covers Workstream 7 |
| 9 | No forbidden non-goal added | **Met** | Independently verified all 13+ absent (§6.1) |
| 10 | Final gates pass | **Met** | §3 |

## 5. Per-Task Findings

### 5.1 Task 6.1 — Documentation truth and version synchronization (SC1, SC2)

The enumerated file set (root READMEs, `opi-spec` EN/ZH, `pi-alignment-matrix` P0 row, all five crate READMEs EN/ZH) is correctly at `0.5.1`, historical `0.5.0` rows are preserved, `opi-web-ui` is consistently scoped as an unpublished Rust component crate (not a browser app, not pi-web-ui parity), and the two guard tests have real teeth — `version` is sourced from `env!("CARGO_PKG_VERSION")`, so the guards catch drift in both directions.

The DoD is met. SC1/SC2 read more broadly are not. See **H1** (agent-context files) and **H2** (ZH matrix).

### 5.2 Task 6.2 — Phase 6 baseline audit + future backlog (SC3, SC8) — **FAIL**

The backlog half (SC8) is genuinely met: the Future Ecosystem section is present, explicitly non-committal ("not committed next-phase scope"), and covers every Workstream 7 item. Phase 5 immutability holds (the three audit files were added once at `ab38401`, never rewritten). The reconciliation's *historical* narrative — GLM-5.1 measured against ledger DoD, Codex/Opus against the full design MVP — is a fair reading.

The classification half (SC3) is the Critical defect. See **C1**, **H3**.

### 5.3 Task 6.3 — Adapter protocol contract hardening (SC5) — pass-with-findings

All eleven behaviors are tested through the production path (`AdapterHost`, `ProcessAdapter`, `start_adapters_from_packages`), and the two 6.3-specific deliverables are real and have teeth: the `start_adapters_from_packages` unsupported-protocol/kind diagnostics now carry **expected + actual** with distinct values (`adapter_extension.rs:781-798`, asserted in `adapter_runtime.rs`), and the `adapter_protocol.rs:38-52` "Version Negotiation" doc-comment was corrected from an inaccurate "wire exact-match handshake" to the honest 0.x truth (startup-time manifest-string gate; `Capabilities` carries no version field; no wire negotiation). Forbidden scope holds: no new adapter kind, protocol types still in `opi-coding-agent`.

One low-severity weak-teeth test (**L1** cancellation tautology), consistent with the documented best-effort contract and backed by a companion test. Separately, the graceful-shutdown path is unreachable in production (adapters are killed on drop) — see **M10**.

### 5.4 Task 6.4 — Package runtime path + degraded diagnostics (SC4, Error Handling) — pass-with-findings

The success path and six of eight degraded paths are genuinely exercised through production entry points with strong teeth — including the load-bearing duplicate-name precedence logic, which correctly distinguishes a same-layer collision from a legitimate project-over-global override, with **both** the positive (duplicate rejected) and negative (override not falsely flagged) cases asserted (`package_resolver.rs:82-131`, `package_resolver.rs` tests).

Three real gaps against the inferred DoD / Error Handling contract: **H4** (init-timeout/process-exit not through `start_adapters_from_packages`, variant content not pinned), **H5** (doctor/list blind to duplicate-name), and **M1/M2** (doctor diagnostic richness; process-env-pollution rationale).

### 5.5 Task 6.5 — Session and RPC adapter boundary (SC6) — pass-with-findings

Three of four sub-claims are genuinely met through the real `CodingHarness` resume + prompt + persist path against a real process adapter, with teeth: RPC `startup_diagnostics` is populated from `harness.resource_metadata().diagnostics` (`rpc.rs:299-310`) and asserted non-empty; the previously-false "Diagnostics go to stderr" module doc is gone (0 stderr writes in `rpc.rs`); restore-before-adapter and persist-after-mutating-turn assert concrete restored/persisted content.

Three gaps: **H8** (command-only RPC sessions lose adapter state on quit), **M3** (the "interactive command path" half of SC6 is vacuous — see independent confirmation in §6.2), and **M4** (spec 9.3/9.4 overclaim persisted concepts).

### 5.6 Task 6.6 — Alignment guards + final gates (SC7, SC9, SC10)

SC9 (code-presence) and SC10 (gates) are genuinely met. SC7 (doc guards) under-delivers: the guards pass against current clean docs, but several have real false-negative surface such that a future overclaim in different wording would slip through (**H6**, **H7**, **M5–M8**). Current docs/code are in fact clean, so no non-goal is actually violated — the weakness is in the guards as regression fences, not in the codebase.

## 6. Cross-Cutting Dimensions

### 6.1 Non-goal compliance (SC9) — **PASS, independently confirmed**

I verified independently of the 6.6 guards that none of the 13+ non-goals is implemented: no `npm`/`jiti`/`deno`/`swc`/`boa`/`v8`/`napi` dependency in any `Cargo.toml`; no `crates/opi-types`; `adapter_protocol`/`adapter_host`/`adapter_extension` remain `pub mod` in `opi-coding-agent` and absent from `opi-agent`; `opi-ai/src` has exactly the 9 documented providers and was not touched by any Phase 6 commit (`git log --since=2026-06-15 -- crates/opi-ai/src/` is empty); `PackageCommand` exposes only Add/Remove/List/Doctor (`cli.rs:127-156`); the only `npm:` hit is a parser that *rejects* npm sources (`package_store.rs:639`); the only "OAuth" mentions are pre-existing bearer-token env plumbing untouched in Phase 6. **SC9 is fully and honestly met.** (That the *guards* enforcing SC9 are weak is a separate, SC7 concern.)

### 6.2 Documentation truth + EN/ZH sync (SC1, SC2) — pass-with-findings

Core docs are synchronized to 0.5.1 with correct `opi-web-ui` scope and correct adapter/session framing on both the files and claims the 6.1 guard checks. Two gaps the guard does not cover: **H1** (agent-context files) and **H2** (ZH matrix). The `phase6_localized_docs_stay_in_sync` test only asserts version + `opi-web-ui`-scope strings, so it is green despite the matrix drift — it has no teeth for Phase 5 capability-content parity.

### 6.3 Ledger integrity — pass-with-findings

The load-bearing integrity claims hold under direct verification:

- **Spec hash — no drift.** I initially flagged a mismatch (raw `sha256sum docs/opi-spec.md` = `99da7137…` vs ledger `9a34e870…`), then retracted it: it is a Windows CRLF artifact. `tr -d '\r' < docs/opi-spec.md | sha256sum` = `9a34e870…`, matching the ledger, the phase4 snapshot, and `git show HEAD:`. The gating test `phase4_ledger.rs:13` normalizes `\r\n`→`\n` before hashing, so it computes `9a34e870` and passes. **Nit N2:** the ledger does not document that its hash is LF-normalized, so a naive `sha256sum` (mine included) false-positives; a one-line comment in `phase4_ledger.rs` would prevent future auditors repeating my detour.
- **Production call sites — all 10 exist** in the cited modules (`AdapterHost`, `ProcessAdapter`, `start_adapters_from_packages`, `handle_package_command`, `resolve_installed_packages`, `start_installed_package_runtime`, `RpcRunner`, `SessionCoordinator`, `SessionWriter`, `SessionReader`).
- **Boundary claims — hold.** `git show --stat 8bb0a0a -- adapter_host.rs` is empty (6.4 did not touch it); `git show --stat e5118fd -- harness.rs` is empty (6.5 did not edit it).
- **Commit chain — consistent.** Each `verified_at_commit` descends from its `start_commit`; all are ancestors of HEAD; `e9e58f4` is the single archival commit that added the snapshot (+1503 lines, only that file; the live root ledger is gitignored).

Two process smells: **M9** (three "reinit reconciliation" events where the live ledger forgot to refresh `spec_files_sha256` after editing `opi-spec.md`, leaving `verified_at_commit` momentarily dishonest and repaired only retroactively — no gate enforces the live-ledger hash at commit time), and **L5** (the "independent phase-exit evaluator" claim is unverifiable and its phrasing closely mirrors ledger task titles).

## 7. Graded Findings Register

### Critical

**C1 — SC3 deliverable materially misclassifies closed findings as open.** `docs/snapshots/phase6/audit-baseline.md` names `693c2e7` as its audit commit and asserts the contested product-loop findings "remain **open**, not closed" (line 50-51), classifying *"Installed package declarations are not connected to runtime startup"*, *"Adapter state does not survive restart"*, and *"Adapter startup diagnostics not surfaced in RPC"* as **open** Phase 6 tasks (lines 76-82); classifying *"prepare_next_turn and transform_context not implemented"* as a permanent **Accepted design difference** (line 67); and classifying *"relative adapter command may resolve outside the package root (`..` escape)"* as an **Accepted design difference** (line 68). All are false at `693c2e7`: `ab38401` ("close phase 5 package loop", 2026-06-15) is an **ancestor** of `693c2e7`, and at `693c2e7` I confirmed `runtime_packages.rs` exists, `main.rs` calls `start_installed_package_runtime` at lines 210/320/385 (all three run modes), `SessionEntry::ExtensionState` exists (`session.rs:89`), `ProcessAdapter` implements both `transform_context` (`adapter_extension.rs:399`) and `prepare_next_turn` (`:458`), `resolve_adapter_command_checked` is invoked at resolution (`package_resolver.rs:549`, rejecting `..` escape), and the RPC `rpc_ready` startup_diagnostics path exists. The product loop was wired the day *before* the baseline was authored (`1d386c5`, 2026-06-16 13:18). The baseline's *historical* reconciliation narrative is fine; its present-tense classification table, the permanent-hook-limitation row, and the escape-accepted row are wrong. (The escape and RPC-diagnostics stale rows were surfaced by cross-check with `audit.codex.md` and confirmed here.) *Fix:* re-audit at HEAD; move these findings to "Closed by 0.5.1 (via `ab38401`/Phase 6 tasks)"; reclassify the hooks row as "Closed by 6.3"; state plainly that the GLM-vs-Codex/Opus disagreement was real against pre-remediation 0.5.0 and is now resolved.

### High

**H1 — Agent-context docs still identify the workspace as 0.5.0 (SC1).** `AGENTS.md:9` = *"Current workspace version: `0.5.0`."* and `CLAUDE.md:7` = *"v0.5.0 ships a multi-provider coding assistant …"* — both present-tense, both loaded as live agent context (spec §8.4 prompt layer 4). Neither is in 6.1's `task_owned_paths` nor covered by any Phase 6 guard. (Already flagged by `audit.codex.md` P2; not closed in 6.1.) Notably this means the very instructions this auditor operated under carry the stale version. *Fix:* bump both to 0.5.1 (or mark historical explicitly) and add an `AGENTS.md`/`CLAUDE.md` current-version assertion to `phase6_current_docs_match_workspace_version`.

**H2 — ZH alignment matrix materially out of sync (SC2).** `docs/pi-alignment-matrix.zh.md` has **0** Phase 5 rows (`grep -c "^| 5 |"` = 0) and **0** `process-jsonl`/`opi-extension-jsonl-v1` mentions, while the EN matrix has a Phase 5 row plus 5+ adapter-content references; ZH rows 29/58/62 and the P1 execution row omit the package-CLI / manifest-V2 / process-jsonl / bridging content the EN rows carry, and ZH row 62 still shows a stale "next action" (补 install/list/config/update/remove). `phase6_localized_docs_stay_in_sync` passes green regardless (it checks only version + `opi-web-ui`-scope strings). *Fix:* backfill the ZH matrix Phase 5 row + rows 29/58/62/P1; extend the sync guard to assert Phase 5 capability-content parity.

**H3 — SC3 gate has no accuracy teeth and pressures the wrong classification.** `phase6_baseline_audit_is_complete` (`productized_packages_docs.rs:574-622`) asserts the baseline merely *contains* the strings `6.3`/`6.4`/`6.5`; its comment (613-615) states these *"map to OPEN Phase 6 tasks, never claimed closed."* It cannot detect the C1 misclassification, and a maintainer who correctly reclassified the closed findings could trip it. *Fix:* invert the intent — assert those findings appear under a closure/Phase-6-done disposition, or at minimum drop the "must be open" pressure; add an assertion that no Phase 5 finding whose owning code is verifiably closed is labeled "open."

**H4 — Init-timeout and process-exit degraded paths not pinned through startup (SC4 / Error Handling).** Paths (e) init-timeout and (f) process-exit-during-startup are not exercised through `start_adapters_from_packages`. The only coverage is `adapter_host.rs:109-119` (`InitializeTimeout { .. }` — wildcard, never asserts the timeout value) and `:125-138` (`AdapterExited { .. } | AdapterUnavailable { .. }` — OR-branch with wildcards, never asserts `exit_code`, doesn't even pin the variant). The Error Handling contract demands a "timeout surface" and exit-code content; neither is asserted to reach a diagnostic. The startup-failure diagnostic test uses a `SpawnFailed` path, not `InitializeTimeout`/`AdapterExited`. *Fix:* drive hang/early-exit modes through `start_adapters_from_packages` (per-child env via `AdapterProcessConfig.env` works — see M2 — or argv mode bits like `package_adapter_example`), and assert the diagnostic carries the timeout duration / exit surface.

**H5 — `package doctor`/`list` blind to same-layer duplicate-name (SC4 / Error Handling).** `cmd_list` (`package_cli.rs:250`) and `cmd_doctor` (`:296`) call `resolve_declared_installed_packages`, which is the inner helper *without* the duplicate-name precedence logic. That logic lives only in `resolve_installed_packages` (`package_resolver.rs:82-131`), which calls the helper then adds the `duplicate_name` branch. So a same-layer duplicate collision is invisible to `opi package doctor`/`list` until runtime startup — contradicting "degraded runtime state must be visible to doctor." *Fix:* switch `cmd_doctor`/`cmd_list` to `resolve_installed_packages`; add a test that declares two same-layer packages with the same manifest name and asserts `doctor` output contains a `duplicate_name` diagnostic.

**H6 — JS/TS-runtime guard scans the wrong file (SC7).** `workspace_has_no_bundled_js_ts_runtime` (`productized_packages_docs.rs:896-916`) reads only the root `Cargo.toml`, but this workspace declares runtime deps in each crate's `Cargo.toml` (all use `= { workspace = true }`). Adding `boa_engine = "0.18"` to `crates/opi-coding-agent/Cargo.toml [dependencies]` — the realistic path — would not trip it. *Fix:* iterate every `crates/*/Cargo.toml` (and optionally `Cargo.lock`) for the forbidden crate names.

**H7 — Positive Phase 5 capability guard is substring-vacuous (SC7).** `docs_describe_phase5_adapter_capability_surface` (`:998+`) checks the spec contains the bare words `tools`/`commands`/`hooks`/`events`/`state`/`cancellation`, which appear 122× in unrelated contexts. Stripping every Phase 5 adapter capability statement would leave it green. *Fix:* tie the guard to adapter-specific phrasings (`adapter tools`, `process-jsonl adapter`, `opi-extension-jsonl-v1`).

**H8 — Command-only RPC sessions lose adapter state on quit (SC6).** `CodingHarness::dispatch_extension_command` (`harness.rs:1194-1216`) takes `&self`, dispatches via `registry.dispatch_command`, and returns **without persisting**. Extension state is written only inside `persist_turn` (`harness.rs:1115`, after a prompt turn). RPC's `extension_command` arm calls it through `self.harness.as_ref()` (`rpc.rs:681`), so a headless client can issue `extension_command todo/add`, receive success, and quit with **no `ExtensionState` entry written** — the mutation is lost on restart. The existing persist test mutates via `todo/add` *and then runs a prompt turn*, which is what triggers persistence; it does not cover a command-only session. The 6.5 DoD says "persisted after turns that mutate adapter state" (a command-only session has no turn, so this is arguably just-outside-DoD), but stateful commands are useful precisely as commands, so the persistence guarantee is weaker than a user would expect. (Surfaced by `audit.codex.md` P1; confirmed by direct read.) *Fix:* persist extension state after a successful stateful `extension_command` dispatch when a session exists, or document command-only state as volatile and add a guard test for that contract.

### Medium

**M1 — Doctor/list diagnostic richness omits adapter command.** `list_diagnostic_json`/`doctor_rows` emit `adapter_command: null` for every diagnostic row, including adapter-related codes (`adapter_command_invalid`, unsupported-protocol), so `package list --json` for a bad adapter shows no command — contradicting "adapter command when relevant." (`package_cli.rs:535-547`.)

**M2 — "Process-env pollution" rationale does not hold.** The 6.4 ledger justifies skipping (e)/(f) through `start_adapters_from_packages` on "forbidden parallel-test flake," but `AdapterHost::start` forwards `config.env` per-child (`adapter_host.rs:163-167`) and the example adapter selects modes via argv (`package_adapter_example.rs:18-31`), so the modes can be driven without touching parent-process env. The decomposition was a test-design shortcut, not a hard constraint.

**M3 — SC6 "interactive command path" is vacuously satisfied.** `dispatch_extension_command` has exactly one production caller — `rpc.rs:681`. There is no interactive/CLI/non-interactive extension-command dispatch (`interactive.rs`: 0 matches). The SC6 qualifier "where the same runtime abstraction is used" arguably makes the interactive half vacuous, but the SC/DoD wording overstates coverage; a reader could infer interactive command dispatch exists. *Fix:* tighten SC6/6.5 wording to "consistency through the shared `CodingHarness` dispatch abstraction as used by RPC; interactive/CLI do not dispatch adapter commands in 0.x," or track interactive dispatch in the Future Ecosystem backlog.

**M4 — Spec 9.3/9.4 overclaim persisted session concepts.** The 9.3 entry-type table lists 10 types (`model_change`, `thinking_level_change`, `branch_summary`, `label`, `session_info`, `custom`, `custom_message`, …) but `SessionEntry` (`session.rs:85-90`) has only 4 variants (`Message`, `Compaction`, `Leaf`, `ExtensionState`). 9.4 (added by 6.5) claims opi "represents … model and thinking-level change markers" — those are session *events*, not persisted JSONL entries. The Rust-native honesty framing is undercut by an entry-type table that lists types opi doesn't persist. *Fix:* reconcile the table with the 4-variant enum (mark the others as events/future), EN+ZH; add a guard cross-checking the table against `SessionEntry`.

**M5 — `no_positive_claim` negation bypass (SC7).** The helper treats `without` and `not claim` as unconditional negation context (`productized_packages_docs.rs:104-110`), so "opi now bundles Node without external deps" passes every guard using it. *Fix:* require the negation cue to co-occur with the needle in the same clause, or drop `without` from the auto-allow list.

**M6 — npm guard `pi` substring bypass (SC7).** `readme_does_not_claim_npm` (`:40-57`) auto-passes any line containing `pi`, which matches inside `opi`; "opi package add supports npm sources" would pass. *Fix:* drop the bare `pi` allowance or require it as a whole-word negation phrase.

**M7 — Narrow update/enable/disable + bundled-runtime needles (SC7).** Needles miss obvious synonyms (`upgrade`, plurals, `enabled`; `V8 engine`, `runs TypeScript natively`, `embedded JS`). Chinese needles require the romanized "package" prefix. *Fix:* broaden needles (`upgrad`, `enabl`; `JavaScript engine`, `V8`, `runs TypeScript`; `升级`/`启用`/`禁用`).

**M8 — Marketplace guard scope too narrow (SC7).** `readme_does_not_claim_marketplace` covers only READMEs, never checks `gallery`/`画廊`/`商店`, and ignores `opi-spec`/`pi-alignment-matrix` — the docs most likely to grow capability claims (the EN matrix line 62 already says "add marketplace/registry if product requires it"). *Fix:* extend to spec + matrix, add gallery needles.

**M9 — Spec-hash refresh not enforced at commit time (ledger process smell).** The ledger self-admits three "reinit reconciliation" events where the live Phase E write forgot to refresh `spec_files_sha256[docs/opi-spec.md]` after editing `opi-spec.md` (e.g. line 456: 6.3's write "omitted the opi-implement hash refresh … and left the active ledger stale"), repaired only retroactively before the next task unblocked. No gate catches a stale *live-ledger* hash — `phase4_ledger` pins the phase4 *snapshot*, which was refreshed. So a stale live ledger passes CI silently. *Fix:* add a Phase E gate that the live ledger's `spec_files_sha256` matches the just-committed spec before allowing `verified_at_commit`.

**M10 — Graceful adapter shutdown unreachable in production (kill-on-drop) (SC5).** `AdapterHost::shutdown(mut self, …)` (`adapter_host.rs:407`) sends the protocol shutdown message and waits up to 5s before killing — but in production the host is stored as `Arc<AdapterHost>` inside `ProcessAdapter`/`ProcessAdapterTool` (`adapter_extension.rs:84,208`), so no owner can call the consuming `shutdown`. `grep` for `.shutdown(` across `crates/opi-coding-agent/src/` is **empty** (no production caller). On last `Arc` drop, `Drop for AdapterHost` (`adapter_host.rs:506-511`) calls `child.start_kill()` directly. So installed adapter packages are torn down by hard kill, not graceful shutdown; the documented/tested "graceful shutdown behavior" (SC5 behavior 10; the baseline's shutdown row) describes an API path production never reaches (`shutdown_waits_for_child_exit_before_kill` covers only the owned-host API). *Fix:* add an explicit async shutdown path for `ProcessAdapter`/`ExtensionRegistry`/`CodingHarness`, or narrow the documented contract to "owned `AdapterHost` shutdown is graceful; registry teardown is best-effort kill" and add a production-path test. (Surfaced by `audit.codex.md` P1; confirmed by direct read. `audit.codex.md` rates this P1; I rate Medium — a latent resource-cleanup risk plus a doc/contract gap, not data loss or corruption.)

### Low

**L1 — Cancellation test tautological (6.3).** `host_sends_cancel_best_effort` cancels a nonexistent id and only asserts `Ok`; `cancel()` returns `Ok(())` unconditionally (`adapter_host.rs:388-404`). `adapter_tool_execute_respects_cancellation` accepts every outcome. Consistent with the documented best-effort contract; companion tests carry the real behavior.

**L3 — Provider guard top-level only (6.6).** `first_class_provider_set_is_unchanged` does not recurse into subdirectories; a provider added under `bedrock/` would be missed. Top-level teeth are real.

**L4 — Guard teeth-tests not committed as reproducible artifacts (6.6).** The inject→red→revert→green and `mkdir crates/opi-types`→red runs live only in session notes. *Fix:* commit a `guards_teeth.rs` that programmatically mutates a temp doc and asserts each guard fails.

**L5 — "Independent phase-exit evaluator" independence unverifiable (ledger).** The summary contains code-level specifics (favorable) but its phrasing mirrors ledger task titles nearly verbatim; treat the independence label as unproven, not established.

**L6 — 6.1 inferred DoD narrower than SC1/SC2 (process smell).** The DoD's closed file list + version/scope claim set let 6.1 satisfy its weaker self while leaving SC1 (agent-context files) and SC2 (matrix content) under-covered. *Fix:* when a DoD is `inferred`, diff it against its parent SC and guard each uncovered clause.

### Nit

**N1 — CHANGELOG `[0.5.1]` link reference missing.** `## [0.5.1] - 2026-06-15` (`CHANGELOG.md:12`) has no matching `[0.5.1]:` reference at the bottom (only `[0.5.0]:`…`[0.1.0]:`), so the header renders as literal bracket text. Add `[0.5.1]: https://github.com/OdradekAI/opi/releases/tag/v0.5.1`.

**N2 — Phase 6 snapshot spec-hash field not gated.** Only the phase4 snapshot's hash is pinned by `phase4_ledger`; the phase6 snapshot's `spec_files_sha256` is purely documentary. Currently correct (`9a34e870`); defense-in-depth would assert the two snapshots agree.

## 8. Overall Verdict and Confidence

**Verdict: PASS-WITH-FINDINGS (conditional).** Confidence: high.

Phase 6's *engineering substance* is sound: the package→runtime→adapter hardening is tested through real production paths with mostly real teeth (SC4 core, SC5); the thirteen non-goals are genuinely absent (SC9); the final mechanical gates pass green when I run them myself (SC10); the Future Ecosystem backlog is honest (SC8); and the ledger's load-bearing integrity claims (hashes, call sites, boundaries, commit chain) hold under direct verification.

Phase 6's *accuracy mandate* — the thing that distinguishes a "hardening + alignment" phase from a feature phase — is where it falls short, and that is the headline:

- **SC3 is not met (Critical, C1).** The phase's own audit baseline materially misclassifies closed findings as open and enshrines a false permanent limitation for implemented hooks, against its own named commit. The SC3 gate (H3) cannot detect this and resists the correct fix.
- **SC2 is not fully met (High, H2).** The ZH alignment matrix is missing the Phase 5 row and all adapter content; the sync guard is toothless for it; the phase-exit summary's "EN/ZH docs synchronized" is unsupported for the full SC2 scope.
- **SC1 has a High gap (H1).** Live agent-context files (`AGENTS.md`, `CLAUDE.md`) still describe the current workspace as 0.5.0.

I therefore **do not confirm** the ledger's `phase_exit[6].exit_criteria_met: true` or its "all 10 Success Criteria met." The accurate statement is: SC5, SC8, SC9, SC10 are met; SC4 and SC6 are mostly met (with addressable test-quality/wording gaps); SC1 and SC7 are partially met; SC2 is not fully met; **SC3 is not met.**

This is primarily not a runtime regression — no non-goal was violated and no correctness or security defect was found. Two production-behavior gaps did surface (**H8** command-only RPC state loss on quit; **M10** adapters killed rather than gracefully shut down), but neither is a correctness or data-corruption bug and both are fixable. The preponderance of the failure is in the phase's self-assigned job to produce an accurate, trustworthy record: the audit baseline (**C1**), documentation synchronization (**H1**, **H2**), and the teeth of the regression guards (**H6**/**H7**, **M5–M8**). For an audit-hardening phase, that documentation/accuracy failure is the material one, and it is bounded — fixable largely without touching runtime behavior.

**Recommended re-sign-off bar (minimum):** fix **C1** (re-audit the baseline at HEAD), **H1** (agent-context version), **H2** (ZH matrix + sync-guard teeth), and **H8** (command-only RPC persistence contract), and correct the phase-exit summary to reflect SC2/SC3 as not-fully-met until then. **H3–H7**, **M10**, and the remaining Mediums are strongly recommended hardening follow-ups (they mostly strengthen guards and test teeth) but do not, on their own, block a truthful "done" once C1/H1/H2/H8 land and the SC2/SC3 status is stated honestly.

## 9. Cross-Reference vs Sibling Audits

*Written after §1–§8, per the independent-first policy. The phase5 trio (`audit.glm5.1.md` / `audit.codex.md` / `audit.opus4.6.md`, all dated 2026-06-09) and the Phase 6 sibling `docs/snapshots/phase6/audit.codex.md` were read only after my conclusions were fixed.*

### 9.1 Convergence with `audit.codex.md` (Phase 6)

The two independent Phase 6 audits — Codex and this one (GLM-5.2) — **converged on the same headline finding via different paths**: both independently identified that `audit-baseline.md` is stale and that `phase6_baseline_audit_is_complete` freezes the obsolete "open" state (my **C1**/**H3**; Codex P1). Both also independently flagged **H1** (`AGENTS.md`/`CLAUDE.md` still 0.5.0; Codex P2). Reaching the same central defect from two independent investigations — neither reading the other, neither trusting the ledger — is strong corroboration that the baseline misclassification is real and not an artifact of either auditor's framing.

### 9.2 Findings Codex surfaced that I under-covered (verified and folded into my register)

Cross-check is a corrective, not just a comparison. Three Codex findings were genuinely beyond (or below) my independent pass; I verified each by direct read and incorporated it:

- **H8 — command-only RPC state lost on quit.** Codex P1. I had verified persist-after-mutating-*turn* but missed that `dispatch_extension_command` itself never persists, so a command-only session (mutate, quit, no turn) loses state. Verified at `harness.rs:1194-1216` + `rpc.rs:681`. Added as **H8**.
- **M10 — graceful shutdown unreachable in production.** Codex P1. I had rated the shutdown-reap test a Low no-op; Codex's deeper read — `Arc<AdapterHost>` storage + `Drop::start_kill` + zero production `.shutdown()` callers — shows the graceful path is unreachable in production (kill-on-drop). Verified by grep + read. Upgraded to **M10** (I rate Medium where Codex rates P1 — see severity note in §7).
- **C1 expansion — adapter-command-escape now rejected.** Codex noted `resolve_adapter_command_checked` rejects `..` escape, so the baseline's "accepted difference" row for the escape is also stale. Verified at `package_resolver.rs:549`. Folded into **C1**.

### 9.3 Findings I surfaced that Codex did not reach

Conversely, my pass went deeper on documentation accuracy, guard mechanics, and ledger integrity:

- **H2 — ZH alignment matrix materially out of sync** (no Phase 5 row, zero adapter content). Codex marked SC2 "Met for guarded docs" because the guard only checks version + `opi-web-ui`-scope strings; the matrix *content* drift was invisible to that guard and to Codex.
- **H4 — init-timeout / process-exit variant content not pinned** (`{ .. }` wildcards; timeout value / exit code never asserted to reach a diagnostic).
- **H5 — `package doctor`/`list` blind to same-layer duplicate-name** (calls `resolve_declared_installed_packages`, not `resolve_installed_packages`).
- **H6 / H7 / M5–M8 — guard teeth** (JS/TS guard scans the wrong file; positive guard substring-vacuous; `without`/`pi` negation bypasses; narrow needles; marketplace scope). Codex's only SC7 caveat was AGENTS/CLAUDE coverage.
- **M4 — spec 9.3/9.4 entry-type table overclaims** persisted concepts vs the 4-variant `SessionEntry` enum.
- **M9 — the three "reinit reconciliation" events** and the live-ledger spec-hash invariant not being gated at commit time.
- **SC10 — I ran the full gate suite independently** (`fmt`/`clippy -D warnings`/`doc -D warnings`/`test --workspace --all-targets`, all exit 0); Codex explicitly marked SC10 "not fully rerun in this audit," so my SC10 confirmation is stronger.

### 9.4 Severity calibration divergences

| Item | GLM-5.2 | Codex | Note |
|---|---|---|---|
| Baseline misclassification (SC3) | Critical / Not-met | P1 / Partially-met | Same defect; I grade it as failing a named SC, Codex as a stale-but-present deliverable. |
| Graceful shutdown unreachable (M10) | Medium | P1 | I weight it as latent-cleanup + doc gap; Codex weights production-teardown impact higher. |
| SC4 | Mostly-met (H4/H5) | Met | I found doctor-blind + variant-content gaps Codex didn't flag. |
| SC5 | Met-with-gap (M10) | Partially-met | Same shutdown concern; I also credit the 10 other well-tested behaviors. |

These are honest calibration differences, not contradictions. The union of both audits is the most defensible picture.

### 9.5 Stance on the historical Phase 5 split

The phase5 trio genuinely disagreed (all dated 2026-06-09): **GLM-5.1** read Phase 5 as complete against the ledger DoDs; **Codex** and **Opus 4.6** read it as "substrate complete, product loop incomplete." My read of that split: **both were right against their respective references.** GLM-5.1 was correct that the ledger-recorded DoDs were met; Codex/Opus were correct that the end-to-end product path (`opi package add` → restart → a resolved, registered adapter) was not wired in the 2026-06-09 code, and that two of four design hooks were absent. `ab38401` ("close phase 5 package loop", 2026-06-15) then wired the loop, and Phase 6 tasks 6.3–6.5 hardened it. The accurate *current* statement is that the contested findings are **closed** — which is exactly what makes the Phase 6 baseline's present-tense "remain open" stamp (**C1**) wrong. So my C1 is not a disagreement with the historical reconciliation; it is a disagreement with stamping a now-resolved disagreement as still-open in the phase's own final record.

### 9.6 Meta-observation

The complementarity is itself a result: GLM-5.2 went deeper on documentation/accuracy/guard-teeth/ledger; Codex went deeper on production runtime behavior. Neither subsumes the other. For a phase whose explicit purpose is auditability and alignment, the fact that two independent model-audits produced overlapping-but-non-identical high-value findings — and converged cleanly on the single most important one — is the strongest available evidence that (a) the C1 baseline defect is real and (b) single-model sign-off undercounts real findings. This argues for retaining multi-model audit for future phases rather than treating any one auditor's "all met" as sufficient.

