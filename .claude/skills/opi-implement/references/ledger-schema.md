# Ledger Schema Reference

Path: `.opi-impl-state.json` at repository root. Gitignored runtime artifact.
Atomic writes via `.opi-impl-state.json.tmp` + rename.

## Schema

```json
{
  "schema_version": 2,
  "spec_files": ["docs/opi-spec.md"],
  "spec_files_sha256": {
    "docs/opi-spec.md": "<hash at last init/reinit>"
  },
  "task_graph_confirmed_at": "2026-05-20T09:30:00Z",
  "current_phase": 1,
  "tasks": [
    {
      "id": "1.6",
      "phase": 1,
      "title": "agent_loop",
      "crate": "opi-agent",
      "definition_of_done": "mock tests cover no-tool and tool-use turns",
      "definition_source": "verbatim",
      "replaces": null,
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
          "RUSTDOCFLAGS=\"-D warnings\" cargo doc -p opi-agent --no-deps"
        ],
        "behavioral_tests": ["crates/opi-agent/tests/agent_loop_mock.rs"],
        "snapshot_tests": [],
        "smoke_addendum": null
      },
      "iteration_count": 0,
      "max_iterations": 5,
      "start_commit": null,
      "baseline_dirty_files": [],
      "task_owned_paths": ["crates/opi-agent/**", "Cargo.toml"],
      "last_attempt": null,
      "verified_at_commit": null,
      "evidence": null,
      "blocker": null,
      "session_notes": []
    }
  ],
  "phase_exit": {
    "1": {
      "completed_at": "2026-04-12T18:00:00Z",
      "exit_criteria_met": true,
      "evaluator_summary": "all Phase 1 exit criteria met; see commit 4d9c64...",
      "snapshot_path": "docs/snapshots/phase1/opi-impl-state.json",
      "task_summary": [
        { "id": "1.0", "title": "introduce Phase 1 dependencies", "status": "passing", "verified_at_commit": "4d9c64..." }
      ]
    }
  }
}
```

## Field Semantics

| Field | Type | Mutability | Notes |
|---|---|---|---|
| `schema_version` | int | reinit-only | Current value `2`. v2 adds `task_owned_paths`, `definition_source`, `replaces`, `baseline_dirty_files`, `spec_files`, `spec_files_sha256`, `phase_exit[N].snapshot_path`, `phase_exit[N].task_summary`, dotted sub-task IDs, and open-string `crate` values. Reading a v1 ledger requires explicit reinit-time migration; refuse unknown versions. |
| `spec_files` | array | const-on-init, reinit-editable | Normative spec file paths whose drift triggers reinit refusal. Default `["docs/opi-spec.md"]`. Adding or removing a path requires `--reinit`. |
| `spec_files_sha256` | object | reinit-only | Map of file path → SHA-256 hash at last init/reinit. Each entry is checked independently; any mismatch triggers the spec-alignment guard. |
| `task_graph_confirmed_at` | string/null | init/reinit | ISO-8601 confirmation time |
| `current_phase` | int | auto | Lowest phase with non-`passing` task |
| `tasks[].id` | string | const | Matches a row in `opi-spec.md` §15 OR a sub-task expansion. Pattern: `^\d+\.\d+(\.\d+)?$`. Sub-task IDs carry a third component (e.g. `4.6.1`) and MUST also set `parent_spec_row`. |
| `tasks[].phase` | int | const | From row's phase grouping |
| `tasks[].title` | string | const | Spec row title |
| `tasks[].crate` | string | const | One of opi's five crates, `workspace`, or any free-string identifier (e.g. `examples`, `package-template`) when the spec row uses an open identifier. Review-gate warns for unknown values but does not refuse. |
| `tasks[].parent_spec_row` | string/null | const | Source spec row ID when this task is a sub-task expansion (e.g. `"4.6"` for `4.6.1`). `null` for direct spec rows. |
| `tasks[].definition_of_done` | string | const | Verbatim from spec |
| `tasks[].definition_source` | enum | const | `verbatim`, `inferred`, or `draft-reviewed`; inferred values require review gate confirmation |
| `tasks[].replaces` | string/null | const | Prior task title/meaning superseded during reinit, when the same task ID was repurposed by spec changes |
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
| `tasks[].baseline_dirty_files` | array | runtime | Files already dirty at Phase B start; used to avoid cleaning or staging unrelated user work |
| `tasks[].task_owned_paths` | array | const-at-Phase-B, append-only during Phase C | Glob patterns the task is allowed to modify. Default derived from `crate` at init/reinit time (e.g. `crate = "opi-agent"` → `["crates/opi-agent/**", "Cargo.toml"]`). Phase C MAY append entries when implementation requires touching outside-prefix files; each append MUST add an `inference_notes` entry with `field = "task_owned_paths"` and a `reason`, written via the atomic ledger write. |
| `tasks[].last_attempt` | object/null | runtime | `{attempt, started_at, ended_at, outcome, failing_gate, touched_files}` |
| `tasks[].verified_at_commit` | string | runtime | Set in Phase E.2 |
| `tasks[].evidence` | object/null | runtime | Mirror of `Opi-*` commit footers |
| `tasks[].blocker` | string | runtime | Populated when `status = blocked` |
| `tasks[].session_notes` | array | runtime | Append-only `{timestamp, attempt, summary, gate_results}` |
| `phase_exit[N]` | object | runtime | `completed_at` + `exit_criteria_met` + evaluator summary |
| `phase_exit[N].snapshot_path` | string/null | runtime | Path to a committed full-ledger snapshot at the moment phase `N` exited. `null` while the phase is incomplete. Written under `docs/snapshots/phase<N>/`. |
| `phase_exit[N].task_summary` | array | runtime | `[{id, title, status, verified_at_commit}]` for every task that belonged to phase `N` at exit time. Lets `--status` report completed phases without reading the snapshot file. |

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

## v1 → v2 Migration

Reinit MUST run this migration before any reconciliation when it loads a
ledger with `schema_version < 2`. The migration is applied to a draft copy;
nothing is overwritten until the task-graph review gate confirms.

Per-task rules:

- `definition_source`: compute by re-parsing the spec roadmap row.
  - Spec row has explicit DoD column AND the v1 DoD string matches → `"verbatim"`.
  - Spec row has no DoD column AND the v1 DoD's SHA-256 matches the
    `Opi-DoD-SHA256` footer of the task's `verified_at_commit` → `"draft-reviewed"`
    (work shipped under that DoD; preserve).
  - Anything else → `"inferred"` AND demote `status` to `failing` AND require
    re-confirmation at the task-graph review gate.
- `replaces`: when the same task ID exists in v1 ledger and current spec but
  title or DoD changed substantively (string distance beyond trivial whitespace
  or punctuation), fill with the v1 title. Else `null`.
- `baseline_dirty_files`: always `[]` at migration time; populated fresh at the
  next Phase B.
- `task_owned_paths`: derive from `tasks[].crate` per the rules in the field
  definition (e.g. `crate = "opi-agent"` → `["crates/opi-agent/**", "Cargo.toml"]`).

Top-level field rules:

- `spec_sha256` (v1 string) → `spec_files` = `["docs/opi-spec.md"]` AND
  `spec_files_sha256` = `{"docs/opi-spec.md": <existing-hash>}`. Delete the
  v1 `spec_sha256` key.
- `phase{N}_summary` (v1 informal top-level array) → `phase_exit["<N>"].task_summary`.
  Delete the v1 top-level keys.
- `phase{N}_snapshot` (v1 informal top-level string) → `phase_exit["<N>"].snapshot_path`.
  Delete the v1 top-level keys.

After successful migration AND task-graph review gate confirmation, write
`schema_version: 2` via the atomic write protocol.

## Interrupt Recovery

On invocation, if a task has `status = in_progress` AND `verified_at_commit = null`:

**No task-owned dirty files beyond `baseline_dirty_files`:** Prompt:
> "Task X was marked `in_progress` but no commit was recorded. Was the prior
> session interrupted? Reset to `failing` and retry, or investigate first?"

**Task-owned dirty files present:** MUST NOT reset/restore/clean/discard. Print:
- `start_commit`
- `baseline_dirty_files`
- `git status --short`
- Task-owned files changed since `start_commit`
- Last failing gate + reproduction commands

Offer only: continue investigation, mark blocked with text, or drop to manual.
