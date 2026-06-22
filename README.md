# opi

[![CI](https://github.com/OdradekAI/opi/actions/workflows/ci.yml/badge.svg)](https://github.com/OdradekAI/opi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> Rust AI agent toolkit and terminal-first coding agent inspired by
> [earendil-works/pi](https://github.com/earendil-works/pi).

[Simplified Chinese](README.zh.md) | [Changelog](CHANGELOG.md) | [Spec](docs/opi-spec.md)

## Status

The workspace package version in `Cargo.toml` is `0.5.3`. `opi` is usable as a
terminal coding agent and as a set of Rust crates for embedding agent runtime
pieces. The repository may also contain unreleased changes on top of that
version; check [CHANGELOG.md](CHANGELOG.md) for the current delta.

`opi` reimplements selected pi ideas in Rust. It is not API-compatible with pi,
does not read pi config by default, and uses its own TOML config and JSONL
session format.

## Install

The CLI binary is named `opi` and is produced by the `opi-coding-agent` crate.

```sh
cargo install opi-coding-agent
opi --version
```

Pre-built binaries for Linux, macOS, and Windows on x64 and arm64 are attached
to [GitHub Releases](https://github.com/OdradekAI/opi/releases).

## Quick Start

Set credentials for the provider you want to use:

```sh
export ANTHROPIC_API_KEY=sk-ant-...
# or OPENAI_API_KEY, OPENROUTER_API_KEY, MISTRAL_API_KEY, GEMINI_API_KEY
# or AWS credentials, AZURE_OPENAI_API_KEY, VERTEX_ACCESS_TOKEN
```

Run the interactive TUI:

```sh
opi
```

Run a single prompt:

```sh
opi "List the Rust crates in this workspace."
```

Emit NDJSON events for automation:

```sh
opi --json "Summarize this repository."
```

Attach images to the first prompt:

```sh
opi --image screenshot.png "Review this UI."
```

Select a model with `provider:model` syntax:

```sh
opi -m anthropic:claude-sonnet-4-5-20250514 "Explain crates/opi-agent/src/lib.rs"
opi -m openai:gpt-4o "Review the public API shape."
```

## Main CLI Surface

```sh
opi --help
opi --list-models
opi --list-models --json
opi --generate-completion powershell
opi doctor
opi package list
```

Common mode flags:

| Flag | Purpose |
|------|---------|
| `--non-interactive` | Force one-shot text mode. |
| `--json` | One-shot NDJSON event stream. |
| `--rpc` | Persistent JSONL command/event protocol over stdin/stdout. |
| `--resume <ID>` | Resume a saved session. |
| `--fork <ID>` | Fork a saved session into a new session. |
| `--tools read,grep` | Enable only the listed built-in tools. |
| `--no-tools` | Disable all tools. |
| `--allow-mutating` | Allow `write`, `edit`, and `bash` in non-interactive/RPC runs. |
| `--trace <PATH>` | Write an opt-in, redacted local trace envelope for the run. |

## Providers

Provider support lives in `opi-ai` and is wired into `opi-coding-agent`.

| Prefix | Backend | Default credentials |
|--------|---------|---------------------|
| `anthropic:` | Anthropic Messages streaming | `ANTHROPIC_API_KEY` |
| `openai:` | OpenAI Chat Completions streaming | `OPENAI_API_KEY` |
| `openai-responses:` | OpenAI Responses streaming | `OPENAI_API_KEY` |
| `openrouter:` | OpenAI-compatible OpenRouter profile | `OPENROUTER_API_KEY` |
| `mistral:` | OpenAI-compatible Mistral profile | `MISTRAL_API_KEY` |
| `gemini:` | Gemini streaming | `GEMINI_API_KEY` |
| `bedrock:` | AWS Bedrock Converse streaming | AWS env vars or shared AWS config |
| `azure:` | Azure OpenAI deployment | `AZURE_OPENAI_API_KEY` plus endpoint config |
| `vertex:` | Google Vertex AI Gemini streaming | `VERTEX_ACCESS_TOKEN` plus project/location config |
| configured profile | OpenAI-compatible Chat Completions profile | profile-specific `api_key_env` |

## Built-in Tools

Available built-in tools are `read`, `write`, `edit`, `bash`, `grep`, `find`,
`ls`, and `glob`.

Default active tools depend on run mode:

| Mode | Default tools |
|------|---------------|
| Interactive TUI | `read`, `write`, `edit`, `bash` |
| Non-interactive / RPC | `read`, `grep`, `find`, `ls`, `glob` |
| Non-interactive / RPC with mutating opt-in | `read`, `write`, `edit`, `bash` |

File writes and edits are scoped to the harness workspace root. Interactive
`read` can inspect absolute paths and paths outside the workspace. These rules
are tool policy, not an operating-system sandbox.

## Config and Sessions

Config layers merge user config, project config, and an explicit `--config`
file. Model precedence is:

1. `--model`
2. `OPI_MODEL` when `--config` was not passed
3. `model` in `--config <FILE>`
4. `<CWD>/.opi/config.toml`
5. User config (`%APPDATA%\opi\config.toml` on Windows,
   `~/.config/opi/config.toml` on Unix)
6. Built-in defaults

Sessions are append-only JSONL files written automatically.

| Platform | Default session directory |
|----------|---------------------------|
| Windows | `%LOCALAPPDATA%\opi\sessions\` |
| Unix | `~/.local/share/opi/sessions/` |

Use `OPI_SESSIONS_DIR` to override the location.

## Workspace Crates

All crates share the workspace version, edition, license, repository, and
authors.

| Crate | Published | Purpose |
|-------|-----------|---------|
| [`opi-ai`](crates/opi-ai) | yes | Provider-neutral LLM API, streaming events, model registry, retries, HTTP/proxy support, usage and cost helpers. |
| [`opi-agent`](crates/opi-agent) | yes | Agent loop, tool execution, hooks, events, queues, sessions, compaction, SDK types, extensions, streaming proxy. |
| [`opi-tui`](crates/opi-tui) | yes | Ratatui widgets, transcript rendering, diff view, pickers, terminal images, themes, keybindings. |
| [`opi-coding-agent`](crates/opi-coding-agent) | yes | The `opi` binary and embeddable coding harness. |

Internal dependency shape:

```text
opi-ai
opi-tui
opi-agent -> opi-ai
opi-coding-agent -> opi-ai + opi-agent + opi-tui -> opi binary
```

## Extensibility

`opi --rpc` exposes an unstable 0.x JSONL command/event protocol with schema
version checks. `opi-agent` also exposes shared SDK types and extension
registry primitives for embedders. RPC commands include `prompt`, `continue`,
`steer`, `follow_up`, `abort`, `set_model`, `set_thinking_level`, `compact`,
`session_info`, `extension_command`, `trace`, and `quit`.

Resource discovery supports extensions, packages, skills, prompt fragments, and
themes. Package manifests can start `process-jsonl` adapters that expose custom
tools, commands, hooks, event observers, state, and model/provider overrides.

Packages are trusted code. Installing a package can start child processes with
the same OS permissions as `opi`; package permission declarations are currently
metadata, not enforced sandbox policy.

## Boundaries

- `opi` does not collect telemetry or analytics and does not share sessions
  automatically.
- `opi doctor` is local and network-free by default; it checks local config,
  provider credential presence, packages, sessions, TUI capability, and RPC
  schema information.
- Production sub-agent, permission-gate, plan/todo, and MCP workflows are not
  built into the core CLI. The repository contains examples and package
  scaffolds for those patterns.
- OAuth or subscription login flows are not implemented.
- Dynamic Rust plugin loading from arbitrary extension paths is not supported.

## Development

Rust 1.85 or newer is required because the workspace uses Rust edition 2024.

```sh
cargo build
cargo run -p opi-coding-agent -- --help
cargo test --workspace --all-targets
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

See [AGENTS.md](AGENTS.md) for repository working rules and
[docs/opi-spec.md](docs/opi-spec.md) for the current technical spec.

## License

MIT (c) OdradekAI. See [LICENSE](LICENSE).
