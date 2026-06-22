# opi-coding-agent

[![Crates.io](https://img.shields.io/crates/v/opi-coding-agent.svg)](https://crates.io/crates/opi-coding-agent)
[![Docs.rs](https://docs.rs/opi-coding-agent/badge.svg)](https://docs.rs/opi-coding-agent)

> The `opi` binary and embeddable coding harness.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.5.3`, inherited from the workspace package version.

This crate connects `opi-ai`, `opi-agent`, and `opi-tui` into a terminal coding
agent. It provides:

- the `opi` CLI binary;
- interactive ratatui TUI mode;
- one-shot text mode and `--json` NDJSON mode;
- `--rpc` JSONL command/event mode;
- model, session, branch, and session-tree pickers;
- image attachments through `--image` and `/image`;
- session list/resume/fork/delete commands;
- eight built-in tools;
- config, context-file loading, session persistence, compaction, retry, usage,
  cost summaries, package/resource discovery, diagnostics, and opt-in traces.

The crate is usable as a library through `CodingHarness`, but most users should
start with the CLI.

## Install

```sh
cargo install opi-coding-agent
opi --version
```

Pre-built binaries are attached to
[GitHub Releases](https://github.com/OdradekAI/opi/releases).

## Quick Start

```sh
export ANTHROPIC_API_KEY=sk-ant-...

# Interactive TUI
opi

# One prompt, assistant text to stdout
opi "Find TODO comments in this repository."

# NDJSON event stream
opi --json "Summarize this workspace."

# Select a provider/model
opi -m openai:gpt-4o "Explain crates/opi-coding-agent/src/main.rs"

# Attach images to the first prompt
opi --image screenshot.png "Review this screenshot."

# Allow write/edit/bash in non-interactive automation
opi --allow-mutating "Update the README."
```

## CLI Commands and Flags

Run `opi --help` for the exact current surface. Important commands and flags:

| Command / flag | Purpose |
|----------------|---------|
| `[PROMPT]...` | Non-empty positional prompt selects one-shot text mode. |
| `-m, --model <SPEC>` | Model spec such as `anthropic:claude-sonnet-4-5-20250514`. |
| `-c, --config <FILE>` | Explicit TOML config file; it must exist. |
| `-s, --system <FILE>` | Append a user system prompt file to the built-in coding prompt. |
| `--non-interactive` | Force one-shot text mode; prompt text is still required. |
| `--json` | Emit NDJSON session/agent events to stdout. |
| `--rpc` | Start bidirectional JSONL command/event mode over stdin/stdout. |
| `--allow-mutating` | Allow `write`, `edit`, and `bash` outside interactive mode. |
| `--tools <TOOLS>` | Comma-separated built-in tool allowlist. |
| `--no-tools` | Disable all tools. |
| `--no-builtin-tools` | Disable built-in tools while leaving extension/custom tools available. |
| `--image <PATH>` | Attach one image to the initial prompt; repeatable. |
| `--list-models` | List models exposed by configured providers and exit. |
| `--list-sessions` | List stored sessions and exit. |
| `--resume <ID>` | Resume a stored session. |
| `--fork <ID>` | Fork a stored session into a new session. |
| `--delete-session <ID>` | Delete a stored session and exit. |
| `--generate-completion <SHELL>` | Generate completion for `bash`, `zsh`, `fish`, `powershell`, or `elvish`. |
| `--trace <PATH>` | Write an opt-in, redacted local trace envelope for a non-interactive/JSON run. |
| `doctor [--json] [--scope ...]` | Local, network-free health check. |
| `package <add|remove|list|doctor>` | Manage local/git extension packages. |

## Providers

| Prefix | Backend | Default credentials/config |
|--------|---------|----------------------------|
| `anthropic:` | `AnthropicProvider` | `ANTHROPIC_API_KEY` |
| `openai:` | `OpenAiChatProvider` | `OPENAI_API_KEY` |
| `openai-responses:` | `OpenAiResponsesProvider` | `OPENAI_API_KEY` |
| `openrouter:` | OpenRouter profile | `OPENROUTER_API_KEY` |
| `mistral:` | Mistral profile | `MISTRAL_API_KEY` |
| `gemini:` | `GeminiProvider` | `GEMINI_API_KEY` |
| `bedrock:` | `BedrockProvider` | AWS env vars or shared AWS profile/config |
| `azure:` | `AzureOpenAIProvider` | `AZURE_OPENAI_API_KEY`; endpoint/deployments in config |
| `vertex:` | `VertexProvider` | `VERTEX_ACCESS_TOKEN`; project/location in config |
| configured profile | OpenAI-compatible profile | profile-specific `api_key_env`, `base_url`, and model list |

Provider credential env names, base URLs, model lists, and proxies can be
overridden in config.

## Built-in Tools

Tools live under `src/tool/`.

| Tool | Args | Notes |
|------|------|-------|
| `read` | `path`, optional `offset`, `limit` | 1-based line offset; parallel. |
| `ls` | `path`, optional `max_entries`, `max_depth` | Deterministic directory listing; gitignore-aware; parallel. |
| `glob` | `pattern` | Gitignore-aware file discovery; parallel. |
| `find` | `pattern`, optional `path` | Gitignore-aware file discovery scoped to an optional subdirectory; parallel. |
| `grep` | `pattern` | Gitignore-aware regex search; parallel. |
| `write` | `path`, `content` | Creates parent dirs; sequential; mutating. |
| `edit` | `path`, `old_string`, `new_string` | Replaces the first exact match and records before/after details; sequential; mutating. |
| `bash` | `command`, optional `timeout_secs` | Runs in workspace root via `cmd /C` on Windows or `sh -c` on Unix; sequential; mutating. |

Default active tools:

| Mode | Tools |
|------|-------|
| Interactive | `read`, `write`, `edit`, `bash` |
| Non-interactive / RPC | `read`, `grep`, `find`, `ls`, `glob` |
| Non-interactive / RPC with mutating opt-in | `read`, `write`, `edit`, `bash` |

In non-interactive/RPC mode, explicit allowlists containing `write`, `edit`, or
`bash` require `--allow-mutating` or `defaults.allow_mutating_tools = true`.

## Modes

### Interactive

With no prompt args, `opi` starts the ratatui TUI. Slash commands include:

| Command | Effect |
|---------|--------|
| `/model` | Open the model picker for the active provider. |
| `/session` | Open the session picker. |
| `/branch` | Open the branch picker. |
| `/tree` | Open the session tree picker. |
| `/fork` | Fork the active branch into a new parented session. |
| `/clone` | Clone the active branch into a new parented session. |
| `/image <path>` | Queue an image for the next prompt. |
| `exit` / `quit` | Exit. |

### Non-interactive and JSON

Text mode writes assistant text to stdout and diagnostics to stderr. `--json`
writes a schema header, serialized session/agent events, and a final
`session_summary` as NDJSON.

Exit codes:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Runtime failure |
| `2` | Config error |
| `3` | Auth failure |
| `4` | Provider failure |
| `5` | Tool failure |
| `130` | Interrupted |

### RPC JSONL

`--rpc` starts a persistent bidirectional JSONL protocol for IDEs, custom UIs,
and other embedders. This is an unstable 0.x protocol; clients must check the
`schema_version` in the `rpc_ready` header. The current SDK/RPC schema version
is `3`. Startup diagnostics are surfaced in the `startup_diagnostics` field of
that ready header.

Commands include `prompt`, `continue`, `steer`, `follow_up`, `abort`,
`set_model`, `set_thinking_level`, `compact`, `session_info`,
`extension_command`, `trace`, and `quit`.

## Config, Sessions, and Context Files

Config layers merge user config, project config, and explicit `--config` files.
Model precedence is `--model`, then `OPI_MODEL` when no `--config` was passed,
then explicit config, project `.opi/config.toml`, user config, and built-in
defaults.

User config paths:

- Windows: `%APPDATA%\opi\config.toml`
- Unix: `~/.config/opi/config.toml`

Sessions are append-only JSONL files under `%LOCALAPPDATA%\opi\sessions\` on
Windows and `~/.local/share/opi/sessions/` on Unix, unless `OPI_SESSIONS_DIR`
is set.

`CodingHarness` loads `AGENTS.md` and `CLAUDE.md` from the workspace ancestors
up to the git root, then from the user config directory. Empty files and files
larger than 128 KiB are ignored. `OPI.md` is intentionally not loaded.

## Resources and Packages

Resource discovery covers extensions, packages, skills, prompt fragments, and
themes from user, project, explicit, and package layers. Higher-precedence
layers override lower-precedence layers; duplicate names within the same layer
are reported as diagnostics.

Package commands:

```sh
opi package add ./vendor/todo
opi package add --local ./vendor/todo
opi package add git:github.com/user/pkg@v1
opi package list
opi package list --json
opi package doctor
opi package doctor --json
opi package remove todo
```

Packages can start `process-jsonl` adapters using the
`opi-extension-jsonl-v1` protocol. That adapter protocol is an unstable 0.x
contract. Packages are trusted code and are not sandboxed by the package
manager.

## Library Use

`CodingHarness` is the embedding entry point. It can be built directly or
through `CodingHarness::builder`, with optional custom hooks, session resume
data, tool selection, runtime package state, resource metadata, and startup
diagnostics.

Common methods include `prompt`, `prompt_with_content`, `queue_images`,
`subscribe`, `cancel`, `set_model`, `model_picker_items`, `branch_picker_items`,
`resource_metadata`, `resolve_theme`, and `session`.

## Boundaries

- `opi` does not collect telemetry or analytics and does not share sessions
  automatically.
- `opi doctor` makes no paid model calls or network checks by default.
- Mutating-tool policy is not an OS sandbox.
- Production sub-agent, permission-gate, plan/todo, and MCP workflows are
  examples/package patterns, not built-in core workflows.
- OAuth or subscription login flows are not implemented.

## License

MIT. See the workspace [LICENSE](../../LICENSE).
