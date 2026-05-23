# Ledger Schema Reference

Path: `.opi-impl-state.json` at repository root. Gitignored runtime artifact.
Atomic writes via `.opi-impl-state.json.tmp` + rename.

## Schema

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
    "1": { "completed_at": null, "exit_criteria_met": false, "evaluator_summary": null }
  }
}
```

## Field Semantics

| Field | Type | Mutability | Notes |
|---|---|---|---|
| `schema_version` | int | const | Bump on format change; refuse unknown versions |
| `spec_path` | string | const | Default `docs/opi-spec.md` |
| `spec_sha256` | string | reinit-only | Drift detection |
| `task_graph_confirmed_at` | string/null | init/reinit | ISO-8601 confirmation time |
| `current_phase` | int | auto | Lowest phase with non-`passing` task |
| `tasks[].id` | string | const | Matches opi-spec.md §15 row id |
| `tasks[].phase` | int | const | From row's phase grouping |
| `tasks[].title` | string | const | Spec row title |
| `tasks[].crate` | string | const | One of opi's five crates, or `workspace` |
| `tasks[].definition_of_done` | string | const | Verbatim from spec |
| `tasks[].status` | enum | runtime | `failing`/`in_progress`/`passing`/`blocked`/`archived` |
| `tasks[].depends_on` | array | const | Task IDs that must be `passing` |
| `tasks[].inference_notes` | array | const | Reasons for inferred fields |
| `tasks[].tier` | enum | const | `workspace`/`library`/`cli-tool`/`cli-runtime`/`tui` |
| `tasks[].commit_type` | enum | const | `feat`/`fix`/`docs`/`refactor`/`test`/`chore`/`perf` |
| `tasks[].parallelize` | array | const | Sub-unit names for parallel dispatch |
| `tasks[].evaluator_required` | bool | const | Static risk flag |
| `tasks[].verification` | object | const | Tier-specific gate spec |
| `tasks[].iteration_count` | int | runtime | Attempts since `in_progress` |
| `tasks[].max_iterations` | int | const | Default 5 |
| `tasks[].start_commit` | string/null | runtime | HEAD when Phase B confirms |
| `tasks[].last_attempt` | object/null | runtime | `{attempt, started_at, ended_at, outcome, failing_gate, touched_files}` |
| `tasks[].verified_at_commit` | string | runtime | Set in Phase E.2 |
| `tasks[].evidence` | object/null | runtime | Mirror of `Opi-*` commit footers |
| `tasks[].blocker` | string | runtime | Populated when `status = blocked` |
| `tasks[].session_notes` | array | runtime | Append-only `{timestamp, attempt, summary, gate_results}` |
| `phase_exit[N]` | object | runtime | `completed_at` + `exit_criteria_met` + evaluator summary |

## Durable Evidence Contract

The ledger is mutable runtime state (gitignored). Every successful task commit
MUST include parseable footers:

```text
Opi-Task: <id>
Opi-DoD-SHA256: <sha256 of definition_of_done>
Opi-Verification: <tier>; <short command/result summary>
Opi-Evaluator: <not-required | passed>
```

These values are also copied into `tasks[].evidence`. A fresh clone without the
ledger can reconstruct completion status via `git log --grep "Opi-Task:"`.

## Atomic Write Protocol

1. Serialize full JSON with a structured writer (not shell echo/string concat).
2. Write to `.opi-impl-state.json.tmp` in repo root.
3. Flush file; fsync parent directory when platform exposes it.
4. Rename `.opi-impl-state.json.tmp` over `.opi-impl-state.json`.
5. On failure, leave previous ledger intact; print tmp path for inspection.

**Write boundaries** (the only times the ledger is written):
- End of Phase B (user confirms): mark `in_progress`, record `start_commit`
- Each attempt boundary: record start, failing gate, touched files
- Failure decision gate: mark `blocked`, extend cap, or record handoff
- End of Phase E: mark `passing`, record commit + evidence
- Reinit after task-graph review gate confirmed

## Interrupt Recovery

On invocation, if a task has `status = in_progress` AND `verified_at_commit = null`:

**Clean working tree:** Prompt:
> "Task X was marked `in_progress` but no commit was recorded. Was the prior
> session interrupted? Reset to `failing` and retry, or investigate first?"

**Dirty working tree:** MUST NOT reset/restore/clean/discard. Print:
- `start_commit`
- `git status --short`
- Files changed since `start_commit`
- Last failing gate + reproduction commands

Offer only: continue investigation, mark blocked with text, or drop to manual.
