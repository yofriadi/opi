# opi

[![CI](https://github.com/OdradekAI/opi/actions/workflows/ci.yml/badge.svg)](https://github.com/OdradekAI/opi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> AI agent toolkit in Rust — a port of [earendil-works/pi](https://github.com/earendil-works/pi) focused on a minimal terminal coding agent.

[简体中文](README.zh.md) · [Changelog](CHANGELOG.md) · [Spec](docs/opi-spec.md)

---

## Status

Phase 1 MVP (`v0.2.0`). Functional Anthropic-based coding assistant with six built-in tools, a ratatui-based TUI, TOML configuration, and a mock-provider test harness (248 unit/integration tests). Other LLM providers, sub-agents, sessions, MCP transport, and the web UI are not implemented yet — see [Roadmap](#roadmap).

## Workspace

Cargo workspace with **lockstep versioning** — every crate shares the same version from `[workspace.package]`.

| Crate | crates.io | Description |
|-------|-----------|-------------|
| [`opi-ai`](crates/opi-ai) | published | Provider abstraction + Anthropic SSE streaming |
| [`opi-agent`](crates/opi-agent) | published | Agent runtime: tool calling, hooks, queue polling |
| [`opi-tui`](crates/opi-tui) | published | Terminal UI widgets (message list, editor, markdown, status bar, tool view) |
| [`opi-coding-agent`](crates/opi-coding-agent) | published | The `opi` binary — interactive & non-interactive coding agent |
| [`opi-web-ui`](crates/opi-web-ui) | `publish = false` | Reserved namespace; not implemented |

Dependency order (also the publish order):

```
opi-ai      ─┬─→ opi-agent ─┐
             │              ├─→ opi-coding-agent  ──╮
opi-tui ─────┴──────────────┘                       │
opi-web-ui ──→ opi-ai                               │
                                                    └→  opi  binary
```

## Install

The binary is named `opi`, produced by the `opi-coding-agent` crate.

```sh
cargo install opi-coding-agent
opi --version
```

Pre-built binaries for Linux, macOS, and Windows (x64 + arm64) are attached to each [GitHub Release](https://github.com/OdradekAI/opi/releases).

## Quick start

```sh
export ANTHROPIC_API_KEY=sk-ant-...

# Interactive TUI
opi

# Non-interactive (positional prompt → stdout → exit)
opi "List the Rust files in this directory."
```

Only `anthropic:<model>` specs work in v0.2.0. The default is `anthropic:claude-sonnet-4`. Override per invocation:

```sh
opi -m anthropic:claude-opus-4 "Explain src/main.rs"
```

Or via `OPI_MODEL`, `--config`, a project `.opi/config.toml`, or a user config file. Model precedence: **`--model` > `OPI_MODEL` (only without `--config`) > `--config` file > project > user > defaults** (see [`opi-coding-agent`](crates/opi-coding-agent/README.md)).

## Built-in tools

The agent ships with six tools, exposed via the `Tool` trait from `opi-agent`:

| Tool | Purpose | Mutating? |
|------|---------|-----------|
| `read` | Read file content with optional line range | no |
| `glob` | List files matching a glob (gitignore-aware) | no |
| `grep` | Search file contents (gitignore-aware) | no |
| `write` | Create or overwrite a file | yes |
| `edit` | Apply an exact string replacement | yes |
| `bash` | Execute a shell command with a timeout | yes |

In non-interactive mode, mutating tools require `--allow-mutating` (or `defaults.allow_mutating_tools = true` in config). In interactive mode, the TUI prompts for confirmation.

## Build from source

Workspace is on **Rust edition 2024**, so you need a toolchain ≥ 1.85.

```sh
# build everything
cargo build
cargo build --release

# run the CLI without installing
cargo run -p opi-coding-agent -- --help

# run the test suite (248 tests across the workspace)
cargo test --workspace --all-targets

# single crate
cargo test -p opi-ai

# the gates CI enforces
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Architecture

The `opi` binary picks one of two paths at startup:

- **Non-interactive** (non-empty positional `[PROMPT]...`, or `--non-interactive`): builds a provider, runs `NonInteractiveRunner::run()`, prints captured stdout/stderr, exits with a numeric code (`0` success, `1` runtime, `2` config, `3` auth, `4` provider, `5` tool, `130` interrupted).
- **Interactive** (default): builds a `CodingHarness` with `InteractiveCodingHooks` and runs the ratatui TUI.

Both paths drive the same core loop in `opi-agent::agent_loop`:

```
transform_context → convert_to_llm → provider.stream(Request) → SSE/tool events
   → validate args (jsonschema) → before_tool_call → execute (parallel/sequential)
   → after_tool_call → should_stop_after_turn → poll steering / follow-up → repeat
```

Key abstractions:

- **`opi_ai::Provider`** — `stream(Request) -> EventStream` of `AssistantStreamEvent`s; cancellation via `tokio_util::sync::CancellationToken`.
- **`opi_agent::Tool`** — `definition()` returns a JSON Schema; `execute()` runs the tool; `execution_mode()` controls parallel-vs-sequential batching.
- **`opi_agent::AgentHooks`** — six hook methods: `transform_context`, `convert_to_llm`, `before_tool_call`, `after_tool_call`, `should_stop_after_turn`, `prepare_next_turn`.
- **`opi_agent::Transport`** — placeholder trait for stdio/SSE tool transport; not wired into the loop yet.

The full specification lives in [`docs/opi-spec.md`](docs/opi-spec.md).

## Roadmap

Phase 1 (✅ shipped in 0.2.0):
- Anthropic provider, `Tool` + `AgentHooks` traits, agent loop, six tools, basic TUI, TOML config.

Not yet implemented:
- Other providers (OpenAI, Google, Mistral, Bedrock, Azure) — `ProviderKind` / `ApiKind` are reserved on the registry and message types; only Anthropic is wired up.
- Persistent sessions, branching, compaction.
- Sub-agents, skills, prompt templates, MCP transport.
- `opi-web-ui` (currently a stub with `publish = false`).
- Subscription / OAuth flows (`/login`).

## Releasing

Releases publish to both **GitHub Releases** and **crates.io** in a single workflow driven by the `opi-release` skill (`.claude/skills/opi-release/skill.md`). Highlights:

- All crates publish at the same version, in dependency order, computed dynamically from `cargo metadata`.
- Tag push (`v*`) triggers [`release.yml`](.github/workflows/release.yml), which builds six platform targets and uploads them to the release.
- Rollback is performed via `git revert` + tag deletion; never `git reset --hard` + `git push --force`.

## Contributing

Project conventions:

- Conventional Commits (`feat:` → Added, `fix:` → Fixed, `feat!:` / `BREAKING CHANGE` → Breaking).
- Each crate inherits `description`, `license`, and `repository` from `[workspace.package]` — don't duplicate per crate.
- See [`CLAUDE.md`](CLAUDE.md) for the rules followed by both humans and agents working in this repo.

Bug reports and PRs welcome at <https://github.com/OdradekAI/opi/issues>.

## License

MIT © OdradekAI. See [`LICENSE`](LICENSE).
