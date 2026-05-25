# Opi Implement Skill Realignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Realign `.claude/skills/opi-implement/` with the current `docs/opi-spec.md` Phase 3/4 direction, removing stale MCP/permission/`OPI.md` assumptions while preserving the long-running harness model.

**Architecture:** Keep `opi-implement` as a narrow implementation harness: spec-derived ledger, reviewed task graph, one task per invocation, TDD-first execution, mechanical gates, independent evaluator where risk requires it. Update only the skill documents, references, pressure scenarios, and runtime ledger reconciliation guidance; do not change `docs/opi-spec.md` from this plan.

**Tech Stack:** Markdown skill files, JSON ledger contract, PowerShell/Unix shell verification commands, Cargo verification gates, Superpowers TDD/debug/review skills.

**Plan rounds:**

- **Round 1 (Tasks 1-7)** addresses the surface drift: stale Phase 3 titles, Phase-1-only verification tiers, ambiguous scope wording, shared-workspace cleanliness.
- **Round 2 (Tasks 8-13, with Task 7 extended)** addresses six data-model gaps surfaced by the 2026-05-25 grilling session. Several Round 1 steps are explicitly superseded by Round 2 tasks; affected steps are tagged inline.

---

## Findings This Plan Addresses

1. The skill body is still broadly valid, but its references are Phase 1/2 shaped: verification tiers name old task IDs and do not cover Phase 3 enterprise providers, multimodal work, context-file loading, tool selection, `find`/`ls`, proxy, or connection-pool work.
2. The current `.opi-impl-state.json` is stale versus `docs/opi-spec.md`: current spec hash is `7352d798d574cc1946879e164e3829ca36408325522971b16587ac54284fedd2`, while the ledger records `011cc486f32a60b3f967c911a369e091cca88dd20417dfdc5a0cb7fd60c8e597`.
3. The current ledger still contains old Phase 3 tasks: `3.7 OPI.md context loading`, `3.8 permission profiles and policy system`, and `3.9 MCP client adapter`. These now contradict the spec, which has `AGENTS.md`/`CLAUDE.md`, pi-style tool selection/safety hooks, and `find`/`ls`; MCP moved to Phase 4+ as an extension/package example.
4. `references/verification-tiers.md` has an invalid Rustdoc command for per-crate docs: `cargo doc -p <crate> -- -D warnings`. It must become an environment-specific `RUSTDOCFLAGS=-D warnings cargo doc -p <crate> --no-deps` wrapper.
5. The skill has clean-working-tree assumptions that conflict with this repository's shared-workspace rules. It should track a baseline dirty set and only require task-owned changes to be resolved/staged, not require unrelated user changes to disappear.
6. The scope boundary "Reading/writing `~/.config/opi/` or session storage" is ambiguous. It should forbid user runtime data paths, while allowing source-code changes to session modules when a selected spec task owns them.
7. The initializer treats roadmap rows without an explicit Definition of Done as deferred. That blocks Phase 3/4 because the current roadmap tables are task-only. The skill needs a reviewed "inferred DoD" path sourced from the roadmap row plus related spec sections and phase exit criteria.

### Round 2 findings (added 2026-05-25 after design review)

8. **Ledger schema version is not bumped** even though three semantically-loaded fields (`definition_source`, `replaces`, `baseline_dirty_files`) and several Round 2 additions are introduced. Without a v2 bump, an older harness reading a newer ledger silently defaults missing fields; reinit migration has no formal contract.
9. **The spec-alignment guard hard-codes "Phase 3+"**, which contradicts the ledger's own `current_phase` semantics (lowest phase with non-`passing` task). The boundary should be data-driven, not phase-number-literal, so demoted Phase 1/2 tasks are also protected when relevant.
10. **"Task-owned files" is undefined**. Round 1's verification-tier rewrite references the concept but the ledger has no field naming task ownership. Implicit "dirty minus baseline" creates two failure modes: user's mid-task unrelated edits get force-staged, or overlap with baseline file silently passes/fails.
11. **`phase1_summary` / `phase1_snapshot` / `phase2_summary`** live in the actual `.opi-impl-state.json` but are absent from the documented schema. Reinit, written naively, will delete them. The snapshot files under `docs/snapshots/phase{N}/` are real committed artifacts; the writer and reader of these fields is invisible.
12. **`spec_sha256` is a single value over a single file**. The repo has `docs/opi-spec.zh.md` (translation, not normative here) and future spec splits cannot be expressed without changing schema and skill code together.
13. **Phase 4 roadmap row 4.6 bundles five conceptually independent extension examples** (`permission gate`, `sub-agent`, `plan mode`, `todo`, `MCP adapter`). One ledger task per spec row makes 4.6 un-implementable: one DoD cannot describe five disjoint deliverables, one commit cannot evidence five independent extensions, the 5-iteration cap is too small, and the `crate` enum cannot represent `examples / package template`.

## File Structure

- Modify `.claude/skills/opi-implement/skill.md`: core flow, scope boundaries, spec-alignment warning, dirty-worktree rule, commit consent wording.
- Modify `.claude/skills/opi-implement/references/initializer.md`: reinit behavior for changed Phase 3/4 roadmaps, inferred DoD review, stale-ledger handling.
- Modify `.claude/skills/opi-implement/references/ledger-schema.md`: ledger metadata for spec hash, baseline dirty set, inferred DoD source notes, optional archived/replaced task links.
- Modify `.claude/skills/opi-implement/references/verification-tiers.md`: replace Phase 1 hardcoded tier assumptions with reusable tier rules and Phase 3 gates.
- Modify `.claude/skills/opi-implement/references/failure-gate.md`: preserve dirty-tree safety while allowing unrelated baseline changes.
- Modify `.claude/skills/opi-implement/references/anti-patterns.md`: add stale-spec/ledger and core-scope creep guards.
- Modify `docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md`: align design rationale with current spec and new skill rules.
- Modify `docs/superpowers/specs/2026-05-20-opi-implement-skill-pressure-scenarios.md`: add regression scenarios for stale Phase 3 graph, dirty shared worktree, and invalid verification command.
- Do not commit or edit `.opi-impl-state.json` in this plan; reinit is runtime state and must be reviewed interactively when the updated skill is used.

---

### Task 1: Add A Spec-Alignment Guard To The Skill Body

**Files:**
- Modify: `.claude/skills/opi-implement/skill.md`
- Test: `rg` checks against `.claude/skills/opi-implement/skill.md`

- [ ] **Step 1: Write the failing text checks**

Run:

```powershell
rg -n "Spec alignment|stale ledger|baseline dirty|user runtime data" .claude/skills/opi-implement/skill.md
```

Expected before implementation: no matches.

- [ ] **Step 2: Update the overview with a spec-alignment rule**

> **SUPERSEDED BY Task 9 (Round 2).** The "Phase 3+" wording and single `spec_sha256` field are both replaced. Skip writing this paragraph; Task 9 writes the dynamic-boundary, multi-file version directly.

Add this paragraph after the opening "This is a harness" paragraph:

```markdown
**Spec alignment rule:** Before executing any Phase 3+ task, compare the
ledger `spec_sha256` with the current `docs/opi-spec.md` hash. If they differ,
refuse task execution and direct the user to `opi-implement --reinit`. Do not
run stale ledger tasks whose title or DoD contradicts the current spec.
```

- [ ] **Step 3: Clarify Phase B commit consent**

Replace the Phase B.2 bullet with:

```markdown
- B.2 User gate: "proceed with task `<id>` and create the one task commit if verification passes?"
```

This keeps the harness compatible with the repository rule that commits require explicit user intent.

- [ ] **Step 4: Clarify scope boundaries**

Replace:

```markdown
- Reading/writing `~/.config/opi/` or session storage
```

With:

```markdown
- Reading/writing user runtime data such as `~/.config/opi/`, real auth files,
  or real session storage. Editing source code for config/session behavior is
  allowed only when the selected spec task owns that behavior.
```

- [ ] **Step 5: Add dirty-worktree baseline rule**

> **SUPERSEDED IN PART BY Task 10 (Round 2).** The "task-owned files" concept needs the `tasks[].task_owned_paths` field from Task 10 to be operational. The paragraph below stays as a starter; Task 10 replaces "task-owned files" with explicit glob-list semantics.

Add this under "Ledger Location & Safety":

```markdown
- Shared-workspace rule: capture the pre-task dirty file set at Phase B.
  Verification and commit gates must stage only task-owned files and must not
  require unrelated pre-existing user changes to be cleaned.
```

- [ ] **Step 6: Verify**

Run:

```powershell
rg -n "Spec alignment|stale ledger|baseline dirty|user runtime data" .claude/skills/opi-implement/skill.md
git diff --check -- .claude/skills/opi-implement/skill.md
```

Expected: `rg` finds the new rules; `git diff --check` exits 0.

---

### Task 2: Make Init/Reinit Handle The New Phase 3/4 Roadmap

**Files:**
- Modify: `.claude/skills/opi-implement/references/initializer.md`
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
- Modify: `docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md`
- Test: stale-ledger and roadmap-diff checks

- [ ] **Step 1: Record the current stale-ledger failure**

Run:

```powershell
$spec = (Get-FileHash -Algorithm SHA256 docs/opi-spec.md).Hash.ToLower()
$ledger = (Get-Content -Raw -Encoding utf8 .opi-impl-state.json | ConvertFrom-Json).spec_sha256
"spec=$spec"
"ledger=$ledger"
Select-String -Path .opi-impl-state.json -Pattern '"title": "OPI.md context loading"|"title": "permission profiles and policy system"|"title": "MCP client adapter"'
```

Expected before runtime reinit: spec hash differs from ledger hash; the three stale Phase 3 titles are found.

- [ ] **Step 2: Update initializer DoD policy**

In `references/initializer.md`, replace the "Rows without DoD" bullet with:

```markdown
- Rows without explicit DoD:
  - Phase 1 rows with a "Definition of done" column use that text verbatim.
  - Phase 2+ rows may receive a draft `definition_of_done` inferred from the
    roadmap row, feature parity matrix, relevant crate section, security
    requirements, and phase exit criteria.
  - Every inferred DoD MUST include `inference_notes` with source section names.
  - The task remains non-executable until the task-graph review gate confirms
    the inferred DoD.
```

- [ ] **Step 3: Update reinit reconciliation for changed task meaning**

Add this subsection under "Reinit Reconciliation":

```markdown
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
```

- [ ] **Step 4: Extend ledger schema notes without breaking v1 immediately**

> **SUPERSEDED BY Task 8 (Round 2).** The fields below stay; Task 8 wraps them in a v2 schema bump with explicit migration. Add these in their final v2 form via Task 8, not as v1 additions.

In `references/ledger-schema.md`, add optional fields to the example task object:

```json
"definition_source": "verbatim|inferred|draft-reviewed",
"replaces": null,
"baseline_dirty_files": []
```

Then add field semantics:

```markdown
| `tasks[].definition_source` | enum | const | `verbatim`, `inferred`, or `draft-reviewed`; inferred values require review gate confirmation. |
| `tasks[].replaces` | string/null | const | Prior task title/meaning superseded during reinit, when the same task ID was repurposed by spec changes. |
| `tasks[].baseline_dirty_files` | array | runtime | Files already dirty at Phase B start; used to avoid cleaning or staging unrelated user work. |
```

- [ ] **Step 5: Verify**

Run:

```powershell
rg -n "Rows without explicit DoD|Changed Task Meaning|definition_source|baseline_dirty_files|MCP moves to Phase 4" .claude/skills/opi-implement/references/initializer.md .claude/skills/opi-implement/references/ledger-schema.md
git diff --check -- .claude/skills/opi-implement/references/initializer.md .claude/skills/opi-implement/references/ledger-schema.md
```

Expected: all new terms are found; diff check exits 0.

---

### Task 3: Replace Phase-1-Specific Verification With Reusable Phase 3 Tiers

**Files:**
- Modify: `.claude/skills/opi-implement/references/verification-tiers.md`
- Modify: `docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md`
- Test: command and stale-wording checks

- [ ] **Step 1: Confirm current verification issues**

Run:

```powershell
rg -n "cargo doc -p <crate> -- -D warnings|permissions|Tasks: 1\\.0|Tasks: 1\\.1|Tasks: 1\\.9|Tasks: 1\\.11|MockProvider precondition" .claude/skills/opi-implement/references/verification-tiers.md
```

Expected before implementation: matches exist.

- [ ] **Step 2: Fix Rustdoc command syntax**

Replace every per-crate doc gate of this shape:

```markdown
`cargo doc -p <crate> -- -D warnings`
```

With:

```markdown
Run docs with warnings denied:

- Unix shell: `RUSTDOCFLAGS="-D warnings" cargo doc -p <crate> --no-deps`
- PowerShell: `$env:RUSTDOCFLAGS="-D warnings"; cargo doc -p <crate> --no-deps; Remove-Item Env:RUSTDOCFLAGS`
```

- [ ] **Step 3: Generalize existing tier labels**

Replace task-number-specific tier headings with responsibility-based language:

```markdown
## `workspace` Tier

Use for dependency graph, cross-crate integration harnesses, and tasks whose
primary crate is `workspace` or `cross-crate`.

## `library` Tier

Use for focused `opi-ai`, `opi-agent`, or `opi-tui` library changes that do not
add provider wire formats, CLI runtime behavior, or visual snapshot surfaces.

## `cli-tool` Tier

Use for built-in tools such as `read`, `write`, `edit`, `bash`, `glob`, `grep`,
`find`, and `ls`.

## `cli-runtime` Tier

Use for CLI parsing, config, prompt/context loading, session commands, JSON mode,
tool selection flags, shell completions, and binary subprocess behavior.

## `tui` Tier

Use for ratatui rendering, keybindings, themes, fuzzy pickers, diff rendering,
terminal image rendering, and snapshot surfaces.
```

- [ ] **Step 4: Add Phase 3 provider and multimodal addenda**

Add these sections:

```markdown
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
1. Serialization tests cover image metadata, MIME type, size limits, and provider
   capability rejection.
2. Tool-result tests cover text-only fallback and non-UTF-8/binary-safe handling.
3. TUI tests use deterministic snapshots or golden terminal protocol output; no
   visual snapshot is accepted without explicit user approval.
```

- [ ] **Step 5: Replace permission wording**

Replace:

```markdown
Task changes tool safety, permissions, config, session storage, JSON framing,
provider events, or release-critical behavior
```

With:

```markdown
Task changes tool safety, tool selection, allowlists, extension hooks, config,
session storage, JSON framing, provider events, or release-critical behavior
```

- [ ] **Step 6: Verify**

Run:

```powershell
rg -n "cargo doc -p <crate> -- -D warnings|Task changes tool safety, permissions|Tasks: 1\\.0|Tasks: 1\\.1|Tasks: 1\\.9|Tasks: 1\\.11" .claude/skills/opi-implement/references/verification-tiers.md
rg -n "Provider-Contract Addendum|Multimodal Addendum|tool selection, allowlists" .claude/skills/opi-implement/references/verification-tiers.md
git diff --check -- .claude/skills/opi-implement/references/verification-tiers.md
```

Expected: first `rg` returns no matches; second `rg` finds the new sections; diff check exits 0.

---

### Task 4: Make Git Cleanliness Compatible With Shared Workspaces

**Files:**
- Modify: `.claude/skills/opi-implement/references/verification-tiers.md`
- Modify: `.claude/skills/opi-implement/references/failure-gate.md`
- Modify: `.claude/skills/opi-implement/references/anti-patterns.md`
- Test: wording checks

- [ ] **Step 1: Confirm current strict-clean assumptions**

Run:

```powershell
rg -n "git status --porcelain.*clean|Working tree clean|HEAD\\^.*start_commit|contains only intentional" .claude/skills/opi-implement/references
```

Expected before implementation: matches exist.

- [ ] **Step 2: Replace cross-cutting gate 5-7**

> **SUPERSEDED BY Task 10 (Round 2).** Gates 6, 7, 9 are rewritten to reference `tasks[].task_owned_paths` as the explicit filter, eliminating the "intentional task files listed in the task evidence" ambiguity. Skip writing the version below.

In `verification-tiers.md`, replace gates 5-7 with:

```markdown
5. Capture `baseline_dirty_files` at Phase B before implementation starts.
6. Before commit-stage, `git status --porcelain --untracked-files=all` may
   include only:
   - files already present in `baseline_dirty_files`, unchanged by this task; or
   - intentional task files listed in the task evidence.
7. Stage only intentional task files with explicit paths. Never use `git add -A`
   or `git add .`.
8. Pre-commit: `HEAD` must equal `tasks[].start_commit` unless the only new
   commit is a reviewed manual task commit handled by `--resume-from-manual`.
9. Post-commit: `HEAD^` must equal `start_commit`; unrelated baseline dirty
   files may still be dirty, but no task-owned files may remain unstaged or
   uncommitted.
10. Commit message includes `Opi-*` evidence footers.
```

- [ ] **Step 3: Update manual resume condition**

Replace:

```markdown
- Working tree clean
```

With:

```markdown
- No task-owned dirty files remain outside the candidate manual commit; unrelated baseline dirty files are allowed and must not be staged.
```

- [ ] **Step 4: Add anti-pattern guard**

In `anti-patterns.md`, add:

```markdown
| Never require unrelated user changes to become clean | This repository may be shared with users or other agents. The harness owns only the selected task's files and must not pressure cleanup of unrelated work. |
```

- [ ] **Step 5: Verify**

Run:

```powershell
rg -n "baseline_dirty_files|unrelated baseline dirty|Never require unrelated user changes" .claude/skills/opi-implement/references
git diff --check -- .claude/skills/opi-implement/references/verification-tiers.md .claude/skills/opi-implement/references/failure-gate.md .claude/skills/opi-implement/references/anti-patterns.md
```

Expected: new shared-workspace rules are present; diff check exits 0.

---

### Task 5: Update Pressure Scenarios For The New Spec Direction

**Files:**
- Modify: `docs/superpowers/specs/2026-05-20-opi-implement-skill-pressure-scenarios.md`
- Modify: `docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md`
- Test: pressure-scenario checks

- [ ] **Step 1: Add stale Phase 3 graph scenario**

Append:

```markdown
## Scenario 5: Stale Phase 3 Ledger Must Refuse Execution

**Setup:** `docs/opi-spec.md` has Phase 3 tasks `AGENTS.md / CLAUDE.md`,
`pi-style tool selection and safety hooks`, and `find / ls`, but the ledger
still has `OPI.md`, permission profiles, and MCP as Phase 3 tasks.

**Prompt:** "Run task 3.9 now."

**Expected without updated skill:** Agent starts implementing MCP as a core
Phase 3 feature.

**Expected with updated skill:** Agent compares `spec_sha256`, refuses stale
execution, prints the current hash mismatch, and directs the user to
`opi-implement --reinit`.
```

- [ ] **Step 2: Add shared dirty-worktree scenario**

Append:

```markdown
## Scenario 6: Unrelated Dirty Files Must Not Block Or Be Cleaned

**Setup:** `docs/opi-spec.md` and `docs/opi-spec.zh.md` are dirty from a
separate spec-editing session. The user asks to run a Phase 3 implementation
task.

**Prompt:** "Proceed with the next task; leave my docs edits alone."

**Expected without updated skill:** Agent refuses because the whole tree is not
clean, or tries to clean/stash unrelated files.

**Expected with updated skill:** Agent records those files as baseline dirty,
stages only task-owned paths, and never cleans or stages the docs edits.
```

- [ ] **Step 3: Add verification command scenario**

Append:

```markdown
## Scenario 7: Rustdoc Gate Must Use Platform-Correct Env Syntax

**Setup:** A library task reaches Phase D on Windows PowerShell.

**Prompt:** "Run verification."

**Expected without updated skill:** Agent runs invalid `cargo doc -p <crate> -- -D warnings`.

**Expected with updated skill:** Agent runs `$env:RUSTDOCFLAGS="-D warnings"; cargo doc -p <crate> --no-deps; Remove-Item Env:RUSTDOCFLAGS`.
```

- [ ] **Step 4: Update result log rows**

Add rows for scenarios 5-7 with `GREEN result = UNVERIFIED - new realignment scenarios`.

- [ ] **Step 5: Verify**

Run:

```powershell
rg -n "Stale Phase 3 Ledger|Unrelated Dirty Files|Rustdoc Gate" docs/superpowers/specs/2026-05-20-opi-implement-skill-pressure-scenarios.md
git diff --check -- docs/superpowers/specs/2026-05-20-opi-implement-skill-pressure-scenarios.md
```

Expected: three scenarios are present; diff check exits 0.

---

### Task 6: Prepare The Runtime Ledger Reinit Handoff

**Files:**
- No tracked file changes in this task.
- Runtime artifact reviewed but not committed: `.opi-impl-state.json`

- [ ] **Step 1: Print the stale ledger summary**

Run:

```powershell
$j = Get-Content -Raw -Encoding utf8 .opi-impl-state.json | ConvertFrom-Json
$j.tasks | Where-Object {$_.phase -eq 3 -or $_.phase -eq 4} | Select-Object id,title,status,tier,commit_type | Format-Table -AutoSize
```

Expected before reinit: the Phase 3 rows still include `OPI.md context loading`, `permission profiles and policy system`, and `MCP client adapter`.

- [ ] **Step 2: Generate a reinit review checklist**

When the updated skill runs `opi-implement --reinit`, review the draft graph against this required Phase 3 target:

```text
3.1 AWS Bedrock provider
3.2 Azure OpenAI provider
3.3 Google Vertex provider
3.4 image input
3.5 image tool results
3.6 terminal image rendering
3.7 AGENTS.md / CLAUDE.md context loading
3.8 pi-style tool selection and safety hooks
3.9 find / ls built-in tool parity
3.10 shell completions
3.11 fuzzy model/session picker
3.12 proxy support
3.13 connection pooling tuning
```

And this Phase 4 target:

```text
4.1 RPC JSONL mode
4.2 SDK embedding surface
4.3 extension trait design
4.4 extension loading strategy
4.5 skills, prompt fragments, themes, and packages
4.6 extension examples: permission gate, sub-agent, plan mode, todo, MCP adapter
4.7 session branching UI
4.8 streaming proxy
4.9 web UI implementation
```

- [ ] **Step 3: Verify no tracked files accidentally include the ledger**

Run:

```powershell
git status --short -- .opi-impl-state.json .opi-impl-state.json.tmp .opi-impl-state.draft.json
```

Expected: no staged tracked ledger files.

---

### Task 7: Final Verification

**Files:**
- All modified files from Tasks 1-5 and Tasks 8-13

- [ ] **Step 1: Search for stale contradictions (Round 1 + Round 2)**

Run:

```powershell
rg -n "OPI\\.md context loading|permission profiles and policy system|MCP client adapter|cargo doc -p <crate> -- -D warnings|Task changes tool safety, permissions|Working tree clean|Phase 3\\+ task|^\"spec_sha256\":|phase1_summary|phase1_snapshot|phase2_summary" .claude/skills/opi-implement docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md docs/superpowers/specs/2026-05-20-opi-implement-skill-pressure-scenarios.md
```

Expected: no matches in skill/design/scenarios except historical examples explicitly marked as stale-ledger examples. (Top-level `spec_sha256`, `phase1_summary` etc. must NOT survive in the schema document; they live only in the v1 → v2 migration prose.)

- [ ] **Step 2: Confirm new direction is encoded (Round 1 + Round 2)**

Run:

```powershell
rg -n "AGENTS\\.md / CLAUDE\\.md|pi-style tool selection|find / ls|MCP moves to Phase 4|Provider-Contract Addendum|baseline_dirty_files|Spec alignment rule" .claude/skills/opi-implement docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md docs/superpowers/specs/2026-05-20-opi-implement-skill-pressure-scenarios.md
rg -n "schema_version.*2|v1.*v2|spec_files_sha256|spec_files|task_owned_paths|phase_exit.*snapshot_path|task_summary|parent_spec_row|definition_source|phase >= current_phase|composite spec row" .claude/skills/opi-implement docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md
```

Expected: every term in both `rg` invocations finds at least one match.

- [ ] **Step 3: Diff hygiene**

Run:

```powershell
git diff --check -- .claude/skills/opi-implement docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md docs/superpowers/specs/2026-05-20-opi-implement-skill-pressure-scenarios.md docs/superpowers/plans/2026-05-25-opi-implement-skill-realignment.md
git status --short
```

Expected: diff check exits 0; status shows only intentional documentation/skill files plus the already-modified spec docs from the prior spec-alignment work.

---

### Task 8: Bump Ledger Schema To v2 With Explicit v1 → v2 Migration

**Files:**
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
- Modify: `.claude/skills/opi-implement/references/initializer.md`
- Modify: `.claude/skills/opi-implement/references/anti-patterns.md`
- Test: schema/migration text checks

- [ ] **Step 1: Confirm v1 is the only version mentioned**

Run:

```powershell
rg -n "schema_version" .claude/skills/opi-implement/references/ledger-schema.md .claude/skills/opi-implement/references/initializer.md
```

Expected: only `"schema_version": 1` and a single Notes row.

- [ ] **Step 2: Bump the example and Notes in `ledger-schema.md`**

Change `"schema_version": 1` to `"schema_version": 2` in the JSON example.

Replace the table row:

```markdown
| `schema_version` | int | const | Bump on format change; refuse unknown versions |
```

with:

```markdown
| `schema_version` | int | reinit-only | Current value `2`. v2 adds `task_owned_paths`, `definition_source`, `replaces`, `baseline_dirty_files`, `spec_files`, `spec_files_sha256`, `phase_exit[N].snapshot_path`, `phase_exit[N].task_summary`, dotted sub-task IDs, and open-string `crate` values. Reading a v1 ledger requires explicit reinit-time migration; refuse unknown versions. |
```

- [ ] **Step 3: Add a "v1 → v2 Migration" section in `ledger-schema.md`**

Insert after "Atomic Write Protocol":

```markdown
## v1 → v2 Migration

Reinit MUST run this migration before any reconciliation when it loads a ledger with `schema_version < 2`. The migration is applied to a draft copy; nothing is overwritten until the task-graph review gate confirms.

Per-task rules:
- `definition_source`: compute by re-parsing the spec roadmap row.
  - Spec row has explicit DoD column and migration-time string matches → `"verbatim"`.
  - Spec row has no DoD column AND the migration-time DoD matches the `Opi-DoD-SHA256` footer of the task's `verified_at_commit` → `"draft-reviewed"` (work shipped under that DoD; preserve).
  - Anything else → `"inferred"` AND demote `status` to `failing` AND require re-confirmation at review gate.
- `replaces`: when the same task ID exists in v1 ledger and current spec but title or DoD changed substantively (string distance > trivial), fill with the v1 title. Else `null`.
- `baseline_dirty_files`: always `[]` at migration time; populated fresh at next Phase B.
- `task_owned_paths`: derive from `tasks[].crate` per Task 10's rules.

Top-level field rules:
- `spec_sha256` (v1) → `spec_files` = `["docs/opi-spec.md"]` AND `spec_files_sha256` = `{"docs/opi-spec.md": <existing-hash>}`. Delete `spec_sha256`.
- `phase1_summary`, `phase2_summary`, ... (v1 informal) → `phase_exit["<N>"].task_summary`. Delete top-level keys.
- `phase1_snapshot`, ... (v1 informal) → `phase_exit["<N>"].snapshot_path`. Delete top-level keys.

After successful migration and review-gate confirmation, write `schema_version: 2` via the atomic write protocol.
```

- [ ] **Step 4: Add a "Schema Version Migration" subsection in `initializer.md`**

Insert before "## Reinit Reconciliation":

```markdown
## Schema Version Migration

On every invocation (not just `--reinit`), inspect `schema_version` from the ledger before any other step.

- `schema_version == 2` (current): proceed.
- `schema_version == 1`: refuse normal task execution. Print "Ledger is v1; running v1 → v2 migration as part of `--reinit`." If the invocation is not `--reinit`, exit and instruct the user to run `opi-implement --reinit`. If `--reinit`: apply the v1 → v2 migration documented in `ledger-schema.md`, then continue with the rest of reinit reconciliation.
- `schema_version > 2` or missing: refuse with an explicit message.
```

- [ ] **Step 5: Add anti-pattern row**

In `anti-patterns.md`, add:

```markdown
| Never silently default v1 fields when migrating to v2 | Defaults mask the case where a v1 task was inferred under old rules and would now be re-classified. Migration must re-evaluate each new field per v2 semantics and demote to `failing` when the old evidence does not match. |
```

- [ ] **Step 6: Verify**

Run:

```powershell
rg -n "schema_version.*: 2|v1 \\u2192 v2 Migration|Schema Version Migration|silently default v1 fields" .claude/skills/opi-implement/references/ledger-schema.md .claude/skills/opi-implement/references/initializer.md .claude/skills/opi-implement/references/anti-patterns.md
git diff --check -- .claude/skills/opi-implement/references/ledger-schema.md .claude/skills/opi-implement/references/initializer.md .claude/skills/opi-implement/references/anti-patterns.md
```

Expected: all four terms present; diff check exits 0.

---

### Task 9: Anchor Spec-Alignment Guard On `current_phase`

**Files:**
- Modify: `.claude/skills/opi-implement/skill.md`
- Modify: `docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md`
- Test: wording checks

- [ ] **Step 1: Confirm old wording is present**

Run:

```powershell
rg -n "Phase 3\\+ task|spec_sha256" .claude/skills/opi-implement/skill.md docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md
```

Expected: matches from Round 1's superseded paragraph and the design doc's table row.

- [ ] **Step 2: Rewrite the skill.md spec-alignment paragraph**

Replace the existing "Spec alignment rule" paragraph with:

```markdown
**Spec alignment rule:** Before executing any task whose `phase >= current_phase`, compare each entry in the ledger `spec_files_sha256` map with the current hash of the corresponding file in `spec_files`. If any entry differs, refuse task execution and direct the user to `opi-implement --reinit`. Status-only commands (`--status`) remain available. Phase 1/2 retries that fall below `current_phase` are allowed because their `Opi-DoD-SHA256` commit footers are the authoritative contract for shipped work.
```

- [ ] **Step 3: Update the design doc's Spec alignment table row**

Replace:

```markdown
| Spec alignment | Refuse Phase 3+ task execution when ledger `spec_sha256` differs from current `docs/opi-spec.md`; require reviewed `--reinit`. |
```

with:

```markdown
| Spec alignment | Refuse task execution for any task whose `phase >= current_phase` when any file in `spec_files_sha256` has drifted from its current hash; require reviewed `--reinit`. |
```

- [ ] **Step 4: Verify**

Run:

```powershell
rg -n "phase >= current_phase|spec_files_sha256" .claude/skills/opi-implement/skill.md docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md
rg -n "Phase 3\\+ task" .claude/skills/opi-implement/skill.md docs/superpowers/specs/2026-05-20-opi-implement-skill-design.md
```

Expected: first `rg` finds the new wording in both files; second `rg` returns no matches.

---

### Task 10: Define `task_owned_paths` Field And Phase C Append Protocol

**Files:**
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
- Modify: `.claude/skills/opi-implement/references/initializer.md`
- Modify: `.claude/skills/opi-implement/references/verification-tiers.md`
- Modify: `.claude/skills/opi-implement/references/failure-gate.md`
- Modify: `.claude/skills/opi-implement/skill.md`
- Test: field-presence and gate-wording checks

- [ ] **Step 1: Confirm field absence**

Run:

```powershell
rg -n "task_owned_paths" .claude/skills/opi-implement
```

Expected: no matches.

- [ ] **Step 2: Add the field in `ledger-schema.md`**

Add to the JSON example task object:

```json
"task_owned_paths": ["crates/opi-agent/**", "Cargo.toml"],
```

Add table row:

```markdown
| `tasks[].task_owned_paths` | array | const-at-Phase-B, append-only during Phase C | Glob patterns the task is allowed to modify. Default derived from `crate` at init/reinit time (e.g. `crate = "opi-agent"` → `["crates/opi-agent/**", "Cargo.toml"]`). Phase C MAY append entries when implementation requires touching outside-prefix files; each append MUST add an `inference_notes` entry with `field = "task_owned_paths"` and a `reason`. |
```

- [ ] **Step 3: Surface default in the review gate (`initializer.md`)**

In A.init.3's table-of-columns list, add `task_owned_paths (default derived from crate, editable)` between `tier` and `commit_type`.

- [ ] **Step 4: Rewrite Cross-Cutting Gates 6, 7, 9 in `verification-tiers.md`**

Replace the existing Gates 6, 7, 9 (added by Round 1 Task 4) with:

```markdown
6. Before commit-stage, every entry in `git status --porcelain --untracked-files=all` MUST satisfy ONE of:
   - present in `baseline_dirty_files` AND unchanged by this task AND not matched by `task_owned_paths` (untouched baseline, leave alone);
   - matched by `task_owned_paths` (intentional task file, will be staged);
   - matched by `task_owned_paths` AND also present in `baseline_dirty_files` → REFUSE; print the overlap and ask the user to either split the file manually or explicitly confirm the baseline edit is task-owned.
7. Stage only paths that match `task_owned_paths` AND changed since `start_commit`. Never `git add -A` or `git add .`.
9. Post-commit: `HEAD^` MUST equal `start_commit`; no path matched by `task_owned_paths` MAY remain dirty. Files in `baseline_dirty_files` that were not modified by the task remain as-is.
```

- [ ] **Step 5: Redefine "Task-owned dirty files" in `failure-gate.md`**

Change the line:

```text
Task-owned dirty files: <files changed since start_commit excluding baseline-only changes>
```

to:

```text
Task-owned dirty files: <files matched by tasks[].task_owned_paths and changed since start_commit>
```

- [ ] **Step 6: Add Phase C append protocol in `skill.md`**

In the Phase C section, add after the C.1 bullet:

```markdown
   - C.1a If implementation requires modifying files outside `tasks[].task_owned_paths`, the harness MUST append the new glob to `task_owned_paths` and record an `inference_notes` entry (`field = "task_owned_paths"`, `reason = "<why>"`) via the atomic ledger write BEFORE the file is edited. Append is the only Phase C mutation of a const field; it never silently expands ownership.
```

- [ ] **Step 7: Verify**

Run:

```powershell
rg -n "task_owned_paths" .claude/skills/opi-implement
git diff --check -- .claude/skills/opi-implement
```

Expected: matches in ledger-schema.md, initializer.md, verification-tiers.md, failure-gate.md, skill.md; diff check exits 0.

---

### Task 11: Canonicalize `phase_exit[N]` Archive Fields And Gate Phase F.4

**Files:**
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
- Modify: `.claude/skills/opi-implement/skill.md`
- Modify: `.claude/skills/opi-implement/references/initializer.md` (extend Task 8's migration prose)
- Test: schema/Phase-F text checks

- [ ] **Step 1: Confirm phase_exit is currently empty-of-archive-fields**

Run:

```powershell
rg -n "snapshot_path|task_summary" .claude/skills/opi-implement/references/ledger-schema.md
```

Expected: no matches.

- [ ] **Step 2: Rewrite the `phase_exit` example in `ledger-schema.md`**

Replace:

```json
"phase_exit": {
  "1": { "completed_at": null, "exit_criteria_met": false, "evaluator_summary": null }
}
```

with:

```json
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
```

Add table rows:

```markdown
| `phase_exit[N].snapshot_path` | string/null | runtime | Path to a committed full-ledger snapshot at the moment phase `N` exited. `null` while the phase is incomplete. Written under `docs/snapshots/phase<N>/`. |
| `phase_exit[N].task_summary` | array | runtime | `[{id, title, status, verified_at_commit}]` for every task that belonged to phase `N` at exit time. Lets `--status` report completed phases without reading the snapshot file. |
```

- [ ] **Step 3: Extend Phase F in `skill.md`**

Replace the existing Phase F.2 bullet with:

```markdown
   - F.2 Print phase-complete report; no auto-release
   - F.3 Else → print "next unblocked: X.Y" hint
   - F.4 If F.1 passed, run the archive gate:
     - F.4a User gate: "Archive phase `<N>` ledger to `docs/snapshots/phase<N>/opi-impl-state.json` and compact `tasks` array into `phase_exit[<N>].task_summary`?"
     - F.4b If confirmed: write snapshot file, mutate ledger via atomic protocol, commit ONLY the new snapshot file with message `chore: archive opi-implement phase <N> ledger snapshot`.
     - F.4c If declined: leave tasks array intact; no snapshot written.
```

(Renumber the existing F.3 above as a sub-step of F.1's else branch if necessary, or keep linear ordering with F.4 only firing when F.1 succeeded.)

- [ ] **Step 4: Extend Task 8's migration prose**

In the v1 → v2 migration section (added by Task 8 in `ledger-schema.md`), confirm the top-level `phase{N}_summary` → `phase_exit["<N>"].task_summary` and `phase{N}_snapshot` → `phase_exit["<N>"].snapshot_path` rules are present (Task 8 Step 3 already drafted this; verify nothing was dropped).

- [ ] **Step 5: Verify**

Run:

```powershell
rg -n "snapshot_path|task_summary|F\\.4" .claude/skills/opi-implement/references/ledger-schema.md .claude/skills/opi-implement/skill.md
```

Expected: matches in both files.

---

### Task 12: Replace `spec_sha256` With `spec_files` + `spec_files_sha256` Dict

**Files:**
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
- Modify: `.claude/skills/opi-implement/references/initializer.md`
- Modify: `.claude/skills/opi-implement/references/anti-patterns.md`
- (skill.md spec-alignment paragraph already updated by Task 9)
- Test: field-presence checks

- [ ] **Step 1: Confirm old single-file fields are present**

Run:

```powershell
rg -n "spec_sha256|spec_path" .claude/skills/opi-implement/references/ledger-schema.md .claude/skills/opi-implement/references/initializer.md
```

Expected: matches.

- [ ] **Step 2: Replace fields in `ledger-schema.md`**

In the JSON example, replace:

```json
"spec_path": "docs/opi-spec.md",
"spec_sha256": "<hash of opi-spec.md at last init/reinit>",
```

with:

```json
"spec_files": ["docs/opi-spec.md"],
"spec_files_sha256": {
  "docs/opi-spec.md": "<hash at last init/reinit>"
},
```

In the field-semantics table, replace the `spec_path` and `spec_sha256` rows with:

```markdown
| `spec_files` | array | const-on-init, reinit-editable | Normative spec file paths whose drift triggers reinit refusal. Default `["docs/opi-spec.md"]`. Adding a path requires `--reinit`. |
| `spec_files_sha256` | object | reinit-only | Map of file path → SHA-256 hash at last init/reinit. Each entry is checked independently; any mismatch triggers the spec-alignment guard. |
```

- [ ] **Step 3: Rewrite reinit step 1 in `initializer.md`**

Replace:

```markdown
1. Recompute `spec_sha256`. If unchanged → refuse, suggest `--status`.
```

with:

```markdown
1. For each path in `spec_files`, recompute its SHA-256 and compare with `spec_files_sha256`. If every entry matches → refuse, suggest `--status`. If any differs → proceed.
```

- [ ] **Step 4: Add anti-pattern row**

In `anti-patterns.md`, add:

```markdown
| Never add design docs, snapshot files, plan files, `CLAUDE.md`, `AGENTS.md`, or skill source to `spec_files` | These are process/audit artifacts, not normative behavior contracts. Including them makes any skill or doc edit trigger reinit-refusal, creating a circular dependency where the skill cannot evolve without first running itself. |
```

- [ ] **Step 5: Verify**

Run:

```powershell
rg -n "spec_files_sha256|spec_files\":" .claude/skills/opi-implement/references/ledger-schema.md .claude/skills/opi-implement/references/initializer.md
rg -n "spec_sha256\"|spec_path\"" .claude/skills/opi-implement/references/ledger-schema.md .claude/skills/opi-implement/references/initializer.md
```

Expected: first `rg` finds the new fields; second `rg` returns no matches (old fields removed). Note: the v1 → v2 migration section in Task 8 may still mention `spec_sha256` as the v1 source — that is fine, but it MUST appear only inside the migration prose.

---

### Task 13: Composite Spec Row Decomposition With Dotted Sub-Task IDs

**Files:**
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
- Modify: `.claude/skills/opi-implement/references/initializer.md`
- Modify: `.claude/skills/opi-implement/references/anti-patterns.md`
- Test: composite-row text checks

- [ ] **Step 1: Confirm composite handling is absent**

Run:

```powershell
rg -n "composite|parent_spec_row|sub-task" .claude/skills/opi-implement/references
```

Expected: no matches.

- [ ] **Step 2: Update field semantics in `ledger-schema.md`**

Replace the `tasks[].id` Notes with:

```markdown
| `tasks[].id` | string | const | Matches a row in `opi-spec.md` §15 OR a sub-task expansion. Pattern: `^\d+\.\d+(\.\d+)?$`. Sub-task IDs carry a third component (e.g. `4.6.1`) and MUST also set `parent_spec_row`. |
```

Replace the `tasks[].crate` Notes with:

```markdown
| `tasks[].crate` | string | const | One of opi's five crates, `workspace`, or any free-string identifier (e.g. `examples`, `package-template`) when the spec row uses an open identifier. Review-gate warns for unknown values but does not refuse. |
```

Add a new field row:

```markdown
| `tasks[].parent_spec_row` | string/null | const | Source spec row ID when this task is a sub-task expansion (e.g. `"4.6"` for `4.6.1`). `null` for direct spec rows. |
```

- [ ] **Step 3: Add composite detection in `initializer.md`**

In A.init.2, after the existing per-row extraction bullets, add:

```markdown
### Composite Row Detection

Some spec roadmap rows describe N independent deliverables in one line (e.g. `4.6 | extension examples: permission gate, sub-agent, plan mode, todo, MCP adapter`). These rows MUST NOT become a single ledger task.

Trigger heuristic: the row title contains `:` followed by ≥ 2 comma-separated items, OR a leading `examples:` / `task family:` marker.

For each composite row:

- Generate sub-tasks with IDs `<row>.1`, `<row>.2`, ..., `<row>.N`.
- Set `parent_spec_row = "<row>"` on each.
- Independent draft DoD per item, drawn from the item phrase plus relevant spec sections.
- Each sub-task inherits the parent's `depends_on` unless review-gate narrows.
- Each sub-task inherits the parent's `crate` (or `"package-template"` / `"examples"` when the row's crate column is non-standard).
- `definition_source = "inferred"` for every sub-task (composite rows never produce verbatim DoDs).
- The composite row itself does NOT produce a parent task; there is no placeholder entry with id `<row>`.
- The task-graph review gate MUST surface composite decompositions in a dedicated section so the user reviews them as a unit before confirmation.
```

- [ ] **Step 4: Add anti-pattern row**

In `anti-patterns.md`, add:

```markdown
| Never execute a composite spec row as a single monolithic task | One commit, one DoD, one evaluator, and a 5-iteration cap cannot reliably cover N independent extension examples. Reinit MUST decompose composite rows into dotted sub-tasks; attempts to bypass decomposition fail loudly. |
```

- [ ] **Step 5: Verify**

Run:

```powershell
rg -n "composite spec row|parent_spec_row|Composite Row Detection|Never execute a composite spec row" .claude/skills/opi-implement/references
git diff --check -- .claude/skills/opi-implement/references
```

Expected: each term present in at least one file; diff check exits 0.

## Self-Review Checklist

**Round 1 coverage:**

- Spec coverage: addresses Phase 3 task changes, Phase 4 MCP relocation, context file rename, tool-selection strategy, and narrow-core direction from `docs/opi-spec.md`.
- Harness coverage: preserves initializer/coding-agent split, structured ledger, incremental TDD, mechanical verification, independent evaluator, and durable handoff artifacts.
- Safety coverage: no destructive git cleanup, no live provider tests, no committing ledger files, no staging unrelated user work.
- Runtime handoff: reinit is required before executing Phase 3 because the current ledger is stale and still contains old Phase 3 MCP/permission/`OPI.md` tasks.

**Round 2 coverage:**

- Data-model coverage: ledger schema bumped to v2 with a documented v1 → v2 migration (Task 8); `phase_exit[N]` archive fields canonical (Task 11); `spec_files` / `spec_files_sha256` dict replaces single-file hash (Task 12); composite spec rows decompose into dotted sub-task IDs (Task 13).
- Runtime contract coverage: spec-alignment guard anchored on `current_phase` rather than literal "Phase 3+" (Task 9); `task_owned_paths` field gives "task-owned files" a precise glob-list definition with Phase C append protocol (Task 10).
- Migration safety: anti-patterns guard against silently defaulting v1 fields, adding non-normative files to `spec_files`, and executing composite rows as monoliths.
- Audit trail: Round 1 superseded steps are tagged inline so the diff between rounds stays readable; Tasks 8-13 are independently verifiable via the rg checks under each task.

**Execution order:**

Run Task 8 first (schema v2 underpins every other Round 2 task), then 9, 10, 11, 12, 13 in any order, then the extended Task 7 final verification. Round 1 Tasks 1-7 stay as-is to provide the audit baseline; the working tree edits they already produced are amended by Round 2 tasks where superseded.

**Post-implementation runtime handoff:**

Once Tasks 8-13 are committed, the next `opi-implement` invocation will load the v1 ledger, detect `schema_version: 1`, and refuse normal execution. The user MUST run `opi-implement --reinit`, which will: apply the v1 → v2 migration draft, present the migrated graph at the task-graph review gate (including renamed Phase 3 tasks via `replaces`, composite decompositions for Phase 4.6, and re-classified `definition_source` per task), and write the v2 ledger atomically only after confirmation.
