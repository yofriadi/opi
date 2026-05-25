# opi

[![CI](https://github.com/OdradekAI/opi/actions/workflows/ci.yml/badge.svg)](https://github.com/OdradekAI/opi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> AI agent toolkit in Rust, reimplementing ideas from [earendil-works/pi](https://github.com/earendil-works/pi) as a terminal-first coding agent and reusable agent crates.

[Simplified Chinese](README.zh.md) | [Changelog](CHANGELOG.md) | [Spec](docs/opi-spec.md)

## Status

Current workspace version: `0.3.0`.

`opi` now has a working coding-agent binary, six built-in tools, a ratatui TUI, non-interactive stdout and NDJSON modes, TOML configuration, multi-provider streaming, session persistence, context compaction, retry/backoff support, configurable keybindings/themes, usage accumulation, and best-effort cost tracking.

The web UI crate still exists as a reserved placeholder and is not published to crates.io.

## Workspace

Cargo workspace with lockstep versioning: every crate inherits `version`, `edition`, `license`, `repository`, and `authors` from `[workspace.package]`.

| Crate | Published | Description |
|-------|-----------|-------------|
| [`opi-ai`](crates/opi-ai) | yes | Multi-provider LLM API, streaming events, registry, retry, usage, cost helpers |
| [`opi-agent`](crates/opi-agent) | yes | Agent loop, tool execution, hooks, events, sessions, compaction, transport trait |
| [`opi-tui`](crates/opi-tui) | yes | Ratatui widgets, diff view, themes, keybindings |
| [`opi-coding-agent`](crates/opi-coding-agent) | yes | The `opi` binary and embeddable coding harness |
| [`opi-web-ui`](crates/opi-web-ui) | no (`publish = false`) | Reserved web chat component crate |

Internal dependency flow:

```text
opi-ai
  -> opi-agent
  -> opi-web-ui

opi-tui

opi-ai + opi-agent + opi-tui
  -> opi-coding-agent
     -> opi binary
```

## Install

The executable is named `opi` and is produced by the `opi-coding-agent` crate.

```sh
cargo install opi-coding-agent
opi --version
```

Pre-built binaries for Linux, macOS, and Windows (x64 and arm64) are attached to each [GitHub Release](https://github.com/OdradekAI/opi/releases).

## Quick Start

Set an API key for the provider you want to use:

```sh
export ANTHROPIC_API_KEY=sk-ant-...
# or OPENAI_API_KEY, OPENROUTER_API_KEY, MISTRAL_API_KEY, GEMINI_API_KEY
```

Run the interactive TUI:

```sh
opi
```

Run a single prompt and print assistant text to stdout:

```sh
opi "List the Rust crates in this workspace."
```

Emit newline-delimited JSON events for automation:

```sh
opi --json "Summarize the latest session state."
```

Pick a model with `provider:model` syntax:

```sh
opi -m anthropic:claude-sonnet-4-5-20250514 "Explain crates/opi-agent/src/lib.rs"
opi -m openai:gpt-4o "Review the public API shape."
opi -m openai-responses:gpt-4o-mini "Find small documentation gaps."
opi -m openrouter:openai/gpt-4o-mini "List TODO comments."
opi -m mistral:codestral-latest "Explain the tool modules."
opi -m gemini:gemini-2.5-flash "Summarize the README files."
```

## Supported Providers

Provider support is implemented in `opi-ai` and wired into `opi-coding-agent`.

| Provider spec prefix | API key env default | Notes |
|----------------------|---------------------|-------|
| `anthropic:` | `ANTHROPIC_API_KEY` | Anthropic Messages API with thinking support |
| `openai:` | `OPENAI_API_KEY` | OpenAI Chat Completions compatible streaming |
| `openai-responses:` | `OPENAI_API_KEY` | OpenAI Responses API streaming |
| `openrouter:` | `OPENROUTER_API_KEY` | OpenAI-compatible OpenRouter profile |
| `mistral:` | `MISTRAL_API_KEY` | OpenAI-compatible Mistral profile |
| `gemini:` | `GEMINI_API_KEY` | Gemini `streamGenerateContent` SSE |

## Built-in Tools

Tools are implemented by `opi-coding-agent` and exposed through the `opi-agent::Tool` trait.

| Tool | Args | Execution | Mutating |
|------|------|-----------|----------|
| `read` | `path`, optional `offset`, `limit` | parallel | no |
| `glob` | `pattern` | parallel | no |
| `grep` | `pattern` | parallel | no |
| `write` | `path`, `content` | sequential | yes |
| `edit` | `path`, `old_string`, `new_string` | sequential | yes |
| `bash` | `command`, optional `timeout_secs` | sequential | yes |

All paths are constrained to the harness workspace root. Mutating tools require `--allow-mutating` or `defaults.allow_mutating_tools = true`.

## Configuration

Config resolution merges user config, project config, and explicit config files. Model precedence is:

1. `--model`
2. `OPI_MODEL` when `--config` was not passed
3. `model` in `--config <FILE>`
4. `<CWD>/.opi/config.toml`
5. User config (`%APPDATA%\opi\config.toml` on Windows, `~/.config/opi/config.toml` on Unix)
6. Built-in defaults

Example:

```toml
[defaults]
model = "anthropic:claude-sonnet-4-5-20250514"
max_iterations = 50
tool_timeout_ms = 30000
theme = "default"
allow_mutating_tools = false

[thinking]
enabled = true
budget_tokens = 10000

[retry]
max_attempts = 3
initial_delay_ms = 1000
max_delay_ms = 60000

[compaction]
enabled = true
threshold_tokens = 100000

[keybindings]
submit = "enter"
abort = "escape"
new_line = "alt+enter"

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"

[providers.openai]
api_key_env = "OPENAI_API_KEY"

[providers.openai_responses]
api_key_env = "OPENAI_API_KEY"

[providers.openrouter]
api_key_env = "OPENROUTER_API_KEY"
# referer = "https://example.com"

[providers.mistral]
api_key_env = "MISTRAL_API_KEY"

[providers.gemini]
api_key_env = "GEMINI_API_KEY"
```

## Sessions

Sessions are JSONL files written automatically by the coding harness.

Default location:

- Windows: `%LOCALAPPDATA%\opi\sessions\`
- Unix: `~/.local/share/opi/sessions/`

Override with `OPI_SESSIONS_DIR`.

```sh
opi --list-sessions
opi --resume <session-id> "Continue from this session."
opi --delete-session <session-id>
```

Session files store a header plus message, compaction, and leaf entries. Resume reconstructs the active branch and honors compaction summaries. `--json` emits session events, retry events, compaction events, and a final session summary with token totals and optional cost totals.

## Build From Source

The workspace uses Rust edition 2024, requiring Rust 1.85 or newer.

```sh
cargo build
cargo build --release

cargo run -p opi-coding-agent -- --help

cargo test --workspace --all-targets
cargo test -p opi-ai

cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Architecture

`opi` chooses a mode at startup:

- Non-interactive: positional prompt, `--non-interactive`, or `--json`; builds a provider and runs `NonInteractiveRunner`.
- Interactive: default with no prompt; builds `CodingHarness` with interactive hooks and launches the ratatui TUI.
- Session commands: `--list-sessions`, `--resume`, and `--delete-session` are handled before provider construction.

Both interactive and non-interactive modes use the same agent loop:

```text
transform_context
  -> convert_to_llm
  -> provider.stream(Request)
  -> accumulate assistant stream events
  -> detect tool calls
  -> validate JSON Schema args
  -> before_tool_call
  -> execute tools in parallel or sequential batches
  -> after_tool_call
  -> should_stop_after_turn
  -> prepare_next_turn
  -> poll steering/follow-up queues
```

Key abstractions:

- `opi_ai::Provider`: streaming LLM backend interface.
- `opi_ai::AssistantStreamEvent`: provider-neutral stream event model for text, thinking, tool calls, completion, and errors.
- `opi_agent::Tool`: JSON Schema based tool contract with parallel/sequential execution modes.
- `opi_agent::AgentHooks`: lifecycle hooks around message conversion, tool policy, tool results, stopping, and next-turn preparation.
- `opi_agent::SessionWriter` / `SessionReader`: append-only JSONL session storage with crash recovery.
- `opi_agent::CompactionEngine`: threshold/manual/overflow compaction support.
- `opi_agent::Transport`: stdio/SSE transport abstraction reserved for external tool servers; not wired into the main loop yet.

## Still Not Implemented

- Sub-agents and skills.
- Prompt template registry.
- MCP tool server integration through `Transport`.
- OAuth or subscription login flows.
- Real web UI widgets in `opi-web-ui`.

## Releasing

Releases publish to GitHub Releases and crates.io with the `opi-release` skill (`.claude/skills/opi-release/skill.md`).

- All publishable crates use the same version.
- Publish order follows internal dependencies.
- Pushing a `v*` tag triggers `.github/workflows/release.yml`.
- Rollback uses `git revert` and tag deletion, never history rewriting.

## Contributing

- Use Conventional Commits.
- Keep crate metadata inherited from `[workspace.package]`.
- Run `cargo fmt --check --all`, `cargo clippy --workspace --all-targets -- -D warnings`, tests, and docs as appropriate.
- See `AGENTS.md` for the repository working rules used by humans and agents.

Bug reports and PRs are welcome at <https://github.com/OdradekAI/opi/issues>.

## License

MIT (c) OdradekAI. See [LICENSE](LICENSE).
