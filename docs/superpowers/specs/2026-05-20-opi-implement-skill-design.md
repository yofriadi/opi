# `opi-implement` Skill Design

> A long-running-agent harness, packaged as a single superpowers skill, that drives the implementation of `docs/opi-spec.md` one task at a time with TDD, tiered verification, and JSON-ledger checkpointing.

## 0. Document Control

| Field | Value |
|---|---|
| Status | Draft — pending approval |
| Spec version | 0.2-draft |
| Date | 2026-05-20 |
| Author session | Brainstorm with the user, grilling format |
| Target skill | `.claude/skills/opi-implement/skill.md` |
| Companion skill | `.claude/skills/opi-release/skill.md` (existing) |
| Implementation plan | `docs/superpowers/plans/2026-05-20-opi-implement-skill-plan.md` (to be written) |

This document captures the design decisions reached during the brainstorming
session. The skill itself is the implementation; this document is the contract
for what that implementation should do.

Normative terms (MUST / SHOULD / MAY) carry the meanings defined in
`docs/opi-spec.md` §0.

## 1. Purpose

`opi-implement` is the harness that drives long-running implementation of
`docs/opi-spec.md`. It is invoked once per spec task (e.g., task `1.6
agent_loop`), reads the task's Definition of Done from a JSON ledger derived
from the spec, drives a TDD loop to completion, runs tiered verification gates,
and commits exactly one conventional commit on success.

It is a **harness**, not a generic coding assistant: it encodes opinions about
where state lives, what evidence counts as "done", how to recover from failure,
and when to escalate to a human. Those opinions are taken from three Anthropic
engineering posts:

- *Effective harnesses for long-running agents*
- *Harness design for long-running apps*
- *Managed agents*

…and adapted to opi's realities (Rust workspace, lockstep versioning, existing
`opi-release` skill, existing superpowers skills like
`test-driven-development`, `systematic-debugging`,
`dispatching-parallel-agents`).

## 2. Non-Goals

- Pi session-file migration (deferred to Phase 2/3 of opi-spec).
- Cross-compilation, binary distribution, crates.io publishing — owned by
  `opi-release`.
- PR creation and merge/blocking repository review — manual. The risk evaluator
  in §6.7 is an internal harness quality gate, not a PR review substitute.
- Live Anthropic API integration tests — `#[ignore]`-gated; never run by this
  skill.
- Phase 4 extensibility scaffolding (extension trait, RPC) — the skill drives
  whatever tasks exist in `opi-spec.md §15` ledger entries; new phases just
  produce new ledger rows after `--reinit`.
- Auto-updating `opi-spec.md` — the spec is the human contract; this skill
  reads it, never writes to it.
- Decision-making about phase boundaries — phase exit reports only; the human
  decides when to invoke `opi-release`.

## 3. Core Decisions

These were settled during the brainstorming session. Each is the chosen
option from a multi-choice grill.

| Dimension | Decision |
|---|---|
| Work unit | One spec task per invocation. |
| State location | JSON ledger derived from spec (`.opi-impl-state.json`). |
| Verification | Tiered by task type: `workspace`, `library`, `cli-tool`, `cli-runtime`, `tui`. |
| TDD enforcement | Invoke `superpowers:test-driven-development` as a mandatory sub-step. |
| Invocation | Smart default (auto-pick) + optional `<task>` / `--status` / `--reinit` overrides. |
| Failure mode | Bounded debug loop → escalate (3 impl attempts, then `systematic-debugging`, total cap 5). |
| Bootstrap | Phase-aware smoke (`scripts/opi-impl-smoke.sh`). |
| Phase exit | Stop and report; no auto-release. |
| Commit policy | One conventional commit per task; type derived from ledger `commit_type` field. |
| Evidence policy | Runtime ledger is gitignored; successful task evidence is recoverable from the task commit footer. |
| Task graph review | Inferred `depends_on`, tier, and commit metadata MUST be explicitly reviewed; no silent graph rewrites. |
| Independent evaluation | Task-level risk evaluator for `cli-runtime`, `tui`, cross-crate/public-protocol tasks; separate Phase F evaluator for phase exits. |
| Sub-agent dispatch | Opt-in via per-task `parallelize:` field; invokes `superpowers:dispatching-parallel-agents`. |
| Name | `opi-implement`. |

## 4. Architecture: Six Phases Per Invocation

Every invocation runs six phases. Phases A, B, F are cheap and always execute;
phases C and D form the body of the work; phase E is the only one that mutates
git.

```text
Phase A: Bootstrap        (every invocation)
  A.1  Detect mode (init / status / reinit / task / auto)
  A.2  Load or create .opi-impl-state.json
  A.3  Session ritual: pwd, git status, git log -5 --oneline, smoke
  A.4  Select target task (auto-pick or validate user override)
       Auto-pick rule: lowest task `id` (lexicographic, numerically aware)
       whose `status` is `failing` AND every entry in `depends_on` is
       `passing`. Tasks with `status: blocked` are skipped during auto-pick;
       they remain in the ledger and become available again only after
       `--clear-blocker`.
       User-override rule: refuse if any `depends_on` entry is not `passing`,
       printing which dep is missing.

Phase B: Plan-the-task
  B.1  Print task DoD + verification tier + parallelize plan
  B.2  User gate: "proceed with task <id>?"
  B.3  If confirmed, mark task `in_progress` and record `start_commit`

Phase C: Implement
  C.1  Invoke superpowers:test-driven-development (red→green→refactor)
       └── if parallelize: → superpowers:dispatching-parallel-agents
  C.2  Iteration cap 3; on 3rd fail → invoke systematic-debugging
  C.3  Total cap 5; on cap hit → failure decision gate

Phase D: Verify
  D.1  Tier-specific mechanical gates
       (library / cli-tool / cli-runtime / tui / workspace)
  D.2  Task-level risk evaluator gate when static `evaluator_required = true`
  D.3  Cross-cutting gates: fmt, clippy -D warnings, cargo doc -D warnings
  D.4  Smoke re-run (phase-aware)
  D.5  If any fail → back to Phase C iteration

Phase E: Commit & ledger update
  E.1  Conventional commit with parseable `Opi-*` evidence footers
       (type derived from ledger commit_type field)
  E.2  Capture HEAD SHA + verification evidence → ledger
  E.3  Flip status to passing; append session_note
  E.4  No push (push is a separate human action)

Phase F: Phase-exit check
  F.1  If all executable tasks in current phase are passing → run phase-exit evaluator
       (dynamic Phase F gate, independent of any task's `evaluator_required`)
  F.2  Print phase-complete report; no auto-release
  F.3  Else → print "next unblocked: X.Y" hint
```

### 4.1 Initializer Mode

When `.opi-impl-state.json` is absent, OR when `--reinit` is passed, phase A
is replaced by an extended initializer:

```text
Phase A.init:
  A.init.1  Pre-flight: confirm git clean, on main, opi-spec.md present
  A.init.2  Parse opi-spec.md §15 roadmap tables; for each task row, extract:
              - id, title, crate, DoD string when present, phase number
              - infer tier from crate + task description
              - infer commit_type from task verbs
              - infer depends_on from numeric ordering + DoD references
              - attach inference_notes for every non-verbatim field
              - infer evaluator_required from risk rules
            Rows without a DoD are rendered as deferred spec rows, not
            executable ledger tasks, unless a reviewed JSON draft supplies a
            concrete DoD.
  A.init.3  Task-graph review gate. Render the complete draft as a table
            with id, title, tier, commit_type, depends_on, execution order,
            evaluator_required, and inference_notes.
            Gate options:
              - confirm-all
              - edit-task <id>
              - apply-rule <selector> <field> <value>
              - export-draft
              - import-draft
              - abort
            Every edit or import re-renders the table before confirmation.
  A.init.4  Refuse to proceed until the whole graph is confirmed. The skill
            MUST NOT silently apply inferred dependency, tier, commit_type, or
            evaluator changes.
  A.init.5  Write .opi-impl-state.json atomically; add `.opi-impl-state.json`,
            `.opi-impl-state.json.tmp`, and `.opi-impl-state.draft.json` to
            .gitignore if missing.
  A.init.6  Write scripts/opi-impl-smoke.sh (.ps1 sibling on Windows)
  A.init.7  Commit ONLY the tracked files (smoke script + .gitignore update);
            the ledger itself is NOT committed since it is gitignored runtime
            state. Commit message:
              chore: bootstrap opi-implement ledger and smoke
  A.init.8  Print success summary with the next-task hint
```

`apply-rule` is for batch graph edits that would be tedious one row at a
time. Examples include adding `1.17` as a dependency to every task whose
verification uses `MockProvider`, changing all `opi-tui` rows to tier `tui`,
or marking public-protocol rows as `evaluator_required = true`. The skill must
show a before/after diff for the affected rows and then return to A.init.3.

`export-draft` writes `.opi-impl-state.draft.json` for human editing. The draft
file is gitignored runtime scratch. `import-draft` validates schema version,
task ID uniqueness, dependency references, cycle freedom, and known tier names
before re-rendering the graph. Import never counts as confirmation by itself.
If a draft promotes a deferred spec row into an executable task, it must supply
a concrete `definition_of_done` and an inference note explaining the source.

### 4.2 Reinit Reconciliation

When `--reinit` runs against an existing ledger:

1. Recompute `spec_sha256`. If unchanged, refuse — suggest `--status` instead.
2. Re-parse the spec into a fresh ledger.
3. Reconcile field-by-field:
   - Task IDs present in both: preserve `status`, `verified_at_commit`,
     `iteration_count`, `session_notes`, `blocker`.
   - Task IDs only in old ledger: warn, ask "keep history, mark `archived`?".
   - Task IDs only in new ledger: add with status `failing`.
   - DoD string changed for existing passing task: warn, ask the user to
     either preserve as `passing` (acknowledging the wording change is
     cosmetic) or demote to `failing` (DoD substantively widened).
   - `depends_on`, `tier`, `commit_type`, or `evaluator_required` changed:
     re-run the task-graph review gate with a row-level diff and require
     confirmation before writing the reconciled ledger.
4. Update `spec_sha256` after confirmation. If tracked files changed
   (`.gitignore` or smoke scripts), commit only those files with:
   `chore: reconcile opi-implement harness files with opi-spec.md changes`.
   If no tracked file changed, do not create an empty commit. The runtime
   ledger and draft file remain gitignored.

## 5. JSON Ledger Schema

Path: `.opi-impl-state.json` at repository root. Gitignored — runtime artifact,
not source. Atomic writes via `.opi-impl-state.json.tmp` + rename.

```json
{
  "schema_version": 1,
  "spec_path": "docs/opi-spec.md",
  "spec_sha256": "<hash of opi-spec.md at last init/reinit>",
  "task_graph_confirmed_at": "2026-05-20T09:30:00Z",
  "current_phase": 1,
  "tasks": [
    {
      "id": "1.6",
      "phase": 1,
      "title": "agent_loop",
      "crate": "opi-agent",
      "definition_of_done": "mock tests cover no-tool and tool-use turns",
      "status": "failing",
      "depends_on": ["1.1", "1.2", "1.5"],
      "inference_notes": [
        {
          "field": "depends_on",
          "reason": "agent_loop consumes provider and tool traits",
          "source": "opi-spec.md §15 + DoD references"
        }
      ],
      "tier": "library",
      "commit_type": "feat",
      "parallelize": [],
      "evaluator_required": false,
      "verification": {
        "library_gates": [
          "cargo test -p opi-agent",
          "cargo clippy -p opi-agent -- -D warnings",
          "cargo doc -p opi-agent -- -D warnings"
        ],
        "behavioral_tests": ["crates/opi-agent/tests/agent_loop_mock.rs"],
        "snapshot_tests": [],
        "smoke_addendum": null
      },
      "iteration_count": 0,
      "max_iterations": 5,
      "start_commit": null,
      "last_attempt": null,
      "verified_at_commit": null,
      "evidence": null,
      "blocker": null,
      "session_notes": []
    }
  ],
  "phase_exit": {
    "1": {
      "completed_at": null,
      "exit_criteria_met": false,
      "evaluator_summary": null
    }
  }
}
```

### 5.1 Field Semantics

| Field | Type | Mutability | Notes |
|---|---|---|---|
| `schema_version` | int | const | Bump when ledger format changes; skill refuses to operate on unknown versions. |
| `spec_path` | string | const | Default `docs/opi-spec.md`; override allowed in init for non-standard layouts. |
| `spec_sha256` | string | reinit-only | Drift detection. |
| `task_graph_confirmed_at` | string/null | init/reinit | ISO-8601 time when the whole inferred graph was confirmed. |
| `current_phase` | int | auto | Set to the lowest phase containing a non-`passing` task. |
| `tasks[].id` | string | const | Matches opi-spec.md §15 row id (`1.6`, `2.7`, etc.). |
| `tasks[].phase` | int | const | Derived from row's phase grouping. |
| `tasks[].title` | string | const | Spec row title, free text. |
| `tasks[].crate` | string | const | One of opi's five crates, or `workspace`. |
| `tasks[].definition_of_done` | string | const | Verbatim from spec; reinit may flag changes. |
| `tasks[].status` | enum | runtime | `failing` / `in_progress` / `passing` / `blocked` / `archived`. |
| `tasks[].depends_on` | array | const | Other task IDs that must be `passing`. |
| `tasks[].inference_notes` | array | const | Human-reviewed reasons for inferred tier, dependencies, commit type, evaluator flag, or execution-order changes. Empty only when every field is verbatim. |
| `tasks[].tier` | enum | const | `workspace` / `library` / `cli-tool` / `cli-runtime` / `tui`. |
| `tasks[].commit_type` | enum | const | Conventional Commits type: `feat`/`fix`/`docs`/`refactor`/`test`/`chore`/`perf`. |
| `tasks[].parallelize` | array | const | Sub-unit names for `dispatching-parallel-agents`. Empty = sequential. |
| `tasks[].evaluator_required` | bool | const | Task-level evaluator flag. True for `cli-runtime`, `tui`, cross-crate, public-protocol, and security-sensitive tasks. Phase-exit evaluation is tracked separately in `phase_exit[N]`. |
| `tasks[].verification` | object | const | Tier-specific gate spec. |
| `tasks[].iteration_count` | int | runtime | Attempts since first `in_progress` flip. Reset on success. |
| `tasks[].max_iterations` | int | const | Default 5; per-task override allowed. |
| `tasks[].start_commit` | string/null | runtime | HEAD at the moment Phase B confirms proceed and marks the task `in_progress`. Used for diff, chain, and cleanup diagnostics. |
| `tasks[].last_attempt` | object/null | runtime | Last attempt status: `{attempt, started_at, ended_at, outcome, failing_gate, touched_files}`. |
| `tasks[].verified_at_commit` | string | runtime | Set in Phase E.2 on success. |
| `tasks[].evidence` | object/null | runtime | Mirror of the parseable `Opi-*` evidence footers in the success commit. |
| `tasks[].blocker` | string | runtime | Populated when `status = blocked`. |
| `tasks[].session_notes` | array | runtime | Append-only `{timestamp, attempt, summary, gate_results}`. Short. |
| `phase_exit[N]` | object | runtime | `completed_at` ISO-8601 + `exit_criteria_met` boolean + evaluator summary. |

### 5.2 Durable Evidence Contract

The ledger is the mutable runtime state for the local harness and remains
gitignored. It is not the only recoverable evidence. Every successful task
commit MUST include parseable footers:

```text
Opi-Task: <id>
Opi-DoD-SHA256: <sha256 of definition_of_done>
Opi-Verification: <tier>; <short command/result summary>
Opi-Evaluator: <not-required | passed>
```

The same values are copied into `tasks[].evidence` together with the full
command list and reviewer summary when present. A fresh clone may not have the
runtime ledger, but `git log --grep "Opi-Task:"` must be enough for a human or
future helper to reconstruct which spec tasks were completed and what evidence
was claimed. Phase-exit evaluator results are recorded in `phase_exit[N]` and
the printed phase report; they are not retroactively added to task commits. A
tracked JSONL progress file MAY be added in a future version, but v1 uses commit
evidence to avoid committing high-churn runtime state.

### 5.3 Atomic Write Protocol

The ledger is written at durable boundaries, never from ad-hoc string
concatenation:

1. End of Phase B after the user confirms proceed: mark target task
   `in_progress`, record `start_commit`, and initialize `last_attempt`. Phase A
   task selection alone does not mutate task status.
2. Each attempt boundary: record attempt start, failing gate, touched files, and
   truncated gate output.
3. Failure decision gate: mark `blocked`, extend cap, or record manual handoff.
4. End of Phase E: mark task `passing`, record commit and evidence, append note.
5. Reinit after the task graph review gate is confirmed.

Write sequence:

1. Serialize the full JSON document with a structured JSON writer
   (`serde_json`, Python `json`, PowerShell `ConvertTo-Json`, or `jq`), not
   shell `echo`.
2. Write to `.opi-impl-state.json.tmp` in the repository root.
3. Flush the file, and fsync the parent directory when the platform exposes it.
4. Rename/replace `.opi-impl-state.json.tmp` over `.opi-impl-state.json` on the
   same volume.
5. On failure, leave the previous ledger intact and print the tmp path for
   manual inspection.

### 5.4 Interrupt Recovery

On next invocation, if a task has `status = in_progress` AND
`verified_at_commit = null`, inspect both `last_attempt` and the working tree.
If the working tree is clean, prompt:

> "Task X was marked `in_progress` but no commit was recorded. Was the prior
> session interrupted? Reset to `failing` and retry, or investigate first?"

If the working tree is dirty, the skill MUST NOT reset, restore, clean, or
discard files. It prints:

- `start_commit`
- current `git status --short`
- files changed since `start_commit`
- last failing gate and reproduction commands

Then it offers only: continue investigation, mark blocked with blocker text, or
drop to manual session. Dirty-tree recovery is a handoff, not an automated
rollback.

## 6. Verification Tiers

Each task carries a `tier` field; the skill selects the gate set from this
table. All tiers also run the **cross-cutting gates** at the bottom.

### 6.1 `workspace`

Tasks: 1.0 (deps), 1.17 (integration harness), and any future task whose
crate field is `workspace`.

Gates:
- `cargo fmt --check --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- Smoke script runs.

### 6.2 `library`

Tasks: 1.1–1.8 (`opi-ai`, `opi-agent` internals).

Gates:
- TDD red→green produced new or changed tests in `crates/<crate>/tests/` OR
  `#[cfg(test)]` modules. `git diff --stat <start_commit> -- crates/<crate>` is
  only the overview; the skill must inspect diff content for test files,
  `#[test]`, async test attributes, or changed assertion bodies.
- `cargo test -p <crate>` green.
- `cargo clippy -p <crate> -- -D warnings` green.
- `cargo doc -p <crate> -- -D warnings` green.
- Workspace `cargo build --workspace` green (catches breaking-API changes).
- No `unwrap`/`expect` in non-test code (grep check; allow-list configurable
  via `.opi-impl-allow-unwrap` if ever needed).

### 6.3 `cli-tool`

Tasks: 1.9 (`read`/`write`/`edit`/`bash`), 1.10 (`glob`/`grep`).

Gates: `library` gates above, plus:
- Behavioral tests in `crates/opi-coding-agent/tests/` that use the `tempfile`
  crate to exercise real filesystem operations.
- For `bash` specifically: separate tests for timeout, cwd capture, and
  cancellation behavior.
- For mutating tools (`write`, `edit`, `bash`): a test asserting that the
  Phase-1 safety boundary (visible command, effective cwd, env policy,
  timeout) is reported before execution. See opi-spec §8.4.

### 6.4 `cli-runtime`

Tasks: 1.11 (system prompt), 1.14 (interactive wiring), 1.15
(non-interactive), 1.16 (config).

Gates: `library` gates plus:
- End-to-end test that boots a `MockProvider` and runs the `opi` binary in a
  subprocess against scripted prompts.
- Assertions on stdout, stderr, and exit code.

**MockProvider precondition**: this tier MUST refuse to run if no
`MockProvider` is registered. The skill greps
`crates/opi-ai/src/test_support.rs` (or feature-gated module path) and
verifies a `MockProvider` symbol exists. If absent, the skill prints:
"Task `<id>` depends on the MockProvider scaffolding (task 1.17). Run task
1.17 first."

This creates a dependency-ordering issue versus `opi-spec.md` §15: tasks
1.14, 1.15 are listed numerically before 1.17 but functionally require it.
The initializer's inference (§4.1 A.init.2) MUST draft a `"1.17"` dependency
for every task whose verification requires `MockProvider`, excluding task 1.17
itself and any task whose DoD is to create the mock provider scaffolding. This
draft edge is an inferred edge, not a silent rewrite: A.init.3 must display the
reason, show the changed execution order, and require whole-graph confirmation.
The numeric ID is preserved as the immutable spec anchor; only execution order
is reshaped after human approval.

### 6.5 `tui`

Tasks: 1.12 (TUI shell), 1.13 (markdown/code rendering).

Gates: `library` gates plus:
- Ratatui snapshot tests at fixed sizes (80×24 and 120×40). Snapshots use
  `insta` (or equivalent). Snapshot file diffs reviewed mechanically; the
  skill refuses to auto-accept snapshot changes — they require explicit user
  approval in Phase B.

### 6.6 Cross-Cutting Gates (every tier)

Run after the tier-specific gates:

- `cargo fmt --check --all` exits 0.
- `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` exits 0.
- `bash scripts/opi-impl-smoke.sh` exits 0.
- Before commit-stage, `git status --porcelain --untracked-files=all` may
  contain only intentional files for the active task. Gitignored ledger/draft
  files do not appear in this output by default and are checked separately by
  path. The skill prints the status and stages only reviewed task files;
  unrelated user changes are never staged or cleaned.
- Immediately before creating the task commit, `HEAD` must still equal
  `tasks[].start_commit`. If an intermediate manual commit exists, the skill
  refuses normal Phase E and requires `--resume-from-manual`.
- After the task commit, `git status --porcelain` must be clean. Gitignored
  runtime ledger/draft files may still exist, but they are checked by path and
  are not part of porcelain cleanliness. `HEAD^` must equal
  `tasks[].start_commit`.
- The commit message must include the `Opi-*` evidence footers from §5.2.

For `--resume-from-manual`, the skill skips commit creation only if there is
exactly one candidate manual commit for the task since `start_commit`, the
working tree is clean, Phase D passes, and that commit message already contains
the required `Opi-*` footers. If the footer is missing, the skill prints the
required footer text and stops; it does not amend the user's commit.

### 6.7 Risk Evaluator Gate

Mechanical gates are necessary but not sufficient for tasks that change public
behavior, cross crate boundaries, or user-facing runtime flows. A task has
`evaluator_required = true` when any of these apply:

- tier is `cli-runtime` or `tui`;
- task touches multiple crates or a public protocol/data model;
- task changes tool safety, permissions, config loading, session storage, JSON
  framing, provider event semantics, or release-critical behavior.

`evaluator_required` is a static task field confirmed during init/reinit. Phase
D MUST NOT dynamically promote a task to evaluator-required merely because it is
the last unfinished task in a phase. Phase-exit evaluation is a separate dynamic
Phase F gate and does not depend on the last task's `evaluator_required` value.

For task-level evaluation, the evaluator receives the DoD, diff from
`start_commit`, new/changed tests, verification outputs, and the planned commit
message evidence. It must answer:

1. Does the diff satisfy the DoD without scope creep?
2. Do tests actually exercise the behavior, or only implementation details?
3. Are there public API/protocol/security risks not covered by mechanical
   gates?
4. Is the evidence footer truthful and sufficient for future recovery?

If the evaluator fails the task, Phase D returns to Phase C with the evaluator
findings as the next implementation input. The generator may not self-approve
the finding away.

For phase exit, the evaluator checks the phase's exit criteria from
`opi-spec.md §15`, the list of task evidence footers, and the smoke output. The
phase is reported complete only when the evaluator finds no blocking gap. The
report is advisory for release; `opi-release` remains a separate human action.

### 6.8 Phase 2/3/4 Tiers

This document covers Phase 1 tiers in detail. Phase 2 will introduce:
- `session-storage` tier (round-trip tests, fuzz harness).
- `provider-contract` tier (SSE fixture tests for each new provider).
- `json-contract` tier (NDJSON schema tests, line framing).

Phase 2 tier definitions and DoDs SHOULD be added to the skill implementation
or reviewed ledger draft at `--reinit` time when the corresponding tasks are
first encountered, NOT silently inferred from roadmap rows that lack DoD text.

## 7. Failure Decision Gate

When `iteration_count` reaches `max_iterations` (default 5), the skill stops
and hands the decision to the user via `AskUserQuestion`. No model
self-deliberation past this point.

### 7.1 Gate Payload

The skill prints:

```text
Task: <id> <title>
DoD: <definition_of_done>
Tier: <tier>
Iterations: <iteration_count> / <max_iterations>
Last gate output (truncated to 50 lines): <…>
Tests added but failing: <list>
Files modified: <list>
Smallest failing assertion: <quote from test output>
Start commit: <tasks[].start_commit>
Dirty status: <git status --short>
Reproduction commands: <exact commands>
```

### 7.2 Options

| Option | Effect |
|---|---|
| (a) Retry with extended cap | Adds 5 attempts (total budget 10). Status stays `in_progress`. |
| (b) Escalate to design | Invokes `superpowers:brainstorming` on the DoD interpretation. User may amend `opi-spec.md` and `--reinit`. |
| (c) Mark blocked | Records blocker text. Leaves failing tests in place. Stages no changes. Status → `blocked`. Skill will skip on `auto` selection until cleared via `--clear-blocker`. |
| (d) Drop to manual session | Prints exact reproduction commands, touched files, and suggested cleanup commands, but does not run them. User finishes manually, then `--resume-from-manual` skips to Phase D verify. |

There is intentionally no "auto-revert" option. The skill MUST NOT run
`git restore`, `git clean`, `git reset`, or equivalent destructive cleanup from
the failure gate. If abandoning an attempt requires cleanup, the skill prints
candidate commands scoped to files changed since `start_commit` and exits for a
human to decide.

### 7.3 Stuck-On-Many-Tasks Meta-Warning

If three consecutive task invocations hit the failure gate, the skill prints
a meta-warning:

> "Harness components may be misaligned with the current spec or model.
> Consider re-reading opi-spec.md §15 exit criteria, or grilling the design
> via `superpowers:brainstorming` before continuing."

This is the harness-design article's "re-examine the harness on every model
release" baked into the skill's operating loop.

## 8. Anti-Pattern Guards

These are explicit prompt rules in the skill body. Each maps to a documented
failure mode in the source articles.

| Rule | Source |
|---|---|
| Never delete or weaken tests to make them pass. | Effective harnesses article |
| Never `git push --force`. | CLAUDE.md + opi-release |
| Never bypass `cargo clippy -D warnings` with crate-wide `#[allow]`. | Project convention |
| Never commit with broken smoke. | Effective harnesses article |
| Never commit unstaged secrets. | opi-spec §13 |
| Never bypass git hooks (`--no-verify`). | CLAUDE.md |
| Never use `git reset --hard` + force push for rollback. | opi-release |
| Never use `--amend` on already-pushed commits. | CLAUDE.md |
| Never self-grade verification — the gates are mechanical. | Harness-design article |
| Never auto-accept TUI snapshot changes without user approval. | This skill |
| Never silently rewrite inferred task graph metadata. | Managed Agents interface discipline |
| Never run live provider tests from this skill. | opi-spec §12 + §15 scope |
| Never commit `.opi-impl-state.json`, `.opi-impl-state.json.tmp`, or `.opi-impl-state.draft.json`. | This skill |
| Never skip `[workspace.dependencies]` when adding internal crate deps. | CLAUDE.md |
| Never satisfy a DoD with placeholder stubs, TODOs, or display-only behavior unless the DoD explicitly asks for scaffolding. | Harness-design article |
| Never broaden a task into cross-task refactors without updating the task graph and returning to the review gate. | Effective harnesses article |
| Never clean, restore, or discard user changes from a failure gate. | Cursor editing constraints |
| Never let sub-agent completion order decide persisted result order. | pi tool execution semantics |

The skill MUST refuse to act if any of these rules would be violated, even
if the user requests it during an interactive failure-decision gate.

## 9. Composition With Existing Skills

The skill invokes existing skills when available and may dispatch a platform
reviewer subagent for the risk evaluator. It never re-implements these
workflows inline.

| Phase | Mechanism | Purpose |
|---|---|---|
| C.1 | `superpowers:test-driven-development` | red→green→refactor body |
| C.1 (when `parallelize` non-empty) | `superpowers:dispatching-parallel-agents` | many-brains for independent sub-units |
| C.2 (attempt 3+) | `superpowers:systematic-debugging` | when implementation can't reach green |
| D.2 (risk-gated) | code-reviewer subagent or `superpowers:requesting-code-review` | independent evaluator for DoD coverage, tests, and public-surface risk |
| D pre-commit | `superpowers:verification-before-completion` | enforce evidence-before-claim |
| Failure gate (b) | `superpowers:brainstorming` | when DoD interpretation is ambiguous |
| Phase F (report only) | `opi-release` | mentioned in phase-exit report; never auto-invoked |

Each invocation announces itself per the using-superpowers contract:
`"Using superpowers:test-driven-development to drive red-green for task 1.6"`.

### 9.1 Parallel Sub-Unit Merge Contract

`parallelize` means independent investigation or implementation sub-units inside
one spec task; it does not relax the one-task/one-commit rule.

- Sub-agents MUST work on disjoint files or return patch bundles for parent
  application. They MUST NOT create commits.
- The parent applies results in ledger order, runs the full task verification
  after each merge, and records merge notes in `last_attempt`.
- Completion events may arrive in sub-agent completion order, but persisted
  task evidence and final commit content are ordered by the `parallelize` array.
- Any conflict, overlapping edit, or inconsistent test expectation fails the
  attempt and enters the normal debug/failure path. The parent does not choose
  a winner silently.

## 10. Skill Argument Surface

```text
/skill opi-implement                                  # auto-pick lowest-ID unblocked failing task
/skill opi-implement <task-id>                        # specific task; validates deps, refuses if blocked
/skill opi-implement --status                         # print ledger summary, exit
/skill opi-implement --reinit                         # re-parse spec, reconcile ledger
/skill opi-implement <task-id> --resume-from-manual   # verify one manual task commit with Opi-* footers
/skill opi-implement <task-id> --extend-cap <N>       # raise iteration cap for this invocation only
/skill opi-implement --clear-blocker <task-id> --because <text>
                                                        # remove blocker text, status → failing, append justification
```

`<task-id>` matches the ID format used in opi-spec §15 (e.g., `1.6`, `2.7`).

## 11. Files Created or Touched

| Path | Owner | Tracked |
|---|---|---|
| `.claude/skills/opi-implement/skill.md` | this skill | yes |
| `.opi-impl-state.json` | runtime | NO (gitignored) |
| `.opi-impl-state.json.tmp` | runtime | NO (gitignored) |
| `.opi-impl-state.draft.json` | task-graph review scratch | NO (gitignored) |
| `scripts/opi-impl-smoke.sh` | initializer | yes |
| `scripts/opi-impl-smoke.ps1` | initializer (Windows) | yes |
| `docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md` | this brainstorm | yes |
| `docs/superpowers/plans/2026-05-20-opi-implement-skill-plan.md` | writing-plans output | yes |
| `.gitignore` (appended) | initializer | yes (modified) |

## 12. Platform & Tooling Requirements

Checked at Phase A.1 of every invocation. Missing tool = clean refusal.

| Tool | Required | Notes |
|---|---|---|
| `cargo` | yes | Rust toolchain ≥ 1.85 (edition 2024). Verified via `rustc --version`. |
| `git` | yes | |
| `jq` | preferred | Used only when present. The skill must have a non-`jq` path via PowerShell JSON cmdlets, Python, or a small Rust helper. |
| SHA-256 helper | yes | `sha256sum`, PowerShell `Get-FileHash`, Python, or Rust helper. Do not require a specific binary on every OS. |
| POSIX `sh` | yes (Linux/macOS) | Runs `scripts/opi-impl-smoke.sh`. |
| PowerShell | yes (Windows) | Runs `scripts/opi-impl-smoke.ps1`. |
| `gh` CLI | NO | Never required by this skill. Release-related actions belong to `opi-release`. |

The skill detects host via `OSTYPE`/`OS` env vars and chooses the smoke
script variant. Bash-on-Windows (as per the CLAUDE.md project shell) uses the
POSIX `.sh` script with forward-slash paths.

All ledger manipulation must use structured JSON APIs. Shell snippets in the
skill may inspect JSON for display, but must not synthesize ledger JSON with
string concatenation.

## 13. Mapping to Anthropic Harness Articles

| Article principle | Skill mechanism |
|---|---|
| Shift-handover model | One task per invocation; ledger is the handover artifact. |
| JSON ledger, not Markdown | `.opi-impl-state.json` is mutable; opi-spec.md is immutable. |
| Boot-time smoke catches prior breakage | Phase A.3 runs `scripts/opi-impl-smoke.sh` before any task work. |
| Generator/evaluator separation | TDD provides the first evaluator; risk-gated independent review catches DoD/test-quality gaps. |
| Test the running app, not artifacts | `cli-runtime` tier runs the binary as a subprocess. |
| Decouple brain/hands/session | Brain = Claude + skill prompt; hands = cargo/git/sub-skills guarded by policy; session = ledger + git evidence. |
| Session is not the context window | Commit evidence and ledger notes are external artifacts, not transient chat memory. |
| Append-only durable session log | git history + parseable `Opi-*` footers + append-only `session_notes`. |
| Iteration caps | 3-attempt impl, then `systematic-debugging`, total cap 5. |
| Re-examine harness on each model release | `schema_version` field + three-consecutive-failure meta-warning. |
| Anti-pattern: trust agent to grade itself | Tiered gates are mechanical, not LLM-graded. |
| Anti-pattern: edit tests to pass | Explicit prompt rule against test deletion/weakening. |
| Anti-pattern: irreversible compaction | Ledger session_notes are append-only; status is a finite state machine. |
| Anti-pattern: bake infrastructure assumptions into the harness | Inferred graph changes are reviewed; smoke script is phase-aware and regenerated at phase-exit boundaries. |

## 14. Decisions Carried Into Implementation

These decisions are fixed for the first implementation of the skill:

1. **Tier inference is conservative and reviewable.** Multi-crate tasks default
   to the strictest applicable tier and `evaluator_required = true`. Any task
   that spans library and CLI behavior inherits the CLI/runtime gate.
2. **TUI snapshots default to `insta`.** If Phase 1 dependency task 1.0 rejects
   `insta` for dependency reasons, the initializer must update the graph through
   A.init.3 rather than silently choosing another library.
3. **MockProvider presence contract is minimal.** Task 1.17 owns the exact API,
   but by the time any MockProvider-gated task runs, a symbol named
   `MockProvider` must exist under `crates/opi-ai/src/test_support.rs` or an
   explicitly reviewed feature-gated equivalent.
4. **`--clear-blocker` requires justification.** The `--because <text>` value is
   appended to `session_notes` before status changes back to `failing`.
5. **Spec graph corrections stay outside `opi-spec.md`.** This skill may infer
   and review execution edges, but it never edits the spec. If the human wants
   the roadmap itself changed, they do that separately and run `--reinit`.

## 15. Out of Scope for This Skill

Restated for clarity; same content as §2 Non-Goals but grouped here as
explicit boundary lines the skill MUST NOT cross:

- Editing `opi-spec.md`.
- Pushing commits or tags to `origin`.
- Publishing to crates.io.
- Building cross-platform binaries.
- Making network calls to Anthropic, OpenAI, or any provider API.
- Opening GitHub issues, PRs, or releases.
- Reading or writing `~/.config/opi/` or session storage paths — those are
  runtime concerns of the `opi` binary, not the implementation skill.

## 16. References

- `docs/opi-spec.md` (the spec this skill implements)
- `.claude/skills/opi-release/skill.md` (companion skill, conventions)
- `docs/superpowers/specs/2026-05-19-opi-release-skill-design.md`
- *Effective harnesses for long-running agents* — Anthropic engineering
- *Harness design for long-running apps* — Anthropic engineering
- *Managed agents* — Anthropic engineering
- `anthropics/claude-quickstarts` (autonomous-coding reference)
- superpowers skills: `test-driven-development`, `systematic-debugging`,
  `dispatching-parallel-agents`, `verification-before-completion`,
  `brainstorming`, `requesting-code-review`, `writing-plans`,
  `executing-plans`, `subagent-driven-development`,
  `finishing-a-development-branch`
