# Phase 6 Audit Baseline

**Audited workspace version:** `0.5.1`
**Audit baseline commit:** `693c2e7` — `docs(workspace): synchronize current-state docs to 0.5.1`
**Baseline date:** 2026-06-16

## Purpose

This baseline classifies every finding from the three Phase 5 audits against
the `0.5.1` codebase so a maintainer can trace each Phase 5 audit observation
to a single, current disposition. It is a point-in-time record: it does not
re-litigate the Phase 5 exit, it implements nothing, and it makes no future
commitments.

Every finding is classified into exactly one of:

- **Closed by 0.5.1** — already resolved in the current workspace.
- **Accepted design difference** — a deliberate `0.x` scope decision, not a defect.
- **Phase 6 task** — owned by an open Phase 6 task (`6.3`–`6.6`).
- **Future ecosystem candidate** — deferred beyond Phase 6, non-committal.

## Inputs (immutable)

These Phase 5 audit files are read-only historical records. They are
**immutable** inputs and must not be edited, rewritten, or deleted by Phase 6:

- `docs/snapshots/phase5/audit.codex.md`
- `docs/snapshots/phase5/audit.glm5.1.md`
- `docs/snapshots/phase5/audit.opus4.6.md`

## Audit verdict reconciliation

The three audits disagree on Phase 5 completeness:

- **GLM-5.1** (`audit.glm5.1.md`): Phase 5 complete and auditable; all 12 design
  success criteria marked met; 14 deferred evaluator notes, none DoD-blocking.
- **Codex** (`audit.codex.md`) and **Opus 4.6** (`audit.opus4.6.md`): substrate
  complete, **product loop incomplete** — installed packages are not connected
  to runtime startup, the package CLI is declaration-only, adapter state does
  not survive restart, and two of four design-specified adapter hooks are absent.

opi position: both views are accurate against their respective references. The
GLM-5.1 audit measured Phase 5 against its confirmed ledger definitions of done,
which the task graph recorded as substrate-complete. The Codex and Opus audits
measured Phase 5 against the original design's full MVP loop and correctly found
that the end-to-end product path (`opi package add` -> restart -> a resolved
adapter registered in the runtime) is not wired in production. Phase 5 was
exited as substrate-complete; the missing product-loop wiring plus lifecycle and
diagnostics hardening were deferred to Phase 6 tasks `6.3`, `6.4`, and `6.5`.
None of those tasks is passing yet, so the contested findings remain **open**,
not closed.

## Finding classification

### Closed by 0.5.1

| Finding | Source audit | Closure evidence |
|---|---|---|
| "Packages are trusted code" not stated in user docs | GLM-5.1 §7.2 | `docs_warn_packages_are_trusted_code` guard enforces the trusted-code / not-sandboxed warning in README and opi-spec (EN + ZH). |
| Stale current-version references (`0.4.0` / `0.5.0` describing the current implementation) | remediation plan task 1; GLM-5.1 §8.1 | Phase 6 task `6.1` synchronized current-state docs to `0.5.1` while preserving historical release rows. |
| CHANGELOG missing Phase 5 entries | Opus 4.6 P2-4 | `changelog_mentions_phase_five_package_loop` guard enforces the package CLI, `opi-extension-jsonl-v1`, and adapter state snapshot entries. |

### Accepted design difference

| Finding | Source audit | Rationale |
|---|---|---|
| Adapter hooks limited to `before_tool_call` / `after_tool_call` + events; `prepare_next_turn` and `transform_context` not implemented | Codex P1; Opus 4.6 P0-3 | The `0.x` `opi-extension-jsonl-v1` protocol documents the observed hook set. The design's four-hook MVP surface is narrowed for `0.x`; task `6.3` codifies the observed contract. Full four-hook support is a future enhancement, not a Phase 6 deliverable. |
| Relative adapter command may resolve outside the package root (`..` escape) | Codex P2; Opus 4.6 P2-3 | Packages are trusted code running with full user privileges and are not sandboxed; the escape is surprising but tolerated under that model and documented. |
| `model_overrides.tools` parsed but ignored; unbounded `on_event` tasks; registration errors reported as strings | GLM-5.1 §9 (deferred #4, #5, #10) | Accepted `0.x` simplifications recorded in evaluator notes. |
| Documentation/code naming drift (`tool` vs `tool_call`/`tool_result`; `ProcessAdapterHooks` absent; `list --json` vs `doctor --json` shapes) | Opus 4.6 P2-5; GLM-5.1 §9 (#3) | Accepted `0.x` naming and output inconsistencies. |

### Phase 6 task

| Finding | Source audit | Owning task | Status |
|---|---|---|---|
| Installed package declarations are not connected to runtime startup (`packages.toml` not read; `start_adapters_from_packages` not called in production) | Codex P0; Opus 4.6 P0-1 | `6.4` Package runtime path and degraded diagnostics hardening | open |
| Package CLI is declaration-only (`add` does not validate/clone/lock; `remove` by source only; `list`/`doctor` project scope only; `doctor` shallow) | Codex P0; Opus 4.6 P0-2 | `6.4` | open |
| Local package identity not canonicalized (duplicate declaration risk) | Codex P2; Opus 4.6 P2-1 | `6.4` | open |
| Adapter state does not survive restart through session JSONL persistence | Codex P1; Opus 4.6 P1-1 | `6.5` Session and RPC adapter boundary hardening | open |
| Adapter startup diagnostics not surfaced in RPC / production startup | Codex P1 | `6.5` | open |
| Shutdown kills the child without a graceful exit window | Opus 4.6 P1-2 | `6.3` Adapter protocol contract hardening | open |
| Event-drop diagnostics absent (silent drop on backpressure) | Codex P1; Opus 4.6 P1-3 | `6.3` | open |

### Future ecosystem candidate

These are deliberately out of scope for Phase 6. They are **non-committal**
candidates for future prioritization, **not committed next-phase scope**, and
none is implemented:

- Package **enable/disable** at runtime.
- Package **update** command (lock refresh / source re-resolve).
- Source filters and `package info` / richer `list` metadata.
- **npm** / registry-backed package sources and package marketplace / gallery metadata.
- Provider **OAuth** flows and interactive provider auth.
- Additional provider breadth and OpenAI-compatible profile hardening.
- Stronger package trust model / **sandboxing** / permission enforcement.
- Browser-based **web-ui** product surface (distinct from pi-web-ui parity, which remains a non-goal).
- Session import / **migration** and pi session compatibility.
- Extension/RPC UI sub-protocol and adapter UI surfaces.

## Notes

- SSH git package sources (`git:ssh://`, `git@`) are not parseable today
  (GLM-5.1 §7.3, Codex P2, Opus 4.6 P2-2). HTTPS is the supported git transport;
  SSH is a future ecosystem candidate, not Phase 6 work.
- This baseline is a single English maintainer artifact, matching the Phase 5
  audit convention; it is not localized user-facing documentation.
