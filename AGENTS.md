# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project

`opi` is a Rust reimplementation of ideas from [earendil-works/pi](https://github.com/earendil-works/pi), organized as an AI agent toolkit and terminal-first coding agent.

Current workspace version: `0.3.0`.

The current implementation includes:

- A working `opi` coding-agent binary.
- Interactive ratatui TUI mode.
- Non-interactive text mode and `--json` NDJSON mode.
- Six built-in tools: `read`, `glob`, `grep`, `write`, `edit`, `bash`.
- Multi-provider streaming through Anthropic, OpenAI Chat Completions, OpenAI Responses, OpenRouter, Mistral, and Gemini.
- TOML config with layered precedence.
- Session JSONL persistence with list/resume/delete CLI commands.
- Context compaction, retry/backoff, usage accumulation, configurable keybindings/themes, edit diff rendering, and best-effort cost tracking.

`opi-web-ui` remains a placeholder crate with `publish = false`; it is not a real web UI implementation yet.

Repository: https://github.com/OdradekAI/opi

## Conversational style

- Keep answers short and concise.
- No emojis in commits, issues, PR comments, or code.
- No fluff or cheerful filler text.
- Technical prose only; be kind but direct.
- When the user asks a question, answer it first before making edits or running commands.

## Code quality

- Read files in full before making wide-ranging changes, before editing files you have not already fully inspected, and when the user asks you to investigate or audit something. Do not rely only on search snippets for broad changes.
- Always ask before removing functionality or code that appears to be intentional.
- Do not preserve backward compatibility unless the user explicitly asks for it.
- Avoid `unsafe` unless absolutely necessary; prefer safe abstractions.
- Prefer `thiserror` for library error types, `anyhow` only in binary/test code.
- Use workspace dependencies. Never add a version directly to a crate's `Cargo.toml` if it can go through `[workspace.dependencies]`.
- Trait objects (`Box<dyn T>`) are fine at crate boundaries; prefer generics within a crate when the concrete type is known at compile time.
- Match the existing module's style. If a file uses `thiserror`, do not switch to manual `impl Display + Error`.

## Workspace layout

Cargo workspace with lockstep versioning. All crates share `version.workspace = true`. Internal dependencies flow through `[workspace.dependencies]` in the root `Cargo.toml`:

```text
opi-ai      (no internal deps)        - multi-provider LLM API
opi-tui     (no internal deps)        - terminal UI widgets
opi-agent   -> opi-ai                 - agent runtime, tool calling, sessions, compaction
opi-web-ui  -> opi-ai                 - placeholder web chat component crate
opi-coding-agent -> opi-ai, opi-agent, opi-tui - produces the `opi` binary
```

The dependency order above is also the crates.io publish order for publishable crates. Adding a new internal dependency means updating `[workspace.dependencies]` in root `Cargo.toml`, then referencing it as `foo = { workspace = true }` in the consumer crate's `Cargo.toml`.

When publishing internal crates, path dependencies must also carry a `version` field. Bare path deps cannot be published to crates.io. The release skill manages this.

## Architecture

The `opi` binary is produced by `opi-coding-agent`. Startup flow:

- Session commands (`--list-sessions`, `--delete-session`) are handled before config/provider construction and then exit.
- `--resume <ID>` loads a JSONL session, reconstructs the active branch, and then continues in interactive or non-interactive mode.
- Non-interactive mode is selected by non-empty positional `[PROMPT]...`, `--non-interactive`, or `--json`. It builds a provider, runs `NonInteractiveRunner`, prints stdout/stderr, and exits with a numeric code.
- Interactive mode is the default with no prompt args. It builds a `CodingHarness` with `InteractiveCodingHooks` and launches `interactive::run_interactive_tui()`.

Both interactive and non-interactive modes use the same core loop from `opi-agent`:

```text
agent_loop()
  -> transform_context
  -> convert_to_llm
  -> stream provider response
  -> accumulate AssistantStreamEvent values
  -> detect tool calls
  -> validate args with JSON Schema
  -> run before_tool_call hook
  -> execute tools in parallel or sequential batches
  -> run after_tool_call hook
  -> check terminate flags and should_stop_after_turn
  -> prepare_next_turn
  -> poll steering/follow-up queues
  -> repeat
```

Key abstractions in `opi-agent`:

- `AgentHooks`: six hook methods: `transform_context`, `convert_to_llm`, `before_tool_call`, `after_tool_call`, `should_stop_after_turn`, `prepare_next_turn`.
- `Tool`: `definition()` returns JSON schema, `execute()` runs the tool, `execution_mode()` controls parallel vs sequential batching.
- `SessionWriter` / `SessionReader`: append-only JSONL session storage with crash recovery.
- `CompactionEngine`: threshold/manual/overflow compaction primitives with hook support.
- `AgentSessionEvent`: session-level event protocol used by JSON mode.
- `Transport`: abstraction over stdio/SSE for MCP-style tool servers; not wired into the main loop yet.

Provider implementations live in `opi-ai`:

- `anthropic:` uses Anthropic Messages SSE.
- `openai:` uses OpenAI Chat Completions streaming.
- `openai-responses:` uses OpenAI Responses streaming.
- `openrouter:` uses an OpenAI-compatible OpenRouter profile.
- `mistral:` uses an OpenAI-compatible Mistral profile.
- `gemini:` uses Gemini `streamGenerateContent?alt=sse`.

Config resolution for model selection:

1. `--model`
2. `OPI_MODEL` only when `--config` was not passed
3. `model` in `--config <FILE>`
4. Project `.opi/config.toml`
5. User config
6. Built-in defaults

TOML layers merge user -> project -> `--config`. Model specs use `provider:model` format, for example `anthropic:claude-sonnet-4-5-20250514` or `openai:gpt-4o`.

## Edition

Workspace is on Rust edition 2024, so Rust 1.85+ is required.

## Commands

```sh
# Build everything
cargo build
cargo build --release

# Run the CLI binary
cargo run -p opi-coding-agent             # interactive TUI
cargo run -p opi-coding-agent -- --help
cargo run -p opi-coding-agent -- --version

# Non-interactive examples
cargo run -p opi-coding-agent -- "Summarize this workspace"
cargo run -p opi-coding-agent -- --json "Summarize this workspace"

# Tests
cargo test --workspace --all-targets
cargo test -p opi-ai
cargo test -p opi-ai -- some_test_name

# Lint and format (CI and release gates)
cargo fmt --all
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings

# Docs with warnings as errors (release gate)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

After code changes (not documentation-only), run `cargo clippy --workspace --all-targets -- -D warnings` and fix all warnings before committing.

If you create or modify a test file, run that test and iterate until it passes.

## Testing

- Tests live in `crates/<crate>/tests/` for integration tests and inline `#[cfg(test)]` modules for unit tests.
- Use `opi_ai::test_support::MockProvider` for agent/harness integration tests. Never hit a real LLM API or require API keys in tests.
- For provider wire-format tests, use fixtures or `wiremock`; do not require external network access.
- For tool tests that touch the filesystem, use `tempfile::tempdir()` and build fixtures in the temp directory.
- For session tests, use isolated temp directories or `OPI_SESSIONS_DIR` to avoid user data and test races.
- For snapshot/UI tests, follow the existing `insta` snapshot pattern in `opi-tui`.
- Run the relevant test after writing it: `cargo test -p <crate> -- <test_name>`.

## Git rules

### Committing

- NEVER commit unless the user asks.
- ONLY commit files YOU changed in THIS session.
- NEVER use `git add -A` or `git add .`; these sweep up changes from other agents or unrelated working-tree state.
- ALWAYS use `git add <specific-file-paths>` listing only files you modified.
- Before committing, run `git status` and verify you are only staging YOUR files.
- Always include `fixes #<number>` or `closes #<number>` in the commit message when there is a related issue.
- NEVER include `Co-Authored-By` trailers in commit messages. No `Co-Authored-By: Codex ...` or similar.

### Forbidden git operations

These commands can destroy work:

- `git reset --hard` - destroys uncommitted changes
- `git checkout .` - destroys uncommitted changes
- `git clean -fd` - deletes untracked files
- `git stash` - stashes ALL changes including other agents' work
- `git add -A` / `git add .` - stages other agents' uncommitted work
- `git commit --no-verify` - bypasses required hooks and is never allowed
- `git push --force` - can overwrite shared history

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
- If conflict is in a file you did not modify, abort and ask the user.
- NEVER force push.

## PR workflow

- Analyze PRs without pulling locally first.
- Work in feature branches until everything meets requirements, then merge into main and push.
- Never open PRs unless the user explicitly asks.

## Changelog

Location: `CHANGELOG.md` at the repo root (single changelog for the whole workspace).

Format is [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) with sections:

- `### Breaking Changes`
- `### Added`
- `### Changed`
- `### Fixed`
- `### Removed`

Rules:

- New entries ALWAYS go under `## [Unreleased]`; create this section if it does not exist.
- NEVER modify already-released version sections.
- Each released version section is immutable.

## Releasing

Releases go to both GitHub Releases and crates.io via the `opi-release` skill at `.claude/skills/opi-release/skill.md`. Invoke with a target semver version, for example `0.3.1`.

Critical properties:

- Phases 1-4 are reversible.
- Phase 5 pushes a tag and is publicly visible.
- Phase 6 publishes to crates.io and is irreversible; crates can only be yanked, never deleted.
- All publishable crates use the same version and publish in dependency order computed from `cargo metadata`.
- Never use `git reset --hard` plus `git push --force` for rollback. Use `git revert` plus tag deletion; this is enforced by the skill.
- Interrupted releases can resume from `.opi-release-state.json` at the repo root.
- CI-driven builds are recommended: push the tag, then `release.yml` builds all 6 platform targets and uploads them to the GitHub Release.

The design rationale is in `docs/superpowers/specs/2026-05-19-opi-release-skill-design.md`.

## CI

Two GitHub Actions workflows live in `.github/workflows/`:

- `ci.yml`: runs on push/PR to `main`. Jobs: `fmt`, `clippy`, `test`, `doc`.
- `release.yml`: triggered by `v*` tags or manual `workflow_dispatch`. Builds `opi` for linux-x64, linux-arm64, darwin-x64, darwin-arm64, windows-x64, and windows-arm64.

## Conventions

- Conventional Commits drive changelog categorization (`feat:` -> Added, `fix:` -> Fixed, `feat!:` / `BREAKING CHANGE` -> Breaking Changes).
- Each crate's `description`, `license`, and `repository` come from the workspace; do not duplicate them per crate.
- The CLI binary is named `opi` (defined by `[[bin]]` in `crates/opi-coding-agent/Cargo.toml`), not `opi-coding-agent`.
- `opi-web-ui` has `publish = false`; do not describe it as implemented until real web components exist.

## User override

If the user's instructions conflict with rules set out here, ask for confirmation that they want to override the rules. Only then execute their instructions.
