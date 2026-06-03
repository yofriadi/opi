# Phase 4 Ledger Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair the Phase 4 `.opi-impl-state.json` task graph and the `opi-implement` initialization rules so future Phase 4 work stays aligned with `docs/opi-spec.md`, pi 0.75.3 design semantics, and the harness schema/verification rules.

**Architecture:** Treat `docs/opi-spec.md` as normative and leave it unchanged. Update the `opi-implement` skill references in both `.claude/skills/opi-implement/` and `.agents/skills/opi-implement/` so reinit can regenerate the same corrected graph, then apply an equivalent structured repair to the gitignored runtime ledger. The repaired graph keeps substrate tasks before package examples, splits multi-resource work, pins ambiguous crate ownership, and makes every behavioral test path executable under the selected task's owned paths and verification tier.

**Tech Stack:** Markdown skill references, JSON ledger v2, PowerShell/Node structured JSON validation, Cargo verification commands.

---

## File Structure

- Modify: `.claude/skills/opi-implement/references/initializer.md`
  - Broadens composite-row detection beyond colon-only rows and documents Phase 4 multi-resource rows.
- Modify: `.agents/skills/opi-implement/references/initializer.md`
  - Same content as the `.claude` copy; both tracked skill trees must stay byte-identical.
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
  - Tightens validation expectations for `parent_spec_row`, `behavioral_tests`, and `task_owned_paths`.
- Modify: `.agents/skills/opi-implement/references/ledger-schema.md`
  - Same content as the `.claude` copy.
- Modify: `.claude/skills/opi-implement/references/verification-tiers.md`
  - Adds validation rules for cross-crate behavioral tests and snapshot-bearing tasks.
- Modify: `.agents/skills/opi-implement/references/verification-tiers.md`
  - Same content as the `.claude` copy.
- Modify: `.opi-impl-state.json`
  - Runtime ledger only. This file is gitignored and must not be committed.
- Read-only reference: `docs/opi-spec.md`
  - Normative spec. Do not edit for this repair.
- Read-only reference: `.repo/pi-0.75.3/packages/agent/README.md`
  - Agent event order, tool batching, hook semantics, queue behavior.
- Read-only reference: `.repo/pi-0.75.3/packages/ai/src/types.ts`
  - Provider stream lifecycle and model/provider abstractions.
- Read-only reference: `.repo/pi-0.75.3/packages/coding-agent/docs/rpc.md`
  - Strict JSONL, command response semantics, extension UI sub-protocol.
- Read-only reference: `.repo/pi-0.75.3/packages/coding-agent/docs/sdk.md`
  - SDK/resource-loader/runtime layering.
- Read-only reference: `.repo/pi-0.75.3/packages/coding-agent/docs/packages.md`
  - Package resource composition, filtering, precedence, security note.
- Read-only reference: `.repo/pi-0.75.3/packages/coding-agent/docs/skills.md`
  - Progressive skill discovery and command exposure.
- Read-only reference: `.repo/pi-0.75.3/packages/coding-agent/docs/prompt-templates.md`
  - Prompt fragment/template discovery and expansion.
- Read-only reference: `.repo/pi-0.75.3/packages/coding-agent/docs/themes.md`
  - Theme discovery, schema, snapshot-bearing TUI behavior.
- Read-only reference: `.repo/pi-0.75.3/packages/coding-agent/docs/custom-provider.md`
  - Custom provider/model registration semantics.
- Read-only reference: `.repo/pi-0.75.3/packages/coding-agent/docs/session-format.md`
  - Session tree and extension state/message entries.

## Target Graph Decisions

Use these fixed graph changes. Do not reinterpret them during execution.

| Area | Decision |
|---|---|
| `parent_spec_row` | Direct spec rows use `null`; sub-tasks use the parent row string such as `"4.7"` or `"4.8"`. |
| `4.1` | Keep as `opi-coding-agent`/`cli-runtime`; it owns subprocess RPC behavior and protocol framing. Its DoD must state that accepted-command failures after acceptance are emitted as async events, matching pi RPC. |
| `4.2` | Keep after `4.1`; its DoD must say SDK/RPC share command/event types and that SDK public docs do not make a stable `Transport` claim before `4.3`. |
| `4.3` | Keep after `4.1` and `4.2`; it settles `opi-agent::Transport` before extension API and proxy tasks consume it. |
| `4.5` | Add user-facing docs paths because its DoD requires a documented precedence model. |
| `4.6` | Add dependency on `4.2`; custom providers/models are SDK or extension registrations, so both SDK and extension/resource substrate must exist. |
| `4.7` | Replace monolithic `4.7` with `4.7.1` skills, `4.7.2` prompt fragments, `4.7.3` themes, and `4.7.4` packages. |
| `4.8.x` | Depend on the specific `4.7.4` package composition task instead of old `4.7`; add `4.2` to sub-agent example; add `4.3` to MCP adapter. |
| `4.9` | Keep `tui` risk classification but add `opi-agent` mechanical gates because behavioral tests include `crates/opi-agent/tests/session_branching.rs`. |
| `4.10` | Pin crate to `opi-agent`; do not create a new crate in this ledger. |
| `4.11` | Keep dependency on `4.1`, `4.2`, and `4.10`; web UI consumes RPC/SDK events and the Phase 4 streaming substrate. |
| Hardening follow-up | Example/package tasks use `docs/extension-examples/**` instead of broad `docs/**`; public RPC, extension, provider/model registration, and transport/proxy tasks have explicit documentation requirements in their DoD. |

## Replacement `4.7` Tasks

Use these exact task records as the semantic target when updating the ledger. Preserve runtime fields (`status`, `iteration_count`, `start_commit`, `baseline_dirty_files`, `last_attempt`, `verified_at_commit`, `evidence`, `blocker`, `session_notes`) at their current failing/default values unless they already contain data at execution time.

| ID | Title | Crate | Tier | Depends On | Behavioral Tests | Owned Paths |
|---|---|---|---|---|---|---|
| `4.7.1` | skills with progressive discovery | `opi-coding-agent` | `cli-runtime` | `4.5` | `crates/opi-coding-agent/tests/skills_discovery.rs` | `crates/opi-coding-agent/**`, `crates/opi-coding-agent/README.md`, `crates/opi-coding-agent/README.zh.md`, `Cargo.toml` |
| `4.7.2` | prompt fragments/templates with progressive discovery | `opi-coding-agent` | `cli-runtime` | `4.5` | `crates/opi-coding-agent/tests/prompt_fragments.rs` | `crates/opi-coding-agent/**`, `crates/opi-coding-agent/README.md`, `crates/opi-coding-agent/README.zh.md`, `Cargo.toml` |
| `4.7.3` | themes with progressive discovery | `opi-coding-agent / opi-tui` | `tui` | `4.5` | `crates/opi-coding-agent/tests/theme_discovery.rs`, `crates/opi-tui/tests/theme_snapshots.rs` | `crates/opi-coding-agent/**`, `crates/opi-tui/**`, `Cargo.toml` |
| `4.7.4` | packages with progressive resource composition | `opi-coding-agent` | `cli-runtime` | `4.7.1`, `4.7.2`, `4.7.3` | `crates/opi-coding-agent/tests/package_discovery.rs` | `crates/opi-coding-agent/**`, `crates/opi-coding-agent/README.md`, `crates/opi-coding-agent/README.zh.md`, `Cargo.toml` |

## Task 1: Update `opi-implement` Initializer Rules

**Files:**
- Modify: `.claude/skills/opi-implement/references/initializer.md`
- Modify: `.agents/skills/opi-implement/references/initializer.md`

- [ ] **Step 1: Edit composite-row detection**

Replace the trigger paragraph under `### A.init.2a Composite Row Detection` in both files with:

```markdown
Trigger heuristic: a roadmap row is composite when any of these is true:

- the row title contains `:` followed by at least two comma-separated items;
- the row title begins with `examples:` or `task family:`;
- the row title is a Phase 4 resource-family row listing at least three independent resource nouns joined by commas or `and`, such as `skills, prompt fragments, themes, and packages`;
- the row's crate column is an open packaging identifier such as `examples / package template` and the title lists at least two deliverables.

Do not split a row merely because the DoD contains commas. The split decision is based on the roadmap row title and crate column.
```

- [ ] **Step 2: Add a Phase 4 split example**

Add this example after the existing composite-row bullet list:

```markdown
Phase 4 examples:

- `4.7 | skills, prompt fragments, themes, and packages with progressive discovery` becomes `4.7.1` skills, `4.7.2` prompt fragments/templates, `4.7.3` themes, and `4.7.4` packages.
- `4.8 | extension/package examples: permission gate, protected paths, sub-agent, plan mode, todo, MCP adapter` becomes six package/example tasks; the parent row is not executable.
```

- [ ] **Step 3: Verify both copies match**

Run:

```powershell
$a = Get-FileHash .\.claude\skills\opi-implement\references\initializer.md -Algorithm SHA256
$b = Get-FileHash .\.agents\skills\opi-implement\references\initializer.md -Algorithm SHA256
$a.Hash -eq $b.Hash
```

Expected output:

```text
True
```

## Task 2: Tighten Ledger Schema Validation Rules

**Files:**
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
- Modify: `.agents/skills/opi-implement/references/ledger-schema.md`

- [ ] **Step 1: Replace `parent_spec_row` semantics**

Replace the `tasks[].parent_spec_row` row with:

```markdown
| `tasks[].parent_spec_row` | string/null | const | Source spec row ID when this task is a sub-task expansion (e.g. `"4.7"` for `4.7.1`). Direct spec rows MUST use `null`, not an empty string. |
```

- [ ] **Step 2: Add behavioral-test ownership validation**

Add this paragraph after the `tasks[].task_owned_paths` row:

```markdown
Validation rule: every path listed in `tasks[].verification.behavioral_tests` MUST be matched by at least one `task_owned_paths` glob before the task graph is confirmed. This prevents Phase C from needing an immediate ownership expansion just to create the task's declared tests.
```

- [ ] **Step 3: Add cross-crate verification validation**

Add this paragraph after the behavioral-test ownership rule:

```markdown
Validation rule: when `behavioral_tests` references more than one crate, either `tier` MUST be `workspace` or `verification.library_gates` MUST include mechanical gates for every referenced crate. Snapshot-bearing tests also require `snapshot_tests` and explicit snapshot approval under the `tui` rules.
```

- [ ] **Step 4: Verify both copies match**

Run:

```powershell
$a = Get-FileHash .\.claude\skills\opi-implement\references\ledger-schema.md -Algorithm SHA256
$b = Get-FileHash .\.agents\skills\opi-implement\references\ledger-schema.md -Algorithm SHA256
$a.Hash -eq $b.Hash
```

Expected output:

```text
True
```

## Task 3: Tighten Verification Tier Rules

**Files:**
- Modify: `.claude/skills/opi-implement/references/verification-tiers.md`
- Modify: `.agents/skills/opi-implement/references/verification-tiers.md`

- [ ] **Step 1: Add graph confirmation checks**

Add this section immediately before `## Risk Evaluator Gate` in both files:

```markdown
## Task Graph Verification Checks

Before confirming an init or reinit graph:

1. Every `behavioral_tests` path must be covered by `task_owned_paths`.
2. If `behavioral_tests` spans multiple crates, use `workspace` tier or include per-crate `cargo test`, `cargo clippy`, and rustdoc gates for every referenced crate.
3. If any behavioral or snapshot test lives under `crates/opi-tui/tests/`, set `snapshot_tests` for the affected snapshot path and mark snapshot acceptance as explicit human approval.
4. Direct spec rows use `parent_spec_row = null`; only dotted sub-task IDs use a parent row string.
5. Rows with open crate labels such as `examples / package template` must include the concrete test paths they declare, even when implementation files live under `examples/**`.
```

- [ ] **Step 2: Verify both copies match**

Run:

```powershell
$a = Get-FileHash .\.claude\skills\opi-implement\references\verification-tiers.md -Algorithm SHA256
$b = Get-FileHash .\.agents\skills\opi-implement\references\verification-tiers.md -Algorithm SHA256
$a.Hash -eq $b.Hash
```

Expected output:

```text
True
```

## Task 4: Repair Runtime Ledger With Structured JSON

**Files:**
- Modify: `.opi-impl-state.json`

- [ ] **Step 1: Confirm the ledger is ignored**

Run:

```powershell
git check-ignore -v .opi-impl-state.json .opi-impl-state.json.tmp .opi-impl-state.draft.json
```

Expected output includes:

```text
.gitignore:30:.opi-impl-state.json
.gitignore:31:.opi-impl-state.json.tmp
.gitignore:32:.opi-impl-state.draft.json
```

- [ ] **Step 2: Apply structured ledger edits**

Use a structured JSON editor. The transformation must make these exact changes:

```text
All direct tasks: parent_spec_row = null
4.5 task_owned_paths += crates/opi-coding-agent/README.md, crates/opi-coding-agent/README.zh.md
4.6 depends_on = [4.2, 4.4, 4.5]
delete old 4.7
insert 4.7.1, 4.7.2, 4.7.3, 4.7.4 using the Replacement 4.7 Tasks table
4.8.1 depends_on = [4.4, 4.5, 4.7.4]
4.8.2 depends_on = [4.4, 4.5, 4.7.4]
4.8.3 depends_on = [4.2, 4.4, 4.5, 4.7.4]
4.8.4 depends_on = [4.4, 4.5, 4.7.2, 4.7.4]
4.8.5 depends_on = [4.4, 4.5, 4.7.4]
4.8.6 depends_on = [4.1, 4.3, 4.4, 4.5, 4.7.4]
4.8.1 task_owned_paths += crates/opi-coding-agent/tests/permission_gate_example.rs
4.8.2 task_owned_paths += crates/opi-coding-agent/tests/protected_paths_example.rs
4.8.3 task_owned_paths += crates/opi-coding-agent/tests/sub_agent_example.rs
4.8.4 task_owned_paths += crates/opi-coding-agent/tests/plan_mode_example.rs
4.8.5 task_owned_paths += crates/opi-coding-agent/tests/todo_example.rs
4.8.6 task_owned_paths += crates/opi-coding-agent/tests/mcp_adapter_example.rs
4.9 verification.library_gates includes cargo test/clippy/doc for opi-agent and opi-tui
4.10 crate = opi-agent
4.10 task_owned_paths = [crates/opi-agent/**, Cargo.toml]
```

- [ ] **Step 3: Preserve runtime history**

Before writing the ledger, preserve these fields for every existing task ID that remains present:

```text
status
iteration_count
max_iterations
start_commit
baseline_dirty_files
last_attempt
verified_at_commit
evidence
blocker
session_notes
```

For new `4.7.x` tasks, initialize runtime fields with:

```json
{
  "status": "failing",
  "iteration_count": 0,
  "max_iterations": 5,
  "start_commit": null,
  "baseline_dirty_files": [],
  "last_attempt": null,
  "verified_at_commit": null,
  "evidence": null,
  "blocker": null,
  "session_notes": []
}
```

- [ ] **Step 4: Keep inference notes truthful**

Every changed inferred field must include an `inference_notes` entry with these fields:

```json
{
  "field": "<field-name>",
  "reason": "phase4 graph repair aligned with opi-spec.md Phase 4 substrate ordering, pi 0.75.3 resource/extension semantics, and opi-implement validation rules",
  "source": "docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md"
}
```

## Task 5: Validate the Repaired Graph

**Files:**
- Read: `.opi-impl-state.json`
- Read: `docs/opi-spec.md`

- [ ] **Step 1: Verify spec hash still matches**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
$actual = (Get-FileHash .\docs\opi-spec.md -Algorithm SHA256).Hash.ToLowerInvariant()
$ledger = $j.spec_files_sha256.'docs/opi-spec.md'
$actual -eq $ledger
```

Expected output:

```text
True
```

- [ ] **Step 2: Verify graph shape**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
$ids = @{}
foreach ($t in $j.tasks) {
  if ($ids.ContainsKey($t.id)) { throw "duplicate id $($t.id)" }
  $ids[$t.id] = $true
}
if ($ids.ContainsKey("4.7")) { throw "old monolithic 4.7 still exists" }
foreach ($id in "4.7.1","4.7.2","4.7.3","4.7.4","4.8.1","4.8.2","4.8.3","4.8.4","4.8.5","4.8.6") {
  if (-not $ids.ContainsKey($id)) { throw "missing $id" }
}
foreach ($t in $j.tasks) {
  if ($t.id -match '^\d+\.\d+$' -and $null -ne $t.parent_spec_row) { throw "$($t.id) parent_spec_row must be null" }
  if ($t.id -match '^\d+\.\d+\.\d+$' -and [string]::IsNullOrWhiteSpace($t.parent_spec_row)) { throw "$($t.id) missing parent_spec_row" }
}
"ok"
```

Expected output:

```text
ok
```

- [ ] **Step 3: Verify dependencies resolve and no cycles exist**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
$active = @{}
foreach ($t in $j.tasks) { $active[$t.id] = $t }
$archived = @{}
foreach ($p in $j.phase_exit.PSObject.Properties) {
  foreach ($s in $p.Value.task_summary) { $archived[$s.id] = $true }
}
foreach ($t in $j.tasks) {
  foreach ($d in @($t.depends_on)) {
    if (-not $active.ContainsKey($d) -and -not $archived.ContainsKey($d)) {
      throw "missing dependency $($t.id) -> $d"
    }
  }
}
function Visit($id, $path) {
  if ($path -contains $id) { throw "cycle $($path -join ' -> ') -> $id" }
  foreach ($d in @($active[$id].depends_on)) {
    if ($active.ContainsKey($d)) { Visit $d ($path + $id) }
  }
}
foreach ($id in $active.Keys) { Visit $id @() }
"ok"
```

Expected output:

```text
ok
```

- [ ] **Step 4: Verify behavioral tests are owned**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
foreach ($t in $j.tasks) {
  foreach ($test in @($t.verification.behavioral_tests)) {
    $owned = $false
    foreach ($glob in @($t.task_owned_paths)) {
      $pattern = '^' + [regex]::Escape($glob).Replace('\*\*','.*').Replace('\*','[^/\\]*') + '$'
      if ($test -match $pattern) { $owned = $true; break }
    }
    if (-not $owned) { throw "$($t.id) declares unowned behavioral test $test" }
  }
}
"ok"
```

Expected output:

```text
ok
```

- [ ] **Step 5: Verify TUI snapshot-bearing tests declare snapshot paths**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
foreach ($t in $j.tasks) {
  $hasTuiTest = @($t.verification.behavioral_tests | Where-Object { $_ -like "crates/opi-tui/tests/*" }).Count -gt 0
  $snapshotTests = @($t.verification.snapshot_tests | Where-Object { $null -ne $_ -and -not [string]::IsNullOrWhiteSpace([string]$_) })
  $hasSnapshotPaths = $snapshotTests.Count -gt 0
  if ($hasTuiTest -and -not $hasSnapshotPaths) {
    throw "$($t.id) has TUI behavioral tests but no snapshot_tests"
  }
}
"ok"
```

Expected output:

```text
ok
```

## Task 6: Verify Tracked Rule Changes

**Files:**
- Read: `.claude/skills/opi-implement/references/initializer.md`
- Read: `.agents/skills/opi-implement/references/initializer.md`
- Read: `.claude/skills/opi-implement/references/ledger-schema.md`
- Read: `.agents/skills/opi-implement/references/ledger-schema.md`
- Read: `.claude/skills/opi-implement/references/verification-tiers.md`
- Read: `.agents/skills/opi-implement/references/verification-tiers.md`

- [ ] **Step 1: Confirm all skill reference pairs match**

Run:

```powershell
$paths = @(
  "references/initializer.md",
  "references/ledger-schema.md",
  "references/verification-tiers.md",
  "references/anti-patterns.md",
  "references/failure-gate.md",
  "skill.md"
)
foreach ($p in $paths) {
  $a = (Get-FileHash ".\.claude\skills\opi-implement\$p" -Algorithm SHA256).Hash
  $b = (Get-FileHash ".\.agents\skills\opi-implement\$p" -Algorithm SHA256).Hash
  if ($a -ne $b) { throw "skill copy mismatch: $p" }
}
"ok"
```

Expected output:

```text
ok
```

- [ ] **Step 2: Confirm ledger is not staged**

Run:

```powershell
git status --short
```

Expected result:

```text
No line beginning with "A  .opi-impl-state.json", "M  .opi-impl-state.json", or "?? .opi-impl-state.json".
```

- [ ] **Step 3: Run documentation-safe checks**

Run:

```powershell
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected result:

```text
Both commands exit 0.
```

## Commit Guidance

Do not commit during execution unless the user explicitly asks. When the user asks for a commit, stage only these tracked files:

```powershell
git add docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md
git add .agents/skills/opi-implement/skill.md
git add .claude/skills/opi-implement/references/initializer.md
git add .agents/skills/opi-implement/references/initializer.md
git add .claude/skills/opi-implement/references/ledger-schema.md
git add .agents/skills/opi-implement/references/ledger-schema.md
git add .claude/skills/opi-implement/references/verification-tiers.md
git add .agents/skills/opi-implement/references/verification-tiers.md
```

Use this commit message only after all checks pass:

```text
chore: tighten phase 4 opi-implement ledger rules
```

Never stage `.opi-impl-state.json`.

## Self-Review

- Spec coverage: Phase 4 substrate ordering is preserved; examples remain extension/package tasks; web UI remains a consumer of RPC/SDK events; custom providers depend on both SDK and extension/resource substrate; `Transport` remains settled before extension/proxy consumers.
- pi alignment: RPC strict JSONL and accepted-vs-runtime failure semantics are preserved; SDK shares session/runtime event semantics; resource discovery follows extensions, skills, prompts, themes, and package composition; session branching follows append-only tree semantics.
- Harness alignment: direct rows use `parent_spec_row = null`; composite rows are decomposed; declared behavioral tests are owned by the declaring task; cross-crate tests have cross-crate gates; snapshot tests require explicit approval; ledger remains gitignored.
