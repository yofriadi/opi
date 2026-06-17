# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`opi` is a Rust reimplementation of [earendil-works/pi](https://github.com/earendil-works/pi), organized as an AI agent toolkit and terminal-first coding agent. v0.5.2 ships a multi-provider coding assistant (Anthropic, OpenAI, OpenAI Responses, OpenRouter, Mistral, Gemini, Bedrock, Azure OpenAI, Vertex AI), config-driven OpenAI-compatible provider profiles, eight built-in tools, image attachments, fuzzy model/session/branch pickers, shell completion generation, a ratatui TUI with configurable keybindings/themes, session JSONL persistence with resume/fork and active branch `parent_id`/`leaf` links, context compaction, retry/backoff, cost tracking, RPC JSONL mode, shared SDK command/event types, extension hooks/tools/state, resource/package discovery, custom provider/model registration, and an unpublished `opi-web-ui` component/state/rendering crate. New work extends this foundation rather than redesigning the layout.

Repository: https://github.com/OdradekAI/opi

`AGENTS.md` is the Codex-flavored sibling of this file. When project rules change, update both in lockstep to avoid drift.

## Conversational style

- Keep answers short and concise.
- No emojis in commits, issues, PR comments, or code.
- No fluff or cheerful filler text.
- Technical prose only — be kind but direct.
- When the user asks a question, answer it first before making edits or running commands.

## Code quality

- Read files in full before making wide-ranging changes, before editing files you have not already fully inspected, and when the user asks you to investigate or audit something. Do not rely only on search snippets for broad changes.
- Always ask before removing functionality or code that appears to be intentional.
- Do not preserve backward compatibility unless the user explicitly asks for it.
- Avoid `unsafe` unless absolutely necessary; prefer safe abstractions.
- Prefer `thiserror` for library error types, `anyhow` only in binary/test code.
- Use workspace dependencies — never add a version directly to a crate's `Cargo.toml` if it can go through `[workspace.dependencies]`.
- Trait objects (`Box<dyn T>`) are fine at crate boundaries; prefer generics within a crate when the concrete type is known at compile time.
- Match the existing module's style: if a file uses `thiserror`, don't switch to manual `impl Display + Error`.

## Workspace layout

Cargo workspace with **lockstep versioning** (all crates share `version.workspace = true`). Internal dependencies flow through `[workspace.dependencies]` in the root `Cargo.toml`:

```
opi-ai      (no internal deps)        — unified multi-provider LLM API
opi-tui     (no internal deps)        — terminal UI with differential rendering
opi-agent   → opi-ai                  — agent runtime, tool calling, session management
opi-web-ui  (no internal deps)        — unpublished web-facing component/state/rendering crate
opi-coding-agent → opi-ai, opi-agent, opi-tui  — produces the `opi` binary
```

The dependency order above is also the **crates.io publish order** (see the `opi-release` skill). Adding a new internal dependency means updating `[workspace.dependencies]` in root `Cargo.toml`, then referencing it as `foo = { workspace = true }` in the consumer crate's `Cargo.toml`.

When publishing internal crates, the `path` dependencies MUST also carry a `version` field — bare path deps cannot be published to crates.io. The release skill manages this.

## Architecture

The `opi` binary (`opi-coding-agent`) chooses a mode at startup:

- **Session commands** (`--list-sessions`, `--resume`, `--fork`, `--delete-session`): handled before any provider is constructed. `--fork <ID>` copies the source session's active branch into a new parented JSONL session and continues from it.
- **RPC** (`--rpc`): builds a provider and `CodingHarness`, then runs the unstable JSONL command/event protocol over stdin/stdout.
- **Non-interactive** (non-empty positional `[PROMPT]...`, `--non-interactive`, or `--json`): builds a provider, runs `NonInteractiveRunner::run()`, prints output (or NDJSON events with `--json`), exits.
- **Interactive** (default, no prompt args): builds a `CodingHarness` with `InteractiveCodingHooks`, launches the ratatui-based TUI via `interactive::run_interactive_tui()`.

Both interactive and non-interactive modes use the same core loop from `opi-agent`:

```
agent_loop() → stream provider response → detect tool calls → validate schema
→ run before_tool_call hook → execute tools (parallel or sequential) → run
after_tool_call hook → check should_stop_after_turn → poll steering/follow-up
queues → repeat
```

Eight built-in tools in `opi-coding-agent`: `read`, `glob`, `grep`, `find`, `ls` (parallel, read-only) and `write`, `edit`, `bash` (sequential, mutating). All paths are constrained to the harness workspace root. Mutating tools require `--allow-mutating` or `defaults.allow_mutating_tools = true`.

Key abstractions:
- **`opi_ai::Provider`** trait — streaming LLM backend; resolved from `provider:model` specs via the registry.
- **`opi_ai::AssistantStreamEvent`** — provider-neutral stream event model (text, thinking, tool calls, completion, errors).
- **`opi_agent::AgentHooks`** trait — 6 hook methods: `transform_context`, `convert_to_llm`, `before_tool_call`, `after_tool_call`, `should_stop_after_turn`, `prepare_next_turn`.
- **`opi_agent::Tool`** trait — `definition()` returns JSON schema, `execute()` runs the tool, `execution_mode()` controls parallel vs sequential batching.
- **`opi_agent::SessionWriter` / `SessionReader`** — append-only JSONL session storage with crash recovery.
- **`opi_agent::CompactionEngine`** — threshold/manual/overflow context compaction.

Provider implementations in `opi-ai`: `anthropic`, `openai`, `openai-responses`, `openrouter`, `mistral`, `gemini`, `bedrock` (AWS SigV4), `azure` (Azure OpenAI deployment), `vertex` (Google Vertex AI).

Config resolution (model): `--model` > `OPI_MODEL` (only when `--config` was not passed) > `--config` file > project `.opi/config.toml` > user config > built-in defaults. TOML layers merge user → project → `--config`. Model specs use `provider:model` format (e.g. `anthropic:claude-sonnet-4-5-20250514`, `openai:gpt-4o`, `gemini:gemini-2.5-flash`).

## Edition

Workspace is on **Rust edition 2024**, so a recent stable toolchain is required (the edition gained stable support in Rust 1.85+).

## Commands

```sh
# Build everything
cargo build
cargo build --release

# Run the CLI binary
cargo run -p opi-coding-agent             # interactive TUI
cargo run -p opi-coding-agent -- --version

# Tests
cargo test --workspace --all-targets
cargo test -p opi-ai                      # single crate
cargo test -p opi-ai -- some_test_name    # single test

# Lint & format (these are the gates CI and the release skill enforce)
cargo fmt --all
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings

# Docs with warnings-as-errors (release gate)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

After code changes (not documentation-only): run `cargo clippy --workspace --all-targets -- -D warnings` and fix all warnings before committing.

If you create or modify a test file, you MUST run that test and iterate until it passes.

## Testing

- Tests live in `crates/<crate>/tests/` (integration) and inline `#[cfg(test)]` modules (unit).
- Use `opi_ai::test_support::MockProvider` for agent/harness integration tests — never hit a real LLM API or require API keys in tests.
- For provider wire-format tests, use fixtures or `wiremock`; do not require external network access.
- For tool tests that touch the filesystem, use `tempfile::tempdir()` and build fixtures in the temp directory.
- For snapshot/UI tests in `opi-tui`, follow the existing `insta` snapshot pattern.
- Run the relevant test after writing it: `cargo test -p <crate> -- <test_name>`.

## Sessions

Sessions are append-only JSONL files written by the coding harness. Default location is `%LOCALAPPDATA%\opi\sessions\` on Windows, `~/.local/share/opi/sessions/` on Unix; override with `OPI_SESSIONS_DIR`. Storage and resume logic lives in `opi-agent::SessionWriter`/`SessionReader`; compaction is in `opi-agent::CompactionEngine`. Session files hold a header plus message, compaction, and leaf entries; resume reconstructs the active branch and honors compaction summaries. Fork commands create a new session whose `parent_session` points at the source; they do not rewrite the source file.

Tests that touch the session dir or `OPI_SESSIONS_DIR` must serialize — parallel env-var mutation has caused flakes before (see CHANGELOG 0.3.0 Fixed).

## Git rules

### Committing

- **NEVER commit unless the user asks.**
- ONLY commit files YOU changed in THIS session.
- NEVER use `git add -A` or `git add .` — these sweep up changes from other agents or unrelated working-tree state.
- ALWAYS use `git add <specific-file-paths>` listing only files you modified.
- Before committing, run `git status` and verify you are only staging YOUR files.
- Always include `fixes #<number>` or `closes #<number>` in the commit message when there is a related issue.
- **NEVER include `Co-Authored-By` trailers in commit messages.** No `Co-Authored-By: Claude ...` or similar.

### Forbidden git operations

These commands can destroy work:

- `git reset --hard` — destroys uncommitted changes
- `git checkout .` — destroys uncommitted changes
- `git clean -fd` — deletes untracked files
- `git stash` — stashes ALL changes including other agents' work
- `git add -A` / `git add .` — stages other agents' uncommitted work
- `git commit --no-verify` — bypasses required hooks and is never allowed
- `git push --force` — can overwrite shared history

### Safe workflow

```bash
# 1. Check status
git status

# 2. Add ONLY your specific files
git add crates/opi-ai/src/anthropic.rs
git add crates/opi-ai/tests/anthropic_test.rs

# 3. Commit (Conventional Commits format)
git commit -m "fix(opi-ai): handle CRLF in SSE parser"

# 4. Push (pull --rebase if needed, but NEVER reset/checkout)
git pull --rebase && git push
```

### If rebase conflicts occur

- Resolve conflicts in YOUR files only.
- If conflict is in a file you didn't modify, abort and ask the user.
- NEVER force push.

## PR workflow

- Analyze PRs without pulling locally first.
- We work in feature branches until everything meets requirements, then merge into main and push.
- You never open PRs yourself unless the user explicitly asks.

## Changelog

Location: `CHANGELOG.md` at the repo root (single changelog for the whole workspace).

Format is [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) with sections:
- `### Breaking Changes`
- `### Added`
- `### Changed`
- `### Fixed`
- `### Removed`

Rules:
- New entries ALWAYS go under `## [Unreleased]` (create this section if it doesn't exist).
- NEVER modify already-released version sections.
- Each released version section is immutable.

## Releasing

Releases go to both **GitHub Releases** and **crates.io** via the `opi-release` skill at `.claude/skills/opi-release/skill.md`. Invoke with a target semver version (e.g. `0.2.0`). Critical properties of that flow:

- **Phases 1–4 are reversible**; **Phase 5 pushes a tag (publicly visible)**; **Phase 6 publishes to crates.io and is irreversible** (crates can only be yanked, never deleted).
- All crates publish at the same version, in dependency order, computed dynamically via `cargo metadata`.
- Never use `git reset --hard` + `git push --force` for rollback. Use `git revert` + tag deletion (this is enforced in the skill).
- Interrupted releases can resume from `.opi-release-state.json` at the repo root.
- **CI-driven builds** (recommended): Push the tag → `release.yml` builds all 6 platform targets and uploads to the GitHub Release. No local `cross` needed.

The design rationale is in `docs/superpowers/specs/2026-05-19-opi-release-skill-design.md`.

## Implementation workflow

Long-running spec implementations track state in `.opi-impl-state.json` at the repo root, driven by the `opi-implement` skill. Don't delete or hand-edit this file; use the skill's commands to query, advance, or reset progress.

## CI

Two GitHub Actions workflows in `.github/workflows/`:

- **ci.yml** — Runs on push/PR to `main`. Jobs: `fmt`, `clippy`, `test`, `doc`. These are the gates that Phase 1.3 of the release skill checks.
- **release.yml** — Triggered by `v*` tags or manual `workflow_dispatch`. Builds `opi` binary for 6 targets (linux-x64, linux-arm64, darwin-x64, darwin-arm64, windows-x64, windows-arm64) using native runners + `cross` for linux-arm64. Uploads artifacts to the GitHub Release.

## Conventions

- Conventional Commits drive changelog categorization (`feat:` → Added, `fix:` → Fixed, `feat!:`/`BREAKING CHANGE` → Breaking Changes).
- Each crate's `description`, `license`, and `repository` come from the workspace — don't duplicate them per crate.
- The CLI binary is named `opi` (defined by `[[bin]]` in `crates/opi-coding-agent/Cargo.toml`), not `opi-coding-agent`.
- `opi-web-ui` has `publish = false`; describe it as reusable components/state/rendering, not as a standalone browser app.

## User override

If the user's instructions conflict with rules set out here, ask for confirmation that they want to override the rules. Only then execute their instructions.
