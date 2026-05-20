---
name: opi-implement
description: Use when implementing opi roadmap tasks from docs/opi-spec.md, checking opi implementation status, resuming blocked or in-progress opi task work, or reconciling the opi task ledger after spec changes
arguments: "[<task-id>] [--status] [--reinit] [--resume-from-manual] [--extend-cap <N>] [--clear-blocker <task-id> --because <text>]"
---

# opi-implement

Long-running-agent harness that implements `docs/opi-spec.md` one spec task per invocation. Reads task definitions from a JSON ledger, drives TDD to completion, runs tiered verification gates, and commits exactly one conventional commit on success.

## Arguments

| Invocation | Effect |
|---|---|
| `/skill opi-implement` | Auto-pick lowest-ID unblocked failing task |
| `/skill opi-implement <task-id>` | Specific task; validates deps, refuses if blocked |
| `/skill opi-implement --status` | Print ledger summary table, exit |
| `/skill opi-implement --reinit` | Re-parse spec, reconcile ledger |
| `/skill opi-implement <task-id> --resume-from-manual` | Verify one manual task commit with Opi-* footers |
| `/skill opi-implement <task-id> --extend-cap <N>` | Raise iteration cap for this invocation only |
| `/skill opi-implement --clear-blocker <task-id> --because <text>` | Remove blocker, status → failing, append justification |

`<task-id>` matches the ID format in opi-spec.md §15 (e.g., `1.6`, `2.7`).

## Phase A: Bootstrap

Run on every invocation. Establishes context and selects the target task.

### A.1 Detect Mode

Parse arguments to determine mode:
- No args → `auto` mode
- `--status` → print ledger summary, exit
- `--reinit` → jump to Initializer Mode (§Phase A.init)
- `<task-id>` → `task` mode (validate deps before proceeding)
- `<task-id> --resume-from-manual` → resume mode
- `--clear-blocker <task-id> --because <text>` → clear blocker, exit

### A.2 Load Ledger

1. If `.opi-impl-state.json` is absent → jump to Initializer Mode (§Phase A.init)
2. Read and parse the JSON ledger
3. Validate `schema_version` equals 1; refuse on unknown versions
4. Compute SHA-256 of `docs/opi-spec.md`; if it differs from `spec_sha256`, warn:
   > "Spec has changed since last init. Consider running `--reinit` to reconcile."

### A.3 Session Ritual

Run these commands and print results:

```bash
pwd
git status --short
git log -5 --oneline
bash scripts/opi-impl-smoke.sh   # or PowerShell on Windows
```

If smoke fails → STOP. Print the failure and refuse to proceed. The smoke must pass before any task work begins.

### A.4 Select Target Task

**Auto-pick rule** (no task-id argument):
- Find the lowest task `id` (lexicographic, numerically aware: 1.2 < 1.10) whose `status` is `failing` AND every entry in `depends_on` has status `passing`.
- Tasks with `status: blocked` are skipped.
- If no task is eligible, print "All tasks are either passing, blocked, or have unmet dependencies" and exit.

**User-override rule** (task-id argument):
- Validate the task exists in the ledger.
- Refuse if any `depends_on` entry is not `passing`, printing which dep is missing.
- Refuse if status is `blocked` (suggest `--clear-blocker`).

**Interrupt recovery** (task has `status = in_progress` AND `verified_at_commit = null`):
- See §Interrupt Recovery section for handling.

## Phase A.init: Initializer Mode

Triggered when `.opi-impl-state.json` is absent OR `--reinit` is passed.

### A.init.1 Pre-flight

Confirm:
- Working tree is clean (`git status --porcelain` is empty)
- On `main` branch
- `docs/opi-spec.md` exists

If any check fails, print the issue and refuse to proceed.

### A.init.2 Parse Spec Roadmap

Parse `docs/opi-spec.md` §15 roadmap tables. For each task row, extract:
- `id` — task number (e.g., `1.6`)
- `title` — task name
- `crate` — target crate
- `definition_of_done` — DoD string (verbatim from spec when present)
- `phase` — phase number from grouping

Infer (with `inference_notes` for each):
- `tier` — from crate + task description:
  - `opi-ai`, `opi-agent` internals → `library`
  - `opi-coding-agent` tool tasks → `cli-tool`
  - `opi-coding-agent` runtime/wiring → `cli-runtime`
  - `opi-tui` → `tui`
  - workspace-level → `workspace`
- `commit_type` — from task verbs (add/create → `feat`, fix → `fix`, etc.)
- `depends_on` — from numeric ordering + DoD references
  - Tasks requiring `MockProvider` get `"1.17"` as dependency
- `evaluator_required` — true when tier is `cli-runtime`/`tui`, crosses crates, or touches public protocol/security

Rows without a DoD → deferred spec rows (not executable), unless an imported draft supplies a concrete DoD.

### A.init.3 Task-Graph Review Gate

Render the complete draft as a table:

| id | title | tier | commit_type | depends_on | exec_order | evaluator_required | inference_notes |
|---|---|---|---|---|---|---|---|

Present gate options:
- `confirm-all` — accept the graph as shown
- `edit-task <id>` — modify one task's inferred fields
- `apply-rule <selector> <field> <value>` — batch edit (show before/after diff)
- `export-draft` — write `.opi-impl-state.draft.json` for manual editing
- `import-draft` — validate and load from `.opi-impl-state.draft.json`
- `abort` — cancel initialization

Every edit or import re-renders the table before confirmation. The skill MUST NOT proceed until the whole graph is confirmed.

### A.init.4 Write Ledger

Write `.opi-impl-state.json` atomically (via tmp + rename). Add to `.gitignore` if missing:
- `.opi-impl-state.json`
- `.opi-impl-state.json.tmp`
- `.opi-impl-state.draft.json`

### A.init.5 Write Smoke Script

Ensure `scripts/opi-impl-smoke.sh` and `scripts/opi-impl-smoke.ps1` exist and contain the tracked templates from this plan. If they are missing, recreate them from the Smoke Script sections. If they already exist, leave them unchanged unless the template version changed.

### A.init.6 Commit Tracked Files

Commit ONLY tracked files that actually changed (smoke scripts + .gitignore update). The ledger is NOT committed. If no tracked file changed, do not create an empty commit.

```bash
git add scripts/opi-impl-smoke.sh scripts/opi-impl-smoke.ps1 .gitignore
git commit -m "chore: bootstrap opi-implement ledger and smoke"
```

### A.init.7 Print Summary

Print success with next-task hint: "Initialized N tasks. Next unblocked: <id> <title>"

## Reinit Reconciliation

When `--reinit` runs against an existing ledger:

### Step 1: Drift Check

Recompute `spec_sha256`. If unchanged, refuse — suggest `--status` instead.

### Step 2: Re-parse

Re-parse the spec into a fresh ledger using the same logic as A.init.2.

### Step 3: Reconcile Field-by-Field

- **Task IDs in both old and new:** Preserve runtime fields (`status`, `verified_at_commit`, `iteration_count`, `session_notes`, `blocker`).
- **Task IDs only in old:** Warn, ask "keep history, mark `archived`?"
- **Task IDs only in new:** Add with status `failing`.
- **DoD changed for existing passing task:** Warn, ask:
  - Preserve as `passing` (wording change is cosmetic), OR
  - Demote to `failing` (DoD substantively widened)
- **`depends_on`, `tier`, `commit_type`, or `evaluator_required` changed:** Re-run task-graph review gate with row-level diff. Require confirmation.

### Step 4: Finalize

Update `spec_sha256`. If tracked files changed (.gitignore or smoke scripts), commit:
```bash
git commit -m "chore: reconcile opi-implement harness files with opi-spec.md changes"
```
If no tracked file changed, do not create an empty commit.

## Phase B: Plan-the-Task

### B.1 Print Task Summary

Display:
- Task ID and title
- Definition of Done (verbatim)
- Verification tier and gate list
- Parallelize plan (if non-empty)
- Dependencies (all must be `passing`)

### B.2 User Gate

Ask: "Proceed with task `<id>` — `<title>`?"

If the user declines, exit cleanly without modifying state.

### B.3 Mark In-Progress

On confirmation:
1. Record `start_commit` = current HEAD SHA
2. Set `status` → `in_progress`
3. Initialize `last_attempt` = `{attempt: 1, started_at: <now>, ended_at: null, outcome: null, failing_gate: null, touched_files: []}`
4. Write ledger atomically

Phase A task selection alone does NOT mutate task status. Only Phase B confirmation triggers the state change.

## Phase C: Implement

### C.1 TDD Loop

Announce: "Using superpowers:test-driven-development to drive red-green for task <id>"

Invoke `superpowers:test-driven-development` with the task's DoD as the requirement.

If `parallelize` is non-empty, announce: "Using superpowers:dispatching-parallel-agents for sub-units: <list>"
- Sub-agents work on disjoint files, do NOT create commits
- Parent applies results in ledger order
- Runs full verification after each merge
- Conflicts or overlapping edits fail the attempt

### C.2 Iteration Cap (3 attempts)

On the 3rd consecutive failure to reach green:
- Announce: "Using superpowers:systematic-debugging — implementation stuck after 3 attempts"
- Invoke `superpowers:systematic-debugging` with the failing test output

### C.3 Total Cap (5 attempts)

On reaching `max_iterations` (default 5):
- Jump to §Failure Decision Gate

Each attempt boundary updates `last_attempt` in the ledger:
- `attempt` number
- `started_at` / `ended_at` timestamps
- `outcome`: `pass` or `fail`
- `failing_gate`: which verification gate failed
- `touched_files`: list of files modified

## Phase D: Verify

Run tier-specific gates, then cross-cutting gates. If any fail → back to Phase C.

### D.1 Tier: `workspace`

Tasks whose crate is `workspace` (e.g., 1.0, 1.17).

```bash
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
bash scripts/opi-impl-smoke.sh
```

### D.2 Tier: `library`

Tasks in `opi-ai`, `opi-agent` internals (e.g., 1.1–1.8).

Gates:
1. TDD produced new/changed tests: inspect `git diff --stat <start_commit> -- crates/<crate>` for test files, `#[test]`, async test attributes, or changed assertions.
2. `cargo test -p <crate>` green
3. `cargo clippy -p <crate> -- -D warnings` green
4. `cargo doc -p <crate> -- -D warnings` green
5. `cargo build --workspace` green (catches breaking API changes)
6. No `unwrap`/`expect` in non-test code: `grep -rn "unwrap\(\)\|expect(" crates/<crate>/src/ --include="*.rs"` must return empty (allow-list via `.opi-impl-allow-unwrap` if needed)

### D.3 Tier: `cli-tool`

Tasks: 1.9, 1.10 (filesystem tools).

All `library` gates above, plus:
- Behavioral tests in `crates/opi-coding-agent/tests/` using `tempfile` for real filesystem ops
- For `bash` tool: tests for timeout, cwd capture, cancellation
- For mutating tools: test asserting Phase-1 safety boundary is reported before execution

### D.4 Tier: `cli-runtime`

Tasks: 1.11, 1.14, 1.15, 1.16.

All `library` gates plus:
- E2E test booting `MockProvider` and running `opi` binary in subprocess
- Assertions on stdout, stderr, exit code

**MockProvider precondition:** Grep `crates/opi-ai/src/test_support.rs` for `MockProvider` symbol. If absent:
> "Task `<id>` depends on MockProvider scaffolding (task 1.17). Run task 1.17 first."

### D.5 Tier: `tui`

Tasks: 1.12, 1.13.

All `library` gates plus:
- Ratatui snapshot tests at 80×24 and 120×40 using `insta`
- Snapshot diffs require explicit user approval — never auto-accept

### D.6 Cross-Cutting Gates (every tier)

Run after tier-specific gates:

```bash
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
bash scripts/opi-impl-smoke.sh
```

Additional checks:
- `git status --porcelain --untracked-files=all` contains only intentional task files
- Before commit: `HEAD` must equal `tasks[].start_commit` (no intermediate manual commits)
- After commit: `git status --porcelain` must be clean; `HEAD^` must equal `start_commit`
- Commit message includes `Opi-*` evidence footers

### D.7 Risk Evaluator Gate

When `evaluator_required = true` for the task:

Announce: "Using superpowers:requesting-code-review for independent evaluation"

Evaluator receives: DoD, diff from `start_commit`, new/changed tests, verification outputs, planned commit message.

Must answer:
1. Does the diff satisfy the DoD without scope creep?
2. Do tests exercise behavior, not just implementation details?
3. Are there public API/protocol/security risks not covered by mechanical gates?
4. Is the evidence footer truthful and sufficient?

If evaluator fails → back to Phase C with findings as input. Generator may NOT self-approve.
