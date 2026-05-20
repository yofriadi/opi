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
