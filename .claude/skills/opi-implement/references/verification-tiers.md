# Verification Tiers Reference

Each task carries a `tier` field; the skill selects gates from this table.
All tiers also run the cross-cutting gates at the bottom.

## `workspace` Tier

Use for dependency graph changes, cross-crate integration harnesses, and tasks
whose primary crate is `workspace` or `cross-crate`.

Gates:
1. `cargo fmt --check --all`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-targets`
4. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
5. Smoke script runs

## `library` Tier

Use for focused `opi-ai`, `opi-agent`, or `opi-tui` library changes that do not
add provider wire formats, CLI runtime behavior, or visual snapshot surfaces.

Gates:
1. TDD red→green produced new/changed tests in `crates/<crate>/tests/` OR
   `#[cfg(test)]` modules. Verify via diff content inspection (not just stat).
2. `cargo test -p <crate>` green
3. `cargo clippy -p <crate> -- -D warnings` green
4. Docs with warnings denied green:
   - Unix shell: `RUSTDOCFLAGS="-D warnings" cargo doc -p <crate> --no-deps`
   - PowerShell: `$env:RUSTDOCFLAGS="-D warnings"; cargo doc -p <crate> --no-deps; Remove-Item Env:RUSTDOCFLAGS`
5. `cargo build --workspace` green (catches breaking-API changes)
6. No `unwrap`/`expect` in non-test code (grep check)

## `cli-tool` Tier

Use for built-in tools such as `read`, `write`, `edit`, `bash`, `glob`, `grep`,
`find`, and `ls`.

Gates: All `library` gates, plus:
1. Behavioral tests in `crates/opi-coding-agent/tests/` using `tempfile` crate
2. For `bash`: tests for timeout, cwd capture, cancellation
3. For mutating tools: test asserting Phase-1 safety boundary is reported
   before execution (per opi-spec §8.4)

## `cli-runtime` Tier

Use for CLI parsing, config, prompt/context loading, session commands, JSON
mode, tool selection flags, shell completions, and binary subprocess behavior.

Gates: All `library` gates, plus:
1. E2E test booting `MockProvider` + `opi` binary subprocess with scripted prompts
2. Assertions on stdout, stderr, and exit code

**MockProvider precondition:** REFUSE to run if no `MockProvider` symbol exists.
Grep `crates/opi-ai/src/test_support.rs` (or feature-gated path). If absent:
> "Task `<id>` depends on MockProvider scaffolding (task 1.17). Run 1.17 first."

## `tui` Tier

Use for ratatui rendering, keybindings, themes, fuzzy pickers, diff rendering,
terminal image rendering, and snapshot surfaces.

Gates: All `library` gates, plus:
1. Ratatui snapshot tests at fixed sizes (80×24 and 120×40) using `insta`
2. Snapshot diffs require explicit user approval — NEVER auto-accept

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
1. Serialization tests cover image metadata, MIME type, size limits, and
   provider capability rejection.
2. Tool-result tests cover text-only fallback and non-UTF-8/binary-safe handling.
3. TUI tests use deterministic snapshots or golden terminal protocol output; no
   visual snapshot is accepted without explicit user approval.

## Cross-Cutting Gates (Every Tier)

Run after tier-specific gates:

1. `cargo fmt --check --all` exits 0
2. `cargo clippy --workspace --all-targets -- -D warnings` exits 0
3. `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` exits 0
4. `bash scripts/opi-impl-smoke.sh` (or `.ps1` on Windows) exits 0
5. Capture `baseline_dirty_files` at Phase B before implementation starts.
6. Before commit-stage, every entry in
   `git status --porcelain --untracked-files=all` MUST satisfy ONE of:
   - present in `baseline_dirty_files` AND unchanged by this task AND not
     matched by `task_owned_paths` (untouched baseline, leave alone);
   - matched by `task_owned_paths` (intentional task file, will be staged);
   - matched by `task_owned_paths` AND also present in `baseline_dirty_files`
     → REFUSE; print the overlap and ask the user to either split the file
     manually or explicitly confirm the baseline edit is task-owned.
7. Stage only paths matched by `task_owned_paths` AND changed since
   `start_commit`. Never use `git add -A` or `git add .`.
8. Pre-commit: `HEAD` must equal `tasks[].start_commit` unless the only new
   commit is a reviewed manual task commit handled by `--resume-from-manual`.
9. Post-commit: `HEAD^` must equal `start_commit`; no path matched by
   `task_owned_paths` may remain dirty. Files in `baseline_dirty_files` that
   were not modified by the task remain as-is.
10. Commit message includes `Opi-*` evidence footers.

### `--resume-from-manual`

Skip commit creation only if:
- Exactly one candidate manual commit since `start_commit`
- No task-owned dirty files remain outside the candidate manual commit;
  unrelated baseline dirty files are allowed and must not be staged.
- Phase D passes
- Commit already contains `Opi-*` footers

If footer missing: print required footer text and stop (do NOT amend).

## Risk Evaluator Gate

A task has `evaluator_required = true` when ANY of:
- Tier is `cli-runtime` or `tui`
- Task touches multiple crates or public protocol/data model
- Task changes tool safety, tool selection, allowlists, extension hooks, config,
  session storage, JSON framing, provider events, or release-critical behavior

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
