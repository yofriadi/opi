# Phase 4 Ledger Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining audit gaps in the Phase 4 ledger repair without changing the Phase 4 task graph semantics.

**Architecture:** Treat `.opi-impl-state.json` as gitignored runtime state and update it only with structured JSON operations. Treat `docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md` as the tracked repair plan that must accurately describe validation and commit guidance for the already-modified skill files.

**Tech Stack:** PowerShell structured JSON, Markdown plan docs, git status checks.

---

## File Structure

- Modify: `.opi-impl-state.json`
  - Add missing `parent_spec_row` inference notes for `4.8.1` through `4.8.6`.
  - Do not stage or commit this file.
- Modify: `docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md`
  - Add snapshot-test validation to the repaired graph checks.
  - Add `.agents/skills/opi-implement/skill.md` to commit guidance because it is an intentional tracked change.
- Read: `.agents/skills/opi-implement/skill.md`
  - Confirm the archived phase dependency wording is the tracked change being added to commit guidance.
- Read: `.claude/skills/opi-implement/skill.md`
  - Confirm skill copies still match after the repair plan guidance is corrected.

### Task 1: Patch 4.8 Parent-Row Audit Notes

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

- [ ] **Step 2: Detect the missing notes**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
foreach ($t in $j.tasks | Where-Object { $_.id -like "4.8.*" }) {
  $hasNote = @($t.inference_notes | Where-Object { $_.field -eq "parent_spec_row" }).Count -gt 0
  if (-not $hasNote) { "$($t.id) missing parent_spec_row note" }
}
```

Expected output before the fix:

```text
4.8.1 missing parent_spec_row note
4.8.2 missing parent_spec_row note
4.8.3 missing parent_spec_row note
4.8.4 missing parent_spec_row note
4.8.5 missing parent_spec_row note
4.8.6 missing parent_spec_row note
```

- [ ] **Step 3: Add the notes with a structured JSON write**

Run:

```powershell
$path = ".\.opi-impl-state.json"
$tmp = ".\.opi-impl-state.json.tmp"
$j = Get-Content -Raw $path | ConvertFrom-Json
$reason = "phase4 graph repair aligned with opi-spec.md Phase 4 substrate ordering, pi 0.75.3 resource/extension semantics, and opi-implement validation rules"
$source = "docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md"

foreach ($t in $j.tasks | Where-Object { $_.id -like "4.8.*" }) {
  if ($t.parent_spec_row -ne "4.8") {
    throw "$($t.id) parent_spec_row is '$($t.parent_spec_row)', expected '4.8'"
  }

  $hasNote = @($t.inference_notes | Where-Object {
    $_.field -eq "parent_spec_row" -and $_.source -eq $source
  }).Count -gt 0

  if (-not $hasNote) {
    $note = [pscustomobject]@{
      field = "parent_spec_row"
      reason = $reason
      source = $source
    }
    $t.inference_notes = @($t.inference_notes) + $note
  }
}

$json = $j | ConvertTo-Json -Depth 100
[System.IO.File]::WriteAllText((Resolve-Path $tmp), $json + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Move-Item -Force $tmp $path
```

Expected result: command exits 0.

- [ ] **Step 4: Verify the notes are present**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
foreach ($t in $j.tasks | Where-Object { $_.id -like "4.8.*" }) {
  $hasNote = @($t.inference_notes | Where-Object {
    $_.field -eq "parent_spec_row" -and $_.source -eq "docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md"
  }).Count -gt 0
  if (-not $hasNote) { throw "$($t.id) still missing parent_spec_row note" }
}
"ok"
```

Expected output:

```text
ok
```

### Task 2: Correct the Repair Plan Validation and Commit Guidance

**Files:**
- Modify: `docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md`
- Read: `.agents/skills/opi-implement/skill.md`

- [ ] **Step 1: Confirm the tracked skill.md diff exists**

Run:

```powershell
git diff -- .agents/skills/opi-implement/skill.md
```

Expected output includes:

```text
phase_exit[*].task_summary
```

- [ ] **Step 2: Add snapshot validation after Task 5 Step 4**

In `docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md`, add this new step immediately after the existing Task 5 Step 4 behavioral-test ownership check:

````markdown
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
````

- [ ] **Step 3: Add `.agents/skills/opi-implement/skill.md` to commit guidance**

In the same file, update the commit guidance command block so it includes:

```powershell
git add .agents/skills/opi-implement/skill.md
```

Place it after:

```powershell
git add docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md
```

- [ ] **Step 4: Verify the repair plan now mentions both fixes**

Run:

```powershell
Select-String -Path .\docs\superpowers\plans\2026-06-03-phase4-ledger-repair.md -Pattern "has TUI behavioral tests but no snapshot_tests|git add \.agents/skills/opi-implement/skill\.md"
```

Expected output includes one match for each pattern.

### Task 3: Re-run Graph and Skill Consistency Checks

**Files:**
- Read: `.opi-impl-state.json`
- Read: `.claude/skills/opi-implement/skill.md`
- Read: `.agents/skills/opi-implement/skill.md`
- Read: `.claude/skills/opi-implement/references/*.md`
- Read: `.agents/skills/opi-implement/references/*.md`

- [ ] **Step 1: Verify spec hash still matches**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
$actual = (Get-FileHash .\docs\opi-spec.md -Algorithm SHA256).Hash.ToLowerInvariant()
$ledger = $j.spec_files_sha256.'docs/opi-spec.md'
if ($actual -ne $ledger) { throw "spec hash mismatch: $actual != $ledger" }
"ok"
```

Expected output:

```text
ok
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

- [ ] **Step 4: Verify behavioral tests and snapshot tests**

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

- [ ] **Step 5: Confirm all skill reference pairs match**

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

- [ ] **Step 6: Confirm the ledger is not staged**

Run:

```powershell
git status --short
```

Expected result:

```text
No line beginning with "A  .opi-impl-state.json", "M  .opi-impl-state.json", or "?? .opi-impl-state.json".
```

## Commit Guidance

Do not commit unless the user explicitly asks. If committing this repair, stage only tracked files intentionally changed by the repair:

```powershell
git add docs/superpowers/plans/2026-06-03-phase4-ledger-review-fixes.md
git add docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md
git add .agents/skills/opi-implement/skill.md
```

If the earlier Phase 4 ledger rule changes are still uncommitted, also stage only their tracked files:

```powershell
git add .claude/skills/opi-implement/references/initializer.md
git add .agents/skills/opi-implement/references/initializer.md
git add .claude/skills/opi-implement/references/ledger-schema.md
git add .agents/skills/opi-implement/references/ledger-schema.md
git add .claude/skills/opi-implement/references/verification-tiers.md
git add .agents/skills/opi-implement/references/verification-tiers.md
```

Never stage `.opi-impl-state.json`.

Suggested commit message after validation:

```text
chore: tighten phase 4 opi-implement ledger rules
```

## Self-Review

- Follow-up hardening is tracked in `docs/superpowers/plans/2026-06-03-phase4-ledger-hardening-repair.md`; it narrows `4.8.*` docs ownership and makes public extensibility documentation requirements explicit.
- Spec coverage: The plan does not reinterpret Phase 4 scope; it only repairs audit metadata and validation/commit guidance around the already-confirmed graph.
- Harness coverage: Runtime ledger edits are structured JSON writes; gitignored ledger state remains unstaged; tracked skill and plan files have explicit staging guidance.
- Validation coverage: The existing graph checks are retained and extended with the missing TUI snapshot rule.
