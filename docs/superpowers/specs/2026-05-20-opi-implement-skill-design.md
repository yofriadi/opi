# `opi-implement` Skill Design

> A long-running-agent harness, packaged as a single superpowers skill, that drives the implementation of `docs/opi-spec.md` one task at a time with TDD, tiered verification, and JSON-ledger checkpointing.

## 0. Document Control

| Field | Value |
|---|---|
| Status | Draft — pending approval |
| Spec version | 0.1-draft |
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
- PR creation, code review — manual.
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

Phase C: Implement
  C.1  Invoke superpowers:test-driven-development (red→green→refactor)
       └── if parallelize: → superpowers:dispatching-parallel-agents
  C.2  Iteration cap 3; on 3rd fail → invoke systematic-debugging
  C.3  Total cap 5; on cap hit → failure decision gate

Phase D: Verify
  D.1  Tier-specific gates (library / cli-tool / cli-runtime / tui / workspace)
  D.2  Cross-cutting gates: fmt, clippy -D warnings, cargo doc -D warnings
  D.3  Smoke re-run (phase-aware)
  D.4  If any fail → back to Phase C iteration

Phase E: Commit & ledger update
  E.1  Conventional commit (type derived from ledger commit_type field)
  E.2  Capture HEAD SHA → ledger.verified_at_commit
  E.3  Flip status to passing; append session_note
  E.4  No push (push is a separate human action)

Phase F: Phase-exit check
  F.1  If all tasks in current phase passing → print phase-complete report
  F.2  Else → print "next unblocked: X.Y" hint
```

### 4.1 Initializer Mode

When `.opi-impl-state.json` is absent, OR when `--reinit` is passed, phase A
is replaced by an extended initializer:

```text
Phase A.init:
  A.init.1  Pre-flight: confirm git clean, on main, opi-spec.md present
  A.init.2  Parse opi-spec.md §15 roadmap tables; for each task row, extract:
              - id, title, crate, DoD string, phase number
              - infer tier from crate + task description
              - infer commit_type from task verbs
              - infer depends_on from numeric ordering + DoD references
  A.init.3  Show inferred ledger to user as a rendered table.
            Gate: "Confirm task-graph inference? (y / edit-task / abort)"
            On 'edit-task': re-prompt for that row, redo A.init.3.
  A.init.4  Write .opi-impl-state.json atomically; add `.opi-impl-state.json`
            and `.opi-impl-state.json.tmp` to .gitignore if missing
  A.init.5  Write scripts/opi-impl-smoke.sh (.ps1 sibling on Windows)
  A.init.6  Commit ONLY the tracked files (smoke script + .gitignore update);
            the ledger itself is NOT committed since it is gitignored runtime
            state. Commit message:
              chore: bootstrap opi-implement ledger and smoke
  A.init.7  Print success summary with the next-task hint
```

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
   - `depends_on` changed: silently update — structural metadata.
4. Update `spec_sha256`. Commit:
   `chore: reconcile opi-implement ledger with opi-spec.md changes`.

## 5. JSON Ledger Schema

Path: `.opi-impl-state.json` at repository root. Gitignored — runtime artifact,
not source. Atomic writes via `.opi-impl-state.json.tmp` + rename.

```json
{
  "schema_version": 1,
  "spec_path": "docs/opi-spec.md",
  "spec_sha256": "<hash of opi-spec.md at last init/reinit>",
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
      "tier": "library",
      "commit_type": "feat",
      "parallelize": [],
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
      "verified_at_commit": null,
      "blocker": null,
      "session_notes": []
    }
  ],
  "phase_exit": {
    "1": { "completed_at": null, "exit_criteria_met": false }
  }
}
```

### 5.1 Field Semantics

| Field | Type | Mutability | Notes |
|---|---|---|---|
| `schema_version` | int | const | Bump when ledger format changes; skill refuses to operate on unknown versions. |
| `spec_path` | string | const | Default `docs/opi-spec.md`; override allowed in init for non-standard layouts. |
| `spec_sha256` | string | reinit-only | Drift detection. |
| `current_phase` | int | auto | Set to the lowest phase containing a non-`passing` task. |
| `tasks[].id` | string | const | Matches opi-spec.md §15 row id (`1.6`, `2.7`, etc.). |
| `tasks[].phase` | int | const | Derived from row's phase grouping. |
| `tasks[].title` | string | const | Spec row title, free text. |
| `tasks[].crate` | string | const | One of opi's five crates, or `workspace`. |
| `tasks[].definition_of_done` | string | const | Verbatim from spec; reinit may flag changes. |
| `tasks[].status` | enum | runtime | `failing` / `in_progress` / `passing` / `blocked`. |
| `tasks[].depends_on` | array | const | Other task IDs that must be `passing`. |
| `tasks[].tier` | enum | const | `workspace` / `library` / `cli-tool` / `cli-runtime` / `tui`. |
| `tasks[].commit_type` | enum | const | Conventional Commits type: `feat`/`fix`/`docs`/`refactor`/`test`/`chore`/`perf`. |
| `tasks[].parallelize` | array | const | Sub-unit names for `dispatching-parallel-agents`. Empty = sequential. |
| `tasks[].verification` | object | const | Tier-specific gate spec. |
| `tasks[].iteration_count` | int | runtime | Attempts since first `in_progress` flip. Reset on success. |
| `tasks[].max_iterations` | int | const | Default 5; per-task override allowed. |
| `tasks[].verified_at_commit` | string | runtime | Set in Phase E.2 on success. |
| `tasks[].blocker` | string | runtime | Populated when `status = blocked`. |
| `tasks[].session_notes` | array | runtime | Append-only `{timestamp, attempt, summary, gate_results}`. Short. |
| `phase_exit[N]` | object | runtime | `completed_at` ISO-8601 + `exit_criteria_met` boolean. |

### 5.2 Atomic Write Protocol

The ledger is written ONLY at three points per invocation:

1. End of Phase A: mark target task `in_progress`.
2. End of Phase E: mark task `passing`, record commit, append note.
3. Failure decision gate: mark `blocked` (or leave unchanged if user picks
   retry/abandon).

Write sequence:
```bash
echo "$json" > .opi-impl-state.json.tmp
# fsync via mv (POSIX rename is atomic on same filesystem)
mv -f .opi-impl-state.json.tmp .opi-impl-state.json
```

On Windows the equivalent is `Move-Item -Force` which is atomic within the
same volume.

### 5.3 Interrupt Recovery

On next invocation, if a task has `status = in_progress` AND
`verified_at_commit = null` AND the working tree is clean, prompt:

> "Task X was marked `in_progress` but no commit was recorded. Was the prior
> session interrupted? Reset to `failing` and retry, or investigate first?"

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
- TDD red→green produced new test files in `crates/<crate>/tests/` OR
  `#[cfg(test)]` modules. (Verified by `git diff --stat HEAD~1` after impl —
  must show at least one test file changed.)
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
The initializer's inference (§4.1 A.init.2) MUST add `"1.17"` to the
`depends_on` array of every `cli-runtime`-tier task. The auto-pick rule in
§4 then naturally promotes 1.17 ahead of 1.14/1.15 even though the spec
lists 1.17 last. The numeric ID is preserved (it is the immutable spec
anchor); only execution order is reshaped. A `session_notes` entry at init
time records this reordering for the human reviewer.

### 6.5 `tui`

Tasks: 1.12 (TUI shell), 1.13 (markdown/code rendering).

Gates: `library` gates plus:
- Ratatui snapshot tests at fixed sizes (80×24 and 120×40). Snapshots use
  `insta` (or equivalent). Snapshot file diffs reviewed mechanically; the
  skill refuses to auto-accept snapshot changes — they require explicit user
  approval in Phase B.

### 6.6 Cross-Cutting Gates (every tier)

Run after the tier-specific gates:

- `bash scripts/opi-impl-smoke.sh` exits 0.
- `git status --porcelain` is empty before the commit-stage (no leftover
  untracked) AND clean after the commit (commit captured everything).
- HEAD's parent commit equals the previous task's `verified_at_commit`
  (chain integrity — flags mid-implementation rebases).

### 6.7 Phase 2/3/4 Tiers

This document covers Phase 1 tiers in detail. Phase 2 will introduce:
- `session-storage` tier (round-trip tests, fuzz harness).
- `provider-contract` tier (SSE fixture tests for each new provider).
- `json-contract` tier (NDJSON schema tests, line framing).

Phase 2 tier definitions SHOULD be added to this document at `--reinit` time
when the corresponding tasks are first encountered, NOT pre-defined here.

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
```

### 7.2 Options

| Option | Effect |
|---|---|
| (a) Retry with extended cap | Adds 5 attempts (total budget 10). Status stays `in_progress`. |
| (b) Escalate to design | Invokes `superpowers:brainstorming` on the DoD interpretation. User may amend `opi-spec.md` and `--reinit`. |
| (c) Mark blocked | Records blocker text. Leaves failing tests in place. Stages no changes. Status → `blocked`. Skill will skip on `auto` selection until cleared via `--clear-blocker`. |
| (d) Auto-revert and abandon | `git restore .` + `git clean -fd` on untracked. Status stays `failing`. No blocker recorded. Last resort. |
| (e) Drop to manual session | Prints exact reproduction commands and exits. User finishes manually, then `--resume-from-manual` skips to Phase D verify. |

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

The skill MUST refuse to act if any of these rules would be violated, even
if the user requests it during an interactive failure-decision gate.

## 9. Composition With Existing Skills

The skill explicitly INVOKES these via the `Skill` tool — it never
re-implements them.

| Phase | Invokes | Purpose |
|---|---|---|
| C.1 | `superpowers:test-driven-development` | red→green→refactor body |
| C.1 (when `parallelize` non-empty) | `superpowers:dispatching-parallel-agents` | many-brains for independent sub-units |
| C.2 (attempt 3+) | `superpowers:systematic-debugging` | when implementation can't reach green |
| D pre-commit | `superpowers:verification-before-completion` | enforce evidence-before-claim |
| Failure gate (b) | `superpowers:brainstorming` | when DoD interpretation is ambiguous |
| Phase F (report only) | `opi-release` | mentioned in phase-exit report; never auto-invoked |

Each invocation announces itself per the using-superpowers contract:
`"Using superpowers:test-driven-development to drive red-green for task 1.6"`.

## 10. Skill Argument Surface

```text
/skill opi-implement                                  # auto-pick lowest-ID unblocked failing task
/skill opi-implement <task-id>                        # specific task; validates deps, refuses if blocked
/skill opi-implement --status                         # print ledger summary, exit
/skill opi-implement --reinit                         # re-parse spec, reconcile ledger
/skill opi-implement <task-id> --resume-from-manual   # skip Phase C, jump to Phase D verify
/skill opi-implement <task-id> --extend-cap <N>       # raise iteration cap for this invocation only
/skill opi-implement --clear-blocker <task-id>        # remove blocker text, status → failing
```

`<task-id>` matches the ID format used in opi-spec §15 (e.g., `1.6`, `2.7`).

## 11. Files Created or Touched

| Path | Owner | Tracked |
|---|---|---|
| `.claude/skills/opi-implement/skill.md` | this skill | yes |
| `.opi-impl-state.json` | runtime | NO (gitignored) |
| `.opi-impl-state.json.tmp` | runtime | NO (gitignored) |
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
| `jq` | preferred | Ledger parsing in bash blocks. On Windows without `jq`, the skill uses a small Rust helper instead. |
| `sha256sum` | yes | For `spec_sha256` drift detection. PowerShell `Get-FileHash` equivalent on Windows. |
| POSIX `sh` | yes (Linux/macOS) | Runs `scripts/opi-impl-smoke.sh`. |
| PowerShell | yes (Windows) | Runs `scripts/opi-impl-smoke.ps1`. |
| `gh` CLI | NO | Never required by this skill. Release-related actions belong to `opi-release`. |

The skill detects host via `OSTYPE`/`OS` env vars and chooses the smoke
script variant. Bash-on-Windows (as per the CLAUDE.md project shell) uses the
POSIX `.sh` script with forward-slash paths.

## 13. Mapping to Anthropic Harness Articles

| Article principle | Skill mechanism |
|---|---|
| Shift-handover model | One task per invocation; ledger is the handover artifact. |
| JSON ledger, not Markdown | `.opi-impl-state.json` is mutable; opi-spec.md is immutable. |
| Boot-time smoke catches prior breakage | Phase A.3 runs `scripts/opi-impl-smoke.sh` before any task work. |
| Generator/evaluator separation | TDD provides this — tests are the evaluator, impl is the generator. |
| Test the running app, not artifacts | `cli-runtime` tier runs the binary as a subprocess. |
| Decouple brain/hands/session | Brain = Claude + skill prompt; hands = cargo/git/sub-skills; session = ledger + git history. |
| Append-only durable session log | git history + `session_notes` array (append-only). |
| Iteration caps | 3-attempt impl, then `systematic-debugging`, total cap 5. |
| Re-examine harness on each model release | `schema_version` field + three-consecutive-failure meta-warning. |
| Anti-pattern: trust agent to grade itself | Tiered gates are mechanical, not LLM-graded. |
| Anti-pattern: edit tests to pass | Explicit prompt rule against test deletion/weakening. |
| Anti-pattern: irreversible compaction | Ledger session_notes are append-only; status is a finite state machine. |
| Anti-pattern: bake infrastructure assumptions into the harness | Smoke script is phase-aware and regenerated at phase-exit boundaries. |

## 14. Open Questions Deferred to Implementation

These were intentionally left open during brainstorming and will be resolved
in the implementation plan:

1. **Exact heuristics for tier inference** during init. The mapping table in
   §4.1 is conservative; edge cases (multi-crate tasks, tasks that span both
   library and tool layers) need explicit handling in the initializer's
   parser.
2. **Snapshot library choice** for the `tui` tier (`insta` vs. `expect-test`
   vs. ratatui's own test helpers). Decided when task 1.12 begins.
3. **MockProvider exact shape** — owned by spec task 1.17, not this skill.
   The skill only checks for its presence.
4. **Whether `--clear-blocker` should require a justification string** logged
   into `session_notes`. Default: yes. Decided in implementation.

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
  `brainstorming`, `writing-plans`, `executing-plans`,
  `subagent-driven-development`, `finishing-a-development-branch`
