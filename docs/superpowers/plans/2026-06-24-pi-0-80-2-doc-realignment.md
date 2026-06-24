# pi 0.80.2 Documentation Realignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebase `opi` planning documents from the stale `.repo/pi-0.75.3` comparison to `.repo/pi-0.80.2`, embed durable evidence in the alignment matrix, and reshape Phase 9-14 around deepening existing Rust-native capabilities before ecosystem expansion.

**Architecture:** This is a documentation-only implementation. The baseline evidence lives inside `pi-alignment-matrix`, while `opi-spec` points to that matrix as the durable evidence and alignment surface. Existing Phase 9-12 design files remain in place for history but are visibly recast as new Phase 11-14 work.

**Tech Stack:** Markdown documentation, local `.repo/pi-0.80.2` evidence, PowerShell verification with `rg`.

## Global Constraints

- Do not change Rust code or runtime behavior.
- Do not commit unless the user explicitly asks.
- Keep English and Chinese normative docs synchronized.
- Preserve evidence paths and line anchors for `.repo/pi-0.80.2`.
- Do not claim parity for OAuth, image generation, custom extension UI, npm/gallery/update, web/share, or `pi` session compatibility.
- Do not introduce a shared `opi-types` crate.
- Treat ecosystem breadth as future candidates with entry conditions.

---

### Task 1: Durable Evidence Baseline in the Alignment Matrix

**Files:**
- Modify: `docs/pi-alignment-matrix.md`

**Interfaces:**
- Consumes: `docs/superpowers/specs/2026-06-24-phase9-pi-0-80-2-baseline-realignment-design.md`
- Produces: Stable evidence anchors for `docs/opi-spec*.md` and `docs/pi-alignment-matrix*.md`

- [ ] **Step 1: Embed the evidence baseline**

Update `docs/pi-alignment-matrix.md` with these sections:

```markdown
## Document Control
## Executive Summary
## Pi Architecture
## Version Evolution Signals
## Evidence Index
## Opi Alignment Dashboard
## Roadmap Implications
## Maintenance Rules
```

Include evidence rows for `Models/Auth`, `AgentHarness`, session durability, extension UI breadth, provider hooks, and session hooks using local line anchors from `.repo/pi-0.80.2`.

- [ ] **Step 2: Verify no unsupported parity claims**

Run:

```powershell
rg -n "OAuth parity|image generation.*complete|custom extension UI parity|npm gallery parity|web/share parity|pi session compatibility" docs/pi-alignment-matrix.md
```

Expected: no matches.

### Task 2: Normative Spec Synchronization

**Files:**
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`

**Interfaces:**
- Consumes: `docs/pi-alignment-matrix.md`
- Produces: Current baseline and roadmap statements for project scope questions

- [ ] **Step 1: Update baseline and roadmap**

In both language variants:

- Change the current studied upstream baseline from `.repo/pi-0.75.3` to `.repo/pi-0.80.2`.
- Link `docs/pi-alignment-matrix.md` as the durable evidence baseline.
- Add the revised Phase 9-14 roadmap.
- Clarify that generic harness primitives belong in `opi-agent`, while coding-agent product behavior remains in `opi-coding-agent`.
- Clarify future ecosystem candidates and entry conditions.

- [ ] **Step 2: Verify synchronization-critical phrases**

Run:

```powershell
rg -n "pi-0\\.80\\.2|Phase 9|Phase 10|Phase 14|ecosystem candidates|AgentHarness|Models/Auth" docs/opi-spec.md docs/opi-spec.zh.md
```

Expected: both files contain equivalent baseline, roadmap, and architecture seam statements.

### Task 3: Alignment Matrix Rebase

**Files:**
- Modify: `docs/pi-alignment-matrix.md`
- Modify: `docs/pi-alignment-matrix.zh.md`

**Interfaces:**
- Consumes: embedded evidence sections in `docs/pi-alignment-matrix.md`
- Produces: Package-level and feature-level comparison against current upstream

- [ ] **Step 1: Rebase comparison**

In both language variants:

- Change the comparison baseline to `.repo/pi-0.80.2`.
- Add the three-layer dashboard: core semantic parity, product parity, ecosystem parity.
- Update package rows so `opi-agent` is Partial until a Rust-native generic harness seam exists.
- Keep `opi-ai` close but Partial because `Models/Auth`, OAuth, image generation, and broad catalog are not complete.
- Keep `opi-tui` Partial because custom renderer/component breadth remains larger in `pi`.
- Keep `opi-coding-agent` Partial because extension UI, provider hooks, web/share, and npm/gallery breadth are future candidates.

- [ ] **Step 2: Verify package and phase rows**

Run:

```powershell
rg -n "opi-ai|opi-agent|opi-tui|opi-coding-agent|Core semantic parity|Product parity|Ecosystem parity|Phase 9|Phase 14" docs/pi-alignment-matrix.md docs/pi-alignment-matrix.zh.md
```

Expected: package rows and dashboards are present in both files.

### Task 4: Recast Existing Phase 9-12 Specs

**Files:**
- Modify: `docs/superpowers/specs/2026-06-24-phase11-tooling-quality-design.md`
- Modify: `docs/superpowers/specs/2026-06-24-phase12-provider-correctness-design.md`
- Modify: `docs/superpowers/specs/2026-06-24-phase13-session-tree-context-reconstruction-design.md`
- Modify: `docs/superpowers/specs/2026-06-24-phase14-tui-product-polish-design.md`

**Interfaces:**
- Consumes: new Phase 9 and Phase 10 design docs
- Produces: Historical phase design files whose visible scope matches the revised roadmap

- [ ] **Step 1: Update visible phase identity**

Apply these recasts:

- Old Phase 9 tooling quality becomes new Phase 11 tooling quality and depends on Phase 10.
- Old Phase 10 provider correctness becomes new Phase 12 provider correctness and depends on the `Models/Auth` seam.
- Old Phase 11 session long-term memory becomes new Phase 13 session tree and context reconstruction and depends on the generic harness/session facade.
- Old Phase 12 TUI product polish becomes new Phase 14 TUI product polish and excludes custom extension UI parity.

- [ ] **Step 2: Verify numbering**

Run:

```powershell
rg -n "Phase 9|Phase 10|Phase 11|Phase 12|Phase 13|Phase 14|Models/Auth|AgentHarness|custom extension UI" docs/superpowers/specs/2026-06-24-phase11-tooling-quality-design.md docs/superpowers/specs/2026-06-24-phase12-provider-correctness-design.md docs/superpowers/specs/2026-06-24-phase13-session-tree-context-reconstruction-design.md docs/superpowers/specs/2026-06-24-phase14-tui-product-polish-design.md
```

Expected: old numbers appear only as historical notes or dependencies; new visible phase numbers are Phase 11-14.

### Task 5: Final Documentation Guards

**Files:**
- Verify all files changed by this plan

**Interfaces:**
- Consumes: Tasks 1-4
- Produces: evidence for final status report

- [ ] **Step 1: Verify stale baseline wording**

Run:

```powershell
rg -n "\\.repo/pi-0\\.75\\.3|pi-0\\.75\\.3" docs/opi-spec.md docs/opi-spec.zh.md docs/pi-alignment-matrix.md docs/pi-alignment-matrix.zh.md
```

Expected: matches only in historical notes that explain the prior baseline, not as current baseline.

- [ ] **Step 2: Verify no placeholders**

Run:

```powershell
rg -n "TB[D]|TO[D]O|FIXM[E]" docs/opi-spec.md docs/opi-spec.zh.md docs/pi-alignment-matrix.md docs/pi-alignment-matrix.zh.md docs/superpowers/specs/2026-06-24-phase9-pi-0-80-2-baseline-realignment-design.md docs/superpowers/specs/2026-06-24-phase10-core-architecture-deepening-design.md
```

Expected: no matches.

- [ ] **Step 3: Verify changed-file scope**

Run:

```powershell
git status --short
```

Expected: only documentation files from this plan and the two approved design specs are changed or added.
