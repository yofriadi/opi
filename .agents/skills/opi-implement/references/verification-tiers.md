# Verification Tiers Reference

Each task carries a `tier` field; the skill selects gates from this table.
All tiers also run the cross-cutting gates at the bottom.

## `workspace` Tier

Use for dependency graph changes, cross-crate integration harnesses, and tasks
whose primary crate is `workspace` or `cross-crate`.

Gates:
1. `cargo fmt --check --all`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-targets`
4. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
5. Smoke script runs

## `library` Tier

Use for focused `opi-ai`, `opi-agent`, or `opi-tui` library changes that do not
add provider wire formats, CLI runtime behavior, or visual snapshot surfaces.

Gates:
1. TDD red→green produced new/changed tests in `crates/<crate>/tests/` OR
   `#[cfg(test)]` modules. Verify via diff content inspection (not just stat).
2. `cargo test -p <crate>` green
3. `cargo clippy -p <crate> -- -D warnings` green
4. Docs with warnings denied green:
   - Unix shell: `RUSTDOCFLAGS="-D warnings" cargo doc -p <crate> --no-deps`
   - PowerShell: `$env:RUSTDOCFLAGS="-D warnings"; cargo doc -p <crate> --no-deps; Remove-Item Env:RUSTDOCFLAGS`
5. `cargo build --workspace` green (catches breaking-API changes)
6. No `unwrap`/`expect` in non-test code (grep check)

## `cli-tool` Tier

Use for built-in tools such as `read`, `write`, `edit`, `bash`, `glob`, `grep`,
`find`, and `ls`.

Gates: All `library` gates, plus:
1. Behavioral tests in `crates/opi-coding-agent/tests/` using `tempfile` crate
2. For `bash`: tests for timeout, cwd capture, cancellation
3. For mutating tools: test asserting Phase-1 safety boundary is reported
   before execution (per opi-spec §8.4)

## `cli-runtime` Tier

Use for CLI parsing, config, prompt/context loading, session commands, JSON
mode, tool selection flags, shell completions, and binary subprocess behavior.

Gates: All `library` gates, plus:
1. E2E test booting `MockProvider` + `opi` binary subprocess with scripted prompts
2. Assertions on stdout, stderr, and exit code

**MockProvider precondition:** REFUSE to run if no `MockProvider` symbol exists.
Grep `crates/opi-ai/src/test_support.rs` (or feature-gated path). If absent:
> "Task `<id>` depends on MockProvider scaffolding (task 1.17). Run 1.17 first."

## `tui` Tier

Use for ratatui rendering, keybindings, themes, fuzzy pickers, diff rendering,
terminal image rendering, and snapshot surfaces.

Gates: All `library` gates, plus:
1. Ratatui snapshot tests at fixed sizes (80×24 and 120×40) using `insta`
2. Snapshot diffs require explicit user approval — NEVER auto-accept

## Provider-Contract Addendum

Apply to enterprise providers and HTTP client work: Bedrock, Azure OpenAI,
Vertex, proxy support, and connection pooling.

Additional gates:
1. Fixture or `wiremock` tests cover success, streamed deltas, tool calls when
   applicable, usage, provider errors, and error mapping.
2. Credential precedence tests never require live cloud credentials.
3. Secret redaction tests assert API keys, OAuth tokens, proxy credentials, and
   cloud credentials do not appear in logs, errors, session files, or snapshots.
4. No live provider tests run unless they are `#[ignore]` and explicitly
   invoked outside this skill.
5. Shared HTTP client/proxy behavior is tested without real network calls.

## Multimodal Addendum

Apply to image input, image tool results, and terminal image rendering.

Additional gates:
1. Serialization tests cover image metadata, MIME type, size limits, and
   provider capability rejection.
2. Tool-result tests cover text-only fallback and non-UTF-8/binary-safe handling.
3. TUI tests use deterministic snapshots or golden terminal protocol output; no
   visual snapshot is accepted without explicit user approval.

## Product Acceptance Addendum

Apply to any task with non-empty `acceptance_scenarios`, and to any task whose
DoD claims runtime/startup/CLI/session/adapter/provider behavior.

Additional gates:

1. Run every command listed in each owned `acceptance_scenarios[].verification`
   item.
2. Inspect code paths and tests to prove each
   `acceptance_scenarios[].production_call_sites` entry is exercised by the
   verification. Direct helper, parser, protocol, mock bridge, or registry-only
   tests are substrate evidence unless they enter through the production
   call-site named in the scenario.
3. For CLI/runtime scenarios, include at least one subprocess, harness, RPC, or
   integration test that starts at the public command/API boundary. Unit tests
   may supplement but cannot replace this.
4. If a task cannot close an acceptance scenario yet, mark or keep the task as
   `substrate_only = true`, leave the scenario `open`, and ensure a later
   vertical-slice task owns closure.
5. Before Phase E, the planned commit evidence must include `Opi-Acceptance`
   for every closed scenario.

## Phase-Specific Addenda

Apply these in addition to the task's tier and the Product Acceptance Addendum.

### Phase 6 Alignment Hardening

Additional gates:
1. Documentation tasks that touch English user docs also update localized
   counterparts or explicitly cite why no localized counterpart exists.
2. Phase 6 baseline audit is additive under `docs/snapshots/phase6/`; do not
   rewrite Phase 5 historical audits.
3. Package runtime tasks include at least one local-package startup path and
   degraded-path coverage for invalid/missing/unsupported adapter states.
4. Docs guard tests cover both completed Phase 5 MVP claims and forbidden
   overclaims: npm, marketplace/gallery, update/enable/disable, permission
   enforcement, hot reload, TypeScript extension API compatibility, pi session
   v3 compatibility, pi-web-ui parity, and broad OAuth/provider parity.

### Phase 7 Reliability and Observability

Additional gates:
1. Diagnostic payload tests cover severity, stable code, source, message,
   redacted details, and optional action.
2. Redaction tests cover API keys, bearer tokens, environment values, prompt
   content, tool output, provider URLs, and absolute paths outside the relevant
   workspace.
3. `opi doctor`, `opi doctor --json`, scope selection, and exit-code policy are
   covered without paid provider calls or network requirements.
4. Trace envelope tests cover schema version, run/turn id, sequence,
   timestamp, source, kind, diagnostic linkage, and summary/verbose redaction.
5. Docs state observability is local and explicit; no telemetry, analytics,
   remote trace service, or web dashboard is added.

### Phase 8 Agent Runtime Stabilization

Additional gates:
1. Contract tests cover event order for no-tool, one-tool, parallel, sequential,
   mixed scheduling, validation failure, hook block, hook modification,
   cancellation, compaction, and steering/follow-up ordering.
2. Hook tests cover every `AgentHooks` method and failure semantics.
3. SDK/RPC contract tests cover busy-state rejection, abort, steer,
   follow-up, set_model, thinking level, compact, session_info, and
   extension_command behavior.
4. Public `opi-agent` API review classifies touched surfaces as supported 0.x,
   unstable internal, or candidate removal in docs.
5. No plan mode, sub-agent system, todo system, permission popup, MCP runtime,
   package ecosystem expansion, or web UI product enters core.

### Phase 9 Tooling Quality

Additional gates:
1. Built-in tool results expose consistent `content`, `details`, `is_error`,
   diagnostics, truncation, and path metadata.
2. Filesystem tool tests cover Windows paths, drive prefixes, Unicode,
   line endings, large files/output, binary/encoding errors, symlinks where
   supported, ignore handling, sorting, limits, and cancellation.
3. Mutating tool tests prove create/overwrite/edit conflict behavior with diff
   or audit summaries and no silent partial writes.
4. `bash` tests cover timeout, cancellation, cwd/env reporting, exit code,
   truncation, mutating classification, and sequential execution.
5. No permission popup, persistent background bash, remote execution, IDE
   index, language server, automatic formatting, sandbox, or workflow tool is
   added to core.

### Phase 10 Provider Correctness

Additional gates:
1. Every existing provider family has fixture coverage for request
   serialization, streaming lifecycle, usage, finish reasons, errors, and
   cancellation where supported.
2. Provider errors map into the documented taxonomy: auth, config, request,
   network, rate_limit, provider, stream, capability, and cancelled.
3. Tool calls, thinking/reasoning, image input, usage/cost, retry, proxy, and
   OpenAI-compatible profile flags are tested with fixtures or `wiremock`.
4. OpenAI-compatible breadth remains config-driven unless the reviewed source
   explains why a first-class adapter is required.
5. No live provider calls run by default; no OAuth, subscription auth, image
   generation, browser usage feature, provider streaming adapter protocol, or
   broad provider catalog expansion is added.

### Phase 11 Session Tree and Context Reconstruction

Additional gates:
1. New session entries round-trip and rebuild context deterministically, or are
   explicitly deferred with source citations.
2. Context-building tests cover active leaf resolution, corrupt trailing lines,
   model/thinking changes, compaction, branch summaries, custom messages, and
   extension state restoration when implemented.
3. Export tests cover local markdown or JSON output, active-branch/full-tree
   selection, tool/thinking inclusion controls, and redaction.
4. All session tests isolate data with temp directories or `OPI_SESSIONS_DIR`.
5. Docs state session files are sensitive and context is bounded to session
   files/explicit exports. No vector memory, global user profile, cloud sync,
   session sharing service, web UI product, or pi session v3 compatibility
   claim is added.

### Phase 12 TUI Product Polish

Additional gates:
1. Snapshot tests cover changed branch/session/model pickers, command palette,
   transcript rendering, diagnostics, tool calls, thinking, images, status bar,
   and narrow terminal layouts.
2. Snapshot updates are intentional and reviewed; never rebaseline unrelated
   snapshots.
3. Unit tests cover wrapping, truncation, CJK width, keybinding resolution,
   `NO_COLOR`, contrast/non-color indicators, and keyboard-only flow where
   touched.
4. Command discovery shows built-in and already-registered extension commands
   without advertising unsupported npm/update/web-ui/custom-TUI features.
5. No standalone browser app, pi-web-ui parity, custom TUI adapter protocol,
   extension overlay/widget system, permission popup subsystem, or package
   ecosystem expansion is added.

## Cross-Cutting Gates (Every Tier)

Run after tier-specific gates:

1. `cargo fmt --check --all` exits 0
2. `cargo clippy --workspace --all-targets -- -D warnings` exits 0
3. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` exits 0
4. `bash scripts/opi-impl-smoke.sh` (or `.ps1` on Windows) exits 0
5. Capture `baseline_dirty_files` at Phase B before implementation starts.
6. Before commit-stage, every entry in
   `git status --porcelain --untracked-files=all` MUST satisfy ONE of:
   - present in `baseline_dirty_files` AND unchanged by this task AND not
     matched by `task_owned_paths` (untouched baseline, leave alone);
   - matched by `task_owned_paths` (intentional task file, will be staged);
   - matched by `task_owned_paths` AND also present in `baseline_dirty_files`
     → REFUSE; print the overlap and ask the user to either split the file
     manually or explicitly confirm the baseline edit is task-owned.
7. Stage only paths matched by `task_owned_paths` AND changed since
   `start_commit`. Never use `git add -A` or `git add .`.
8. Pre-commit: `HEAD` must equal `tasks[].start_commit` unless the only new
   commit is a reviewed manual task commit handled by `--resume-from-manual`.
9. Post-commit: `HEAD^` must equal `start_commit`; no path matched by
   `task_owned_paths` may remain dirty. Files in `baseline_dirty_files` that
   were not modified by the task remain as-is.
10. Commit message includes `Opi-*` evidence footers.

### `--resume-from-manual`

Skip commit creation only if:
- Exactly one candidate manual commit since `start_commit`
- No task-owned dirty files remain outside the candidate manual commit;
  unrelated baseline dirty files are allowed and must not be staged.
- Phase D passes
- Commit already contains `Opi-*` footers

If footer missing: print required footer text and stop (do NOT amend).

## Task Graph Verification Checks

Before confirming an init or reinit graph:

1. Every `behavioral_tests` path must be covered by `task_owned_paths`.
2. If `behavioral_tests` spans multiple crates, use `workspace` tier or include per-crate `cargo test`, `cargo clippy`, and rustdoc gates for every referenced crate.
3. If any behavioral or snapshot test lives under `crates/opi-tui/tests/`, set `snapshot_tests` for the affected snapshot path and mark snapshot acceptance as explicit human approval.
4. Direct spec rows use `parent_spec_row = null`; only dotted sub-task IDs use a parent row string.
5. Rows with open crate labels such as `examples / package template` must include the concrete test paths they declare, even when implementation files live under `examples/**`.
6. Example/package tasks must not own broad `docs/**`; use a task-specific
   docs subtree such as `docs/extension-examples/**`.
7. Reviewed documentation/alignment tasks may own exact documentation files
   required by their DoD, including `docs/opi-spec.md` and localized
   counterparts. They still must not own broad `docs/**`.
8. Public protocol or extension substrate tasks must include documentation
   requirements in their DoD when they introduce RPC, SDK, extension,
   provider/model registration, adapter protocol, transport, or proxy surfaces.
9. No task may include `docs/opi-spec.md` in `task_owned_paths` unless it is a
   reviewed documentation/alignment task whose DoD explicitly requires updating
   `docs/opi-spec.md` and the localized counterpart. Use exact file paths only.
10. Every source-spec goal, success criterion, exit criterion, or named user
    workflow for the active phase must be covered by at least one
    `acceptance_scenarios` entry, or be explicitly deferred by a current spec
    citation.
11. A runtime/startup/CLI/session/adapter/provider acceptance scenario must list
    production call sites. If no production call site exists yet, the owning
    task must be `substrate_only = true` and a later vertical-slice task must
    close the scenario.
12. Vague DoD verbs (`works`, `supports`, `loads`, `integrates`, `bridges`,
    `productizes`, `handles`) must be expanded into observable assertions before
    graph confirmation.
13. For phases 5-12, `spec_files` must match the reviewed source registry in
    `skill.md` for the active phase; arbitrary docs under
    `docs/superpowers/specs/` are not normative.
14. Phase non-goals must appear as `forbidden_scope` inference notes or
    phase-specific verification checks before graph confirmation.

## Risk Evaluator Gate

A task has `evaluator_required = true` when ANY of:
- Tier is `cli-runtime` or `tui`
- Task touches multiple crates or public protocol/data model
- Task changes tool safety, tool selection, allowlists, extension hooks, config,
  session storage, JSON framing, provider events, or release-critical behavior
- Task changes diagnostics, trace envelopes, doctor output, runtime event
  ordering, cancellation, tool result contracts, provider wire formats,
  session context reconstruction, exports, TUI command discovery, accessibility,
  or documented phase non-goal boundaries

`evaluator_required` is static (confirmed at init). Phase D MUST NOT dynamically
promote a task. Phase-exit evaluation is separate (Phase F).

The evaluator receives: DoD, diff from `start_commit`, new/changed tests,
verification outputs, planned commit evidence, acceptance scenarios, production
call-site traces, and current source-spec success/exit criteria. It answers:
1. Does diff satisfy DoD without scope creep?
2. Do tests exercise behavior (not just implementation details)?
3. Public API/protocol/security risks not covered by mechanical gates?
4. Do closed acceptance scenarios start at the promised user/API boundary and
   reach the runtime effect claimed by the design?
5. Are all runtime claims wired through production call sites rather than only
   tested through helper functions?
6. Is evidence footer truthful and sufficient, including `Opi-Acceptance` when
   scenarios are closed?

If evaluator fails → back to Phase C with findings as input. Generator may NOT
self-approve the finding away.
