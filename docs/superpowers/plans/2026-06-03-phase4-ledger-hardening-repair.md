# Phase 4 Ledger Hardening Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining Phase 4 ledger audit gaps without changing Phase 4 task ordering or implementation scope.

**Architecture:** Treat `docs/opi-spec.md` as immutable and `.opi-impl-state.json` as gitignored runtime state. Apply structured JSON edits to narrow example-task ownership and strengthen public-surface definitions of done, then update the tracked `opi-implement` guardrails so future reinit work does not regenerate broad ownership.

**Tech Stack:** PowerShell structured JSON, Markdown skill references, git status verification.

---

## File Structure

- Modify: `.opi-impl-state.json`
  - Replace broad `docs/**` ownership on `4.8.1` through `4.8.6` with `docs/extension-examples/**`.
  - Add `definition_of_done` documentation requirements to public extensibility substrate tasks `4.1`, `4.4`, `4.6`, and `4.10`.
  - Add truthful `inference_notes` for each changed ledger field.
  - Never stage or commit this file.
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
  - Document that `task_owned_paths` must not include broad documentation globs such as `docs/**` when a narrower docs subtree can satisfy the task.
  - Document that `docs/opi-spec.md` is never task-owned.
- Modify: `.agents/skills/opi-implement/references/ledger-schema.md`
  - Keep byte-identical with the `.claude` copy.
- Modify: `.claude/skills/opi-implement/references/verification-tiers.md`
  - Add task-graph verification checks for broad docs ownership and public-surface documentation requirements.
- Modify: `.agents/skills/opi-implement/references/verification-tiers.md`
  - Keep byte-identical with the `.claude` copy.
- Modify: `docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md`
  - Record the ownership narrowing and public-surface DoD hardening so the original repair plan matches the repaired graph.
- Modify: `docs/superpowers/plans/2026-06-03-phase4-ledger-review-fixes.md`
  - Mark this hardening pass as the remaining follow-up if that file is kept as the review-fix plan.

## Target Ledger Changes

Use these exact semantic changes:

| Task | Change |
|---|---|
| `4.1` | Append RPC protocol documentation requirement to `definition_of_done`. |
| `4.4` | Append extension API documentation requirement to `definition_of_done`. |
| `4.6` | Append custom provider/model registration documentation requirement to `definition_of_done`. |
| `4.8.1`-`4.8.6` | Replace `docs/**` with `docs/extension-examples/**` in `task_owned_paths`. |
| `4.10` | Append streaming proxy/transport documentation requirement to `definition_of_done`. |

Use this inference note for every changed field:

```json
{
  "field": "<field-name>",
  "reason": "phase4 hardening narrows task-owned documentation paths and makes public extensibility surface documentation explicit without changing task ordering or implementation scope",
  "source": "docs/superpowers/plans/2026-06-03-phase4-ledger-hardening-repair.md"
}
```

## Task 1: Add Guardrails To Skill References

**Files:**
- Modify: `.claude/skills/opi-implement/references/ledger-schema.md`
- Modify: `.agents/skills/opi-implement/references/ledger-schema.md`
- Modify: `.claude/skills/opi-implement/references/verification-tiers.md`
- Modify: `.agents/skills/opi-implement/references/verification-tiers.md`

- [ ] **Step 1: Add the ledger-schema ownership rule**

In both `ledger-schema.md` copies, add this paragraph immediately after the existing `task_owned_paths` validation rules:

```markdown
Validation rule: `task_owned_paths` MUST NOT include broad documentation globs such as `docs/**` when a narrower subtree can satisfy the task. Use a purpose-specific path such as `docs/extension-examples/**` for example packages. `docs/opi-spec.md` is normative input and MUST NOT be task-owned.
```

- [ ] **Step 2: Add verification-tier graph checks**

In both `verification-tiers.md` copies, add these items to `## Task Graph Verification Checks` after the current item 5:

```markdown
6. Example/package tasks must not own `docs/**`; use a task-specific docs subtree such as `docs/extension-examples/**`.
7. Public protocol or extension substrate tasks must include documentation requirements in their DoD when they introduce RPC, SDK, extension, provider/model registration, transport, or proxy surfaces.
8. No task may include `docs/opi-spec.md` in `task_owned_paths`.
```

- [ ] **Step 3: Verify skill copies match**

Run:

```powershell
$paths = @(
  "references/ledger-schema.md",
  "references/verification-tiers.md"
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

## Task 2: Narrow Example Documentation Ownership

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

- [ ] **Step 2: Detect broad docs ownership before the fix**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
$rows = @()
foreach ($t in $j.tasks | Where-Object { $_.id -like "4.8.*" }) {
  if (@($t.task_owned_paths) -contains "docs/**") {
    $rows += "$($t.id) owns docs/**"
  }
}
$rows
```

Expected output before the fix:

```text
4.8.1 owns docs/**
4.8.2 owns docs/**
4.8.3 owns docs/**
4.8.4 owns docs/**
4.8.5 owns docs/**
4.8.6 owns docs/**
```

- [ ] **Step 3: Apply the structured JSON ownership patch**

Run:

```powershell
$path = ".\.opi-impl-state.json"
$tmp = ".\.opi-impl-state.json.tmp"
$j = Get-Content -Raw $path | ConvertFrom-Json
$reason = "phase4 hardening narrows task-owned documentation paths and makes public extensibility surface documentation explicit without changing task ordering or implementation scope"
$source = "docs/superpowers/plans/2026-06-03-phase4-ledger-hardening-repair.md"

foreach ($t in $j.tasks | Where-Object { $_.id -like "4.8.*" }) {
  $paths = @($t.task_owned_paths)
  if ($paths -contains "docs/**") {
    $paths = @($paths | Where-Object { $_ -ne "docs/**" })
    if ($paths -notcontains "docs/extension-examples/**") {
      $paths += "docs/extension-examples/**"
    }
    $t.task_owned_paths = $paths

    $hasNote = @($t.inference_notes | Where-Object {
      $_.field -eq "task_owned_paths" -and $_.source -eq $source
    }).Count -gt 0

    if (-not $hasNote) {
      $note = [pscustomobject]@{
        field = "task_owned_paths"
        reason = $reason
        source = $source
      }
      $t.inference_notes = @($t.inference_notes) + $note
    }
  }
}

$json = $j | ConvertTo-Json -Depth 100
[System.IO.File]::WriteAllText((Resolve-Path $tmp), $json + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Move-Item -Force $tmp $path
```

Expected result: command exits 0.

- [ ] **Step 4: Verify docs ownership is narrowed**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
foreach ($t in $j.tasks) {
  foreach ($owned in @($t.task_owned_paths)) {
    if ($owned -eq "docs/**") { throw "$($t.id) still owns docs/**" }
    if ($owned -eq "docs/opi-spec.md") { throw "$($t.id) owns docs/opi-spec.md" }
  }
}
foreach ($t in $j.tasks | Where-Object { $_.id -like "4.8.*" }) {
  if (@($t.task_owned_paths) -notcontains "docs/extension-examples/**") {
    throw "$($t.id) missing docs/extension-examples/**"
  }
}
"ok"
```

Expected output:

```text
ok
```

## Task 3: Harden Public-Surface Definitions Of Done

**Files:**
- Modify: `.opi-impl-state.json`

- [ ] **Step 1: Apply the structured JSON DoD patch**

Run:

```powershell
$path = ".\.opi-impl-state.json"
$tmp = ".\.opi-impl-state.json.tmp"
$j = Get-Content -Raw $path | ConvertFrom-Json
$reason = "phase4 hardening narrows task-owned documentation paths and makes public extensibility surface documentation explicit without changing task ordering or implementation scope"
$source = "docs/superpowers/plans/2026-06-03-phase4-ledger-hardening-repair.md"

$additions = @{
  "4.1" = " Public RPC command/event schema, error semantics, cancellation semantics, and unstable 0.x protocol status are documented in crate docs or user-facing docs, with localized counterparts updated when applicable."
  "4.4" = " Public extension API docs describe lifecycle ordering, hook error/blocking semantics, state serialization, custom tool/command/message contracts, and unstable 0.x status."
  "4.6" = " Public provider/model registration docs cover capability declaration, duplicate/invalid registration behavior, --list-models integration, streaming contract expectations, and the rule that provider breadth should arrive through registration rather than core provider additions."
  "4.10" = " Public transport/proxy docs cover framing, backpressure, cancellation, client disconnect behavior, secret redaction expectations, and unstable 0.x status."
}

foreach ($id in $additions.Keys) {
  $task = @($j.tasks | Where-Object { $_.id -eq $id })[0]
  if ($null -eq $task) { throw "missing task $id" }

  $addition = $additions[$id]
  if ($task.definition_of_done -notlike "*$($addition.Trim())*") {
    $task.definition_of_done = $task.definition_of_done.TrimEnd() + $addition
  }

  $hasNote = @($task.inference_notes | Where-Object {
    $_.field -eq "definition_of_done" -and $_.source -eq $source
  }).Count -gt 0

  if (-not $hasNote) {
    $note = [pscustomobject]@{
      field = "definition_of_done"
      reason = $reason
      source = $source
    }
    $task.inference_notes = @($task.inference_notes) + $note
  }
}

$json = $j | ConvertTo-Json -Depth 100
[System.IO.File]::WriteAllText((Resolve-Path $tmp), $json + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
Move-Item -Force $tmp $path
```

Expected result: command exits 0.

- [ ] **Step 2: Verify the DoD additions are present**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
$checks = @{
  "4.1" = "Public RPC command/event schema"
  "4.4" = "Public extension API docs"
  "4.6" = "Public provider/model registration docs"
  "4.10" = "Public transport/proxy docs"
}
foreach ($id in $checks.Keys) {
  $task = @($j.tasks | Where-Object { $_.id -eq $id })[0]
  if ($task.definition_of_done -notlike "*$($checks[$id])*") {
    throw "$id missing DoD text: $($checks[$id])"
  }
  $hasNote = @($task.inference_notes | Where-Object {
    $_.field -eq "definition_of_done" -and $_.source -eq "docs/superpowers/plans/2026-06-03-phase4-ledger-hardening-repair.md"
  }).Count -gt 0
  if (-not $hasNote) { throw "$id missing hardening inference note" }
}
"ok"
```

Expected output:

```text
ok
```

## Task 4: Update Repair Plan Documentation

**Files:**
- Modify: `docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md`
- Modify: `docs/superpowers/plans/2026-06-03-phase4-ledger-review-fixes.md`

- [ ] **Step 1: Add the hardening summary to the original repair plan**

In `docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md`, add this row to the `Target Graph Decisions` table:

```markdown
| Hardening follow-up | Example/package tasks use `docs/extension-examples/**` instead of broad `docs/**`; public RPC, extension, provider/model registration, and transport/proxy tasks have explicit documentation requirements in their DoD. |
```

- [ ] **Step 2: Add this plan to the review-fixes plan**

In `docs/superpowers/plans/2026-06-03-phase4-ledger-review-fixes.md`, add this sentence under `## Self-Review`:

```markdown
- Follow-up hardening is tracked in `docs/superpowers/plans/2026-06-03-phase4-ledger-hardening-repair.md`; it narrows `4.8.*` docs ownership and makes public extensibility documentation requirements explicit.
```

- [ ] **Step 3: Verify both docs mention the hardening**

Run:

```powershell
Select-String -Path .\docs\superpowers\plans\2026-06-03-phase4-ledger-repair.md -Pattern "docs/extension-examples"
Select-String -Path .\docs\superpowers\plans\2026-06-03-phase4-ledger-review-fixes.md -Pattern "phase4-ledger-hardening-repair"
```

Expected result: both commands print at least one match.

## Task 5: Validate The Repaired Ledger And Guardrails

**Files:**
- Read: `.opi-impl-state.json`
- Read: `docs/opi-spec.md`
- Read: `.claude/skills/opi-implement/references/ledger-schema.md`
- Read: `.agents/skills/opi-implement/references/ledger-schema.md`
- Read: `.claude/skills/opi-implement/references/verification-tiers.md`
- Read: `.agents/skills/opi-implement/references/verification-tiers.md`

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

- [ ] **Step 2: Verify graph shape and dependencies**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
$active = @{}
foreach ($t in $j.tasks) {
  if ($active.ContainsKey($t.id)) { throw "duplicate id $($t.id)" }
  $active[$t.id] = $t
}
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

- [ ] **Step 3: Verify behavioral tests remain owned**

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

- [ ] **Step 4: Verify hardening-specific invariants**

Run:

```powershell
$j = Get-Content -Raw .\.opi-impl-state.json | ConvertFrom-Json
foreach ($t in $j.tasks) {
  foreach ($owned in @($t.task_owned_paths)) {
    if ($owned -eq "docs/**") { throw "$($t.id) owns broad docs/**" }
    if ($owned -eq "docs/opi-spec.md") { throw "$($t.id) owns docs/opi-spec.md" }
  }
}
foreach ($id in "4.1","4.4","4.6","4.10") {
  $task = @($j.tasks | Where-Object { $_.id -eq $id })[0]
  if ($task.definition_of_done -notmatch "docs|documentation") {
    throw "$id DoD does not mention docs/documentation"
  }
}
"ok"
```

Expected output:

```text
ok
```

- [ ] **Step 5: Verify skill copies match**

Run:

```powershell
$paths = @(
  "skill.md",
  "references/initializer.md",
  "references/ledger-schema.md",
  "references/verification-tiers.md",
  "references/anti-patterns.md",
  "references/failure-gate.md"
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

- [ ] **Step 6: Confirm ledger remains unstaged**

Run:

```powershell
git status --short
```

Expected result: no line begins with `A  .opi-impl-state.json`, `M  .opi-impl-state.json`, or `?? .opi-impl-state.json`.

## Commit Guidance

Do not commit unless the user explicitly asks. If committing this hardening repair, stage only tracked files intentionally changed by this plan:

```powershell
git add docs/superpowers/plans/2026-06-03-phase4-ledger-hardening-repair.md
git add docs/superpowers/plans/2026-06-03-phase4-ledger-repair.md
git add docs/superpowers/plans/2026-06-03-phase4-ledger-review-fixes.md
git add .claude/skills/opi-implement/references/ledger-schema.md
git add .agents/skills/opi-implement/references/ledger-schema.md
git add .claude/skills/opi-implement/references/verification-tiers.md
git add .agents/skills/opi-implement/references/verification-tiers.md
```

If earlier Phase 4 ledger-rule changes are still uncommitted, keep their staging separate or explicitly include only the already-reviewed tracked files from that repair. Never stage `.opi-impl-state.json`.

Suggested commit message after validation:

```text
chore: harden phase 4 opi-implement ledger rules
```

## Self-Review

- Spec coverage: The plan preserves Phase 4 ordering and scope while tightening the third-party extensibility contract through explicit documentation requirements.
- Harness coverage: Runtime ledger edits are structured JSON writes; broad docs ownership is banned; `docs/opi-spec.md` remains immutable input.
- Verification coverage: The plan checks spec hash, dependencies, behavioral-test ownership, hardening invariants, skill-copy parity, and unstaged ledger state.
- Placeholder scan: No steps rely on placeholder markers or unspecified validation.
