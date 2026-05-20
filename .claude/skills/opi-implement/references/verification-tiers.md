# Verification Tiers Reference

Each task carries a `tier` field; the skill selects gates from this table.
All tiers also run the cross-cutting gates at the bottom.

## `workspace` Tier

Tasks: 1.0 (deps), 1.17 (integration harness), any `crate: workspace` task.

Gates:
1. `cargo fmt --check --all`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-targets`
4. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
5. Smoke script runs

## `library` Tier

Tasks: 1.1–1.8 (`opi-ai`, `opi-agent` internals).

Gates:
1. TDD red→green produced new/changed tests in `crates/<crate>/tests/` OR
   `#[cfg(test)]` modules. Verify via diff content inspection (not just stat).
2. `cargo test -p <crate>` green
3. `cargo clippy -p <crate> -- -D warnings` green
4. `cargo doc -p <crate> -- -D warnings` green
5. `cargo build --workspace` green (catches breaking-API changes)
6. No `unwrap`/`expect` in non-test code (grep check)

## `cli-tool` Tier

Tasks: 1.9 (read/write/edit/bash), 1.10 (glob/grep).

Gates: All `library` gates, plus:
1. Behavioral tests in `crates/opi-coding-agent/tests/` using `tempfile` crate
2. For `bash`: tests for timeout, cwd capture, cancellation
3. For mutating tools: test asserting Phase-1 safety boundary is reported
   before execution (per opi-spec §8.4)

## `cli-runtime` Tier

Tasks: 1.11 (system prompt), 1.14 (interactive), 1.15 (non-interactive), 1.16 (config).

Gates: All `library` gates, plus:
1. E2E test booting `MockProvider` + `opi` binary subprocess with scripted prompts
2. Assertions on stdout, stderr, and exit code

**MockProvider precondition:** REFUSE to run if no `MockProvider` symbol exists.
Grep `crates/opi-ai/src/test_support.rs` (or feature-gated path). If absent:
> "Task `<id>` depends on MockProvider scaffolding (task 1.17). Run 1.17 first."

## `tui` Tier

Tasks: 1.12 (TUI shell), 1.13 (markdown/code rendering).

Gates: All `library` gates, plus:
1. Ratatui snapshot tests at fixed sizes (80×24 and 120×40) using `insta`
2. Snapshot diffs require explicit user approval — NEVER auto-accept

## Cross-Cutting Gates (Every Tier)

Run after tier-specific gates:

1. `cargo fmt --check --all` exits 0
2. `cargo clippy --workspace --all-targets -- -D warnings` exits 0
3. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` exits 0
4. `bash scripts/opi-impl-smoke.sh` (or `.ps1` on Windows) exits 0
5. `git status --porcelain --untracked-files=all` contains only intentional
   task files. Stage only reviewed files; never stage/clean unrelated changes.
6. Pre-commit: `HEAD` must equal `tasks[].start_commit`. If intermediate manual
   commit exists → refuse, require `--resume-from-manual`.
7. Post-commit: `git status --porcelain` clean; `HEAD^` equals `start_commit`.
8. Commit message includes `Opi-*` evidence footers.

### `--resume-from-manual`

Skip commit creation only if:
- Exactly one candidate manual commit since `start_commit`
- Working tree clean
- Phase D passes
- Commit already contains `Opi-*` footers

If footer missing: print required footer text and stop (do NOT amend).

## Risk Evaluator Gate

A task has `evaluator_required = true` when ANY of:
- Tier is `cli-runtime` or `tui`
- Task touches multiple crates or public protocol/data model
- Task changes tool safety, permissions, config, session storage, JSON framing,
  provider events, or release-critical behavior

`evaluator_required` is static (confirmed at init). Phase D MUST NOT dynamically
promote a task. Phase-exit evaluation is separate (Phase F).

The evaluator receives: DoD, diff from `start_commit`, new/changed tests,
verification outputs, planned commit evidence. It answers:
1. Does diff satisfy DoD without scope creep?
2. Do tests exercise behavior (not just implementation details)?
3. Public API/protocol/security risks not covered by mechanical gates?
4. Is evidence footer truthful and sufficient?

If evaluator fails → back to Phase C with findings as input. Generator may NOT
self-approve the finding away.
