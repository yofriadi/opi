# Initializer & Reinit Reference

## Init Mode (A.init)

Triggered when `.opi-impl-state.json` is absent OR `--reinit` is passed.

### A.init.1 Pre-flight
- Record current branch, baseline dirty files, and `opi-spec.md` presence.
  Refuse only when dirty files would be overwritten by init/reinit outputs;
  do not require unrelated user changes to be cleaned.

### A.init.2 Parse Spec
Parse `opi-spec.md` §15 roadmap tables. For each task row extract:
- id, title, crate, DoD (when present), phase number
- **Infer:** tier (from crate + description), commit_type (from task verbs),
  depends_on (from ordering + DoD references), evaluator_required (from risk rules)
- Attach `inference_notes` for every non-verbatim field
- Rows without explicit DoD:
  - Phase 1 rows with a "Definition of done" column use that text verbatim.
  - Phase 2+ rows may receive a draft `definition_of_done` inferred from the
    roadmap row, feature parity matrix, relevant crate section, security
    requirements, and phase exit criteria.
  - Every inferred DoD MUST include `inference_notes` with source section names.
  - The task remains non-executable until the task-graph review gate confirms
    the inferred DoD.

### A.init.2b Supplemental Phase 5 Task Source

After Phase 4 exits, Phase 5 productized extension/package tasks are not
derived from arbitrary roadmap rows. They are derived from these reviewed
sources:

- `docs/superpowers/specs/2026-06-08-productized-extensions-package-ecosystem-design.md`
- `docs/superpowers/plans/2026-06-08-productized-extensions-package-ecosystem.md`

When either source exists, the initializer MAY add Phase 5 tasks from the
implementation plan's task list, but it MUST include both paths in
`spec_files` and MUST record SHA-256 hashes for both in `spec_files_sha256`.
Do not auto-scan `docs/superpowers/specs/` or `docs/superpowers/plans/`.

Phase 5 task IDs use `5.<N>` in plan order. Task titles, DoDs, owned paths,
verification gates, and dependencies come from the reviewed implementation
plan, not from inferred prose. If the plan and design spec contradict each
other, stop and ask for a revised design/plan before writing the ledger.

### A.init.2a Composite Row Detection

Some spec roadmap rows describe N independent deliverables in one line
(e.g. `4.6 | extension examples: permission gate, sub-agent, plan mode, todo, MCP adapter`).
These rows MUST NOT become a single ledger task.

Trigger heuristic: a roadmap row is composite when any of these is true:

- the row title contains `:` followed by at least two comma-separated items;
- the row title begins with `examples:` or `task family:`;
- the row title is a Phase 4 resource-family row listing at least three independent resource nouns joined by commas or `and`, such as `skills, prompt fragments, themes, and packages`;
- the row's crate column is an open packaging identifier such as `examples / package template` and the title lists at least two deliverables.

Do not split a row merely because the DoD contains commas. The split decision is based on the roadmap row title and crate column.

For each composite row:

- Generate sub-tasks with IDs `<row>.1`, `<row>.2`, ..., `<row>.N`.
- Set `parent_spec_row = "<row>"` on each.
- Independent draft DoD per item, drawn from the item phrase plus relevant
  spec sections.
- Each sub-task inherits the parent's `depends_on` unless review-gate narrows.
- Each sub-task inherits the parent's `crate` (or `"package-template"` /
  `"examples"` when the row's crate column is non-standard).
- `definition_source = "inferred"` for every sub-task (composite rows never
  produce verbatim DoDs).
- The composite row itself does NOT produce a parent task; there is no
  placeholder entry with id `<row>`.
- The task-graph review gate MUST surface composite decompositions in a
  dedicated section so the user reviews them as a unit before confirmation.

Phase 4 examples:

- `4.7 | skills, prompt fragments, themes, and packages with progressive discovery` becomes `4.7.1` skills, `4.7.2` prompt fragments/templates, `4.7.3` themes, and `4.7.4` packages.
- `4.8 | extension/package examples: permission gate, protected paths, sub-agent, plan mode, todo, MCP adapter` becomes six package/example tasks; the parent row is not executable.

### A.init.3 Task-Graph Review Gate

Render complete draft as table with: id, title, tier, `task_owned_paths`
(default derived from `crate`, editable), commit_type, depends_on,
execution order, evaluator_required, inference_notes.

Gate options:
- **confirm-all** — accept the graph as shown
- **edit-task `<id>`** — modify one task's inferred fields
- **apply-rule `<selector>` `<field>` `<value>`** — batch edit (show before/after diff)
- **export-draft** — write `.opi-impl-state.draft.json` for human editing
- **import-draft** — validate schema, uniqueness, deps, cycles, tiers; re-render
- **abort** — stop without writing

Every edit or import re-renders before confirmation.
REFUSE to proceed until whole graph is confirmed.
MUST NOT silently apply inferred changes.

### A.init.4 Write Ledger
- Write `.opi-impl-state.json` atomically
- Add `.opi-impl-state.json`, `.opi-impl-state.json.tmp`,
  `.opi-impl-state.draft.json` to `.gitignore` if missing

### A.init.5 Write Smoke Script
- `scripts/opi-impl-smoke.sh` (+ `.ps1` sibling on Windows)

### A.init.6 Commit

**Note:** This is the only git mutation outside Phase E. It commits harness
infrastructure (smoke script + .gitignore), not task implementation code.

- Commit ONLY tracked files (smoke + .gitignore update)
- Ledger is NOT committed (gitignored runtime state)
- Message: `chore: bootstrap opi-implement ledger and smoke`

### A.init.7 Print Summary
- Success message + next-task hint

## Schema Version Migration

On every invocation (not just `--reinit`), inspect `schema_version` from the
ledger before any other step.

- `schema_version == 2` (current): proceed.
- `schema_version == 1`: refuse normal task execution. Print
  "Ledger is v1; running v1 → v2 migration as part of `--reinit`." If the
  invocation is not `--reinit`, exit and instruct the user to run
  `opi-implement --reinit`. If `--reinit`: apply the v1 → v2 migration
  documented in `ledger-schema.md`, then continue with the rest of reinit
  reconciliation below.
- `schema_version > 2` or missing: refuse with an explicit message identifying
  the offending value.

## Reinit Reconciliation

When `--reinit` runs against an existing ledger:

1. For each path in `spec_files`, recompute its SHA-256 and compare with
   `spec_files_sha256`. If every entry matches → refuse, suggest `--status`.
   If any differs → proceed.
2. Re-parse spec into fresh ledger.
3. Reconcile field-by-field:
   - **Both:** preserve `status`, `verified_at_commit`, `iteration_count`,
     `session_notes`, `blocker`
   - **Only in old:** warn, ask "keep history, mark `archived`?"
   - **Only in new:** add with `status: failing`
   - **DoD changed for passing task:** warn, ask preserve-as-passing (cosmetic)
     or demote-to-failing (substantive)
   - **depends_on/tier/commit_type/evaluator_required changed:** re-run
     task-graph review gate with row-level diff, require confirmation
4. Update every entry in `spec_files_sha256` to the freshly recomputed hash
   after confirmation.
5. If tracked files changed (.gitignore, smoke): commit with
   `chore: reconcile opi-implement harness files with opi-spec.md changes`
6. If no tracked file changed: no empty commit. Ledger/draft remain gitignored.

### Changed Task Meaning

If a task ID is present in both old and new graphs but the title or DoD changes
substantively, show a row-level diff and default to:

- preserve runtime history in `session_notes`;
- keep `status = failing` unless the user explicitly confirms the old passing
  evidence still satisfies the new DoD;
- record an `inference_notes` entry with `field = "replaces"` when the new task
  intentionally supersedes an old one under the same ID.

Examples from the 2026-05-25 spec adjustment:

- `3.7 OPI.md context loading` becomes `3.7 AGENTS.md / CLAUDE.md context loading`;
- `3.8 permission profiles and policy system` becomes `3.8 pi-style tool selection and safety hooks`;
- `3.9 MCP client adapter` becomes `3.9 find / ls built-in tool parity`;
- MCP moves to Phase 4 as an extension/package example, not a Phase 3 core task.

## Draft Export/Import

**export-draft:** Writes `.opi-impl-state.draft.json` (gitignored scratch).

**import-draft:** Validates:
- Schema version
- Task ID uniqueness
- Dependency references exist
- No cycles
- Known tier names

Import never counts as confirmation by itself. If a draft promotes a deferred
row into executable, it must supply `definition_of_done` + inference note.

## apply-rule Examples

Batch graph edits for tedious one-by-one changes:
- Add `1.17` as dep to every task using `MockProvider`
- Change all `opi-tui` rows to tier `tui`
- Mark public-protocol rows as `evaluator_required = true`

Always show before/after diff for affected rows, then return to A.init.3.
