# opi-coding-agent

[![Crates.io](https://img.shields.io/crates/v/opi-coding-agent.svg)](https://crates.io/crates/opi-coding-agent)
[![Docs.rs](https://docs.rs/opi-coding-agent/badge.svg)](https://docs.rs/opi-coding-agent)

> The `opi` binary — a minimal terminal coding agent. Produced by this crate, built on [`opi-ai`](https://crates.io/crates/opi-ai), [`opi-agent`](https://crates.io/crates/opi-agent), and [`opi-tui`](https://crates.io/crates/opi-tui).

[简体中文](README.zh.md) · [← opi](../../README.md)

---

## Status (v0.2.0)

Phase 1 MVP. The interactive TUI and non-interactive (positional prompt or
`--non-interactive`) modes both work end-to-end against Anthropic. Six built-in
tools, TOML configuration, exit codes, and a high-risk-tool safety policy are in
place.

Not yet implemented: other providers, persistent sessions, sub-agents,
skills, slash commands, `/login` / OAuth.

## Install

```sh
cargo install opi-coding-agent
opi --version
```

Or download the pre-built binary for your platform from a
[GitHub Release](https://github.com/OdradekAI/opi/releases).

## Quick start

```sh
export ANTHROPIC_API_KEY=sk-ant-...

# Interactive (ratatui TUI)
opi

# Non-interactive — positional prompt to stdout, exit
opi "Find every TODO in this repo."

# Pick a different model
opi -m anthropic:claude-opus-4 "Explain src/main.rs"

# Allow write/edit/bash in non-interactive mode
opi "Add a CHANGELOG entry for the latest commit." --allow-mutating
```

## CLI flags

| Flag / arg | Description |
|------------|-------------|
| `[PROMPT]...` | Positional prompt text; non-empty args select non-interactive mode |
| `-m, --model <SPEC>` | Model spec (e.g. `anthropic:claude-sonnet-4`) |
| `-c, --config <FILE>` | Path to a TOML config file (must exist) |
| `-s, --system <FILE>` | System prompt file (prepended to the built-in prompt) |
| `--non-interactive` | Force non-interactive mode (prompt text still required) |
| `--allow-mutating` | Allow `write` / `edit` / `bash` in non-interactive mode |
| `-v, --verbose` | Enable debug tracing |

## Configuration

TOML files merge **user → project → `--config`** (later layers override earlier keys).

**Model** resolution (highest → lowest):

1. `--model` (CLI)
2. `OPI_MODEL` — only when `--config` was **not** passed
3. `model` in `--config <file>`
4. Project — `<CWD>/.opi/config.toml`
5. User — `%APPDATA%\opi\config.toml` (Windows) or `~/.config/opi/config.toml` (Unix)
6. Built-in defaults

`.opi/config.toml` shape (every field is optional; values shown are defaults):

```toml
[defaults]
model = "anthropic:claude-sonnet-4"
max_iterations = 50
tool_timeout_ms = 30000
theme = "default"
allow_mutating_tools = false

[thinking]
enabled = true
budget_tokens = 10000

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"  # override only if needed
```

Set `defaults.allow_mutating_tools = true` to skip `--allow-mutating` on every
non-interactive invocation.

## Built-in tools

Tools live in [`src/tool/`](src/tool):

| Tool | Args | Notes |
|------|------|-------|
| `read` | `path`, optional `offset` + `limit` | 1-based line range |
| `glob` | `pattern`, optional `path` | gitignore-aware |
| `grep` | `pattern`, optional `glob` / `path` | gitignore-aware, regex |
| `write` | `path`, `content` | mutating; needs `--allow-mutating` non-interactively |
| `edit` | `path`, `old_string`, `new_string` | exact-match replacement; mutating |
| `bash` | `command`, optional `timeout_secs` | uses `cmd.exe` on Windows, `sh` on Unix |

All paths are resolved relative to (and constrained within) the workspace root
passed to `CodingHarness::new` (the CLI uses the current working directory).

## Modes

### Non-interactive

`NonInteractiveRunner::run()` captures assistant text to `stdout`, diagnostics
to `stderr`, and returns one of these exit codes:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Runtime failure |
| `2` | Config error |
| `3` | Auth failure (missing or invalid API key) |
| `4` | Provider failure |
| `5` | Tool failure |
| `130` | Interrupted (Ctrl+C) |

### Interactive

`CodingHarness` + `InteractiveCodingHooks` drives a ratatui TUI built from
`opi-tui` widgets. Streaming text deltas update the live transcript, tool
calls render with status, and mutating tools surface confirmation.

## Library use

`opi-coding-agent` is also a regular library crate. The `harness` and
`runner` modules let you embed the same loop in your own Rust app:

```rust
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::harness::CodingHarness;

# async fn example(provider: Box<dyn opi_ai::Provider>) -> anyhow::Result<()> {
let config = OpiConfig::default();
let mut harness = CodingHarness::new(
    provider,
    config.defaults.model.clone(),
    config,
    std::env::current_dir()?,
);
let _messages = harness.prompt("Hello!").await?;
# Ok(()) }
```

## License

MIT — see workspace [`LICENSE`](../../LICENSE).
