# Initializer & Reinit Reference

## Init Mode (A.init)

Triggered when `.opi-impl-state.json` is absent OR `--reinit` is passed.

### A.init.1 Pre-flight
- Confirm git clean, on main, `opi-spec.md` present

### A.init.2 Parse Spec
Parse `opi-spec.md` §15 roadmap tables. For each task row extract:
- id, title, crate, DoD (when present), phase number
- **Infer:** tier (from crate + description), commit_type (from task verbs),
  depends_on (from ordering + DoD references), evaluator_required (from risk rules)
- Attach `inference_notes` for every non-verbatim field
- Rows without DoD → deferred spec rows (not executable) unless a reviewed
  draft supplies a concrete DoD

### A.init.3 Task-Graph Review Gate

Render complete draft as table with: id, title, tier, commit_type, depends_on,
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

## Reinit Reconciliation

When `--reinit` runs against an existing ledger:

1. Recompute `spec_sha256`. If unchanged → refuse, suggest `--status`.
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
4. Update `spec_sha256` after confirmation.
5. If tracked files changed (.gitignore, smoke): commit with
   `chore: reconcile opi-implement harness files with opi-spec.md changes`
6. If no tracked file changed: no empty commit. Ledger/draft remain gitignored.

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
