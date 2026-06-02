# opi-coding-agent

[![Crates.io](https://img.shields.io/crates/v/opi-coding-agent.svg)](https://crates.io/crates/opi-coding-agent)
[![Docs.rs](https://docs.rs/opi-coding-agent/badge.svg)](https://docs.rs/opi-coding-agent)

> The `opi` binary: an interactive and non-interactive terminal coding agent built on `opi-ai`, `opi-agent`, and `opi-tui`.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.4.0`.

This crate produces the `opi` CLI and exposes the coding harness as a Rust library. It supports interactive TUI mode, positional-prompt non-interactive mode, NDJSON output, nine provider prefixes, eight available built-in tools, pi-aligned interactive default tools, conservative non-interactive default tools, image attachments, model/session pickers, shell completion generation, context file loading, session persistence, resume/list/delete session commands, context compaction, configurable keybindings/themes, per-provider proxy config, retry, token usage totals, and best-effort cost summaries.

## Install

```sh
cargo install opi-coding-agent
opi --version
```

Or download a pre-built binary from a [GitHub Release](https://github.com/OdradekAI/opi/releases).

## Quick Start

```sh
export ANTHROPIC_API_KEY=sk-ant-...

# Interactive TUI
opi

# Single prompt, assistant text to stdout
opi "Find all TODO comments in this repository."

# NDJSON event stream for automation
opi --json "Summarize this workspace."

# Pick a provider/model
opi -m openai:gpt-4o "Explain crates/opi-coding-agent/src/main.rs"

# Attach images to the first prompt
opi --image screenshot.png "Review this screenshot."

# Allow mutating tools in non-interactive automation
opi --allow-mutating "Update the README."
```

## CLI Flags

| Flag / arg | Description |
|------------|-------------|
| `[PROMPT]...` | Positional prompt text; non-empty args select non-interactive mode |
| `-m, --model <SPEC>` | Model spec such as `anthropic:claude-sonnet-4-5-20250514` |
| `-c, --config <FILE>` | Explicit TOML config file; must exist |
| `-s, --system <FILE>` | User system prompt file appended to the built-in coding prompt |
| `--non-interactive` | Force non-interactive mode; prompt text is still required |
| `--allow-mutating` | Allow `write`, `edit`, and `bash` in non-interactive mode |
| `--json` | Output NDJSON events to stdout; also uses non-interactive mode |
| `--list-sessions` | List stored sessions and exit |
| `--resume <ID>` | Resume a stored session by id |
| `--delete-session <ID>` | Delete a stored session by id and exit |
| `--generate-completion <SHELL>` | Generate shell completions for `bash`, `zsh`, `fish`, `powershell`, or `elvish` |
| `-v, --verbose` | Enable debug tracing |
| `--tools <TOOLS>` | Comma-separated active tool allowlist, for example `read,grep` |
| `--no-tools` | Disable all tools |
| `--no-builtin-tools` | Disable built-in tools; reserved for extension/custom tools |
| `--image <IMAGE>` | Attach one image file to the initial prompt; can be repeated |
| `--list-models` | List available models from configured providers and exit |

## Providers

`opi-coding-agent` builds a provider from the configured model prefix.

| Prefix | Provider | Default credentials/config |
|--------|----------|----------------------------|
| `anthropic:` | `AnthropicProvider` | `ANTHROPIC_API_KEY` |
| `openai:` | `OpenAiChatProvider` | `OPENAI_API_KEY` |
| `openai-responses:` | `OpenAiResponsesProvider` | `OPENAI_API_KEY` |
| `openrouter:` | OpenRouter profile | `OPENROUTER_API_KEY` |
| `mistral:` | Mistral profile | `MISTRAL_API_KEY` |
| `gemini:` | `GeminiProvider` | `GEMINI_API_KEY` |
| `bedrock:` | `BedrockProvider` | AWS env vars or shared AWS profile/config |
| `azure:` | `AzureOpenAIProvider` | `AZURE_OPENAI_API_KEY`; endpoint/deployments in config |
| `vertex:` | `VertexProvider` | `VERTEX_ACCESS_TOKEN`; project/location in config |

Environment variable names, base URLs, provider-specific fields, and proxies can be overridden in config.

## Configuration

Config layers merge in this order: user config, project config, explicit `--config` file. Later layers override earlier fields.

Model precedence:

1. `--model`
2. `OPI_MODEL` only when `--config` was not passed
3. `model` in `--config <FILE>`
4. `<CWD>/.opi/config.toml`
5. User config
6. Built-in defaults

Full shape with common defaults:

```toml
[defaults]
model = "anthropic:claude-sonnet-4"
max_iterations = 50
tool_timeout_ms = 30000
max_image_bytes = 20971520
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
# base_url = "https://api.openai.com"

[providers.openai_responses]
api_key_env = "OPENAI_API_KEY"
# base_url = "https://api.openai.com"

[providers.openrouter]
api_key_env = "OPENROUTER_API_KEY"
# base_url = "https://openrouter.ai/api"
# referer = "https://example.com"

[providers.mistral]
api_key_env = "MISTRAL_API_KEY"
# base_url = "https://api.mistral.ai"

[providers.gemini]
api_key_env = "GEMINI_API_KEY"
# base_url = "https://generativelanguage.googleapis.com"

[providers.bedrock]
region = "us-east-1"
# profile = "default"
# base_url = "https://bedrock-runtime.us-east-1.amazonaws.com"
# secret_access_key_env = "AWS_SECRET_ACCESS_KEY"
# session_token_env = "AWS_SESSION_TOKEN"

[providers.azure]
api_key_env = "AZURE_OPENAI_API_KEY"
endpoint = "https://my-resource.openai.azure.com"
api_version = "2024-06-01"
deployments = ["my-deployment"]

[providers.vertex]
access_token_env = "VERTEX_ACCESS_TOKEN"
project = "my-gcp-project"
location = "us-central1"
models = ["gemini-2.5-flash", "gemini-2.5-pro"]

[providers.openai.proxy]
url = "http://proxy.example.com:8080"
no_proxy = "localhost,127.0.0.1"
```

If a provider-specific proxy is not configured, the HTTP client falls back to `HTTP_PROXY`, `HTTPS_PROXY`, and `NO_PROXY`.

## Built-in Tools

Tools live in `src/tool/`.

| Tool | Args | Notes |
|------|------|-------|
| `read` | `path`, optional `offset`, `limit` | 1-based line offset; parallel |
| `ls` | `path`, optional `max_entries`, `max_depth` | Deterministic directory listing; gitignore-aware; parallel |
| `glob` | `pattern` | Gitignore-aware file discovery; parallel |
| `find` | `pattern`, optional `path` | Gitignore-aware file discovery scoped to an optional subdirectory; parallel |
| `grep` | `pattern` | Gitignore-aware regex search; parallel |
| `write` | `path`, `content` | Creates parent dirs; sequential; mutating |
| `edit` | `path`, `old_string`, `new_string` | Replaces first exact match and records before/after details; sequential; mutating |
| `bash` | `command`, optional `timeout_secs` | Runs in workspace root via `cmd /C` on Windows or `sh -c` on Unix; sequential; mutating |

Available built-in tools are `read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`, and additional `glob`.

Default active tools depend on run mode:

- Interactive mode: `read`, `write`, `edit`, `bash`.
- Non-interactive mode: `read`, `grep`, `find`, `ls`, `glob`.
- Non-interactive mode with `--allow-mutating` or `defaults.allow_mutating_tools = true`: `read`, `write`, `edit`, `bash`.

Use `--tools <TOOLS>` to provide an explicit active tool allowlist. In non-interactive mode, allowlists containing `write`, `edit`, or `bash` require `--allow-mutating` or `defaults.allow_mutating_tools = true`.

Path policy is mode-aware. File writes and edits are restricted to the harness workspace root. Interactive `read` can resolve absolute paths and paths outside the workspace; non-interactive file tools remain workspace-only by default. File tool details include `workspace_root`, `resolved_path`, and `inside_workspace`.

Tool selection precedence is `--no-tools` > `--tools` > `--no-builtin-tools` > default.

## Images

`--image <PATH>` attaches images to the first prompt in interactive or non-interactive mode. The flag can be repeated. Interactive mode also accepts `/image <path>` to queue an image for the next prompt.

Supported formats are PNG, JPEG, GIF, and WebP. The default file-size limit is 20 MiB and can be changed with `defaults.max_image_bytes`.

## Sessions

Sessions are persisted automatically through `SessionCoordinator`.

Default storage:

- Windows: `%LOCALAPPDATA%\opi\sessions\`
- Unix: `~/.local/share/opi/sessions/`

Override with `OPI_SESSIONS_DIR`.

```sh
opi --list-sessions
opi --resume <session-id> "Continue the work."
opi --delete-session <session-id>
```

Resume reconstructs the active branch from session JSONL entries. If a session contains compaction markers, the resumed context includes the compaction summary and kept tail.

## Modes

### Interactive

With no prompt args, `opi` starts the ratatui TUI. It uses `opi-tui` widgets for transcript rendering, input editing, status, markdown, tool calls, edit diffs, themes, keybindings, model/session pickers, and terminal image output.

Slash commands:

| Command | Effect |
|---------|--------|
| `/model` | Open the model picker for the active provider |
| `/session` | Open the session picker |
| `/image <path>` | Queue an image for the next prompt |
| `exit` or `quit` | Exit |

### Text non-interactive

With prompt args or `--non-interactive`, `NonInteractiveRunner::run()` captures assistant text to stdout and diagnostics to stderr.

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

### JSON non-interactive

`--json` emits NDJSON to stdout. The first line is a schema header, followed by serialized session/agent events and a final `session_summary` with token totals and optional cost totals.

## Context Files

`CodingHarness` discovers `AGENTS.md` and `CLAUDE.md` from the workspace directory upward to the git root, then from the user config directory. Empty files and files larger than 128 KiB are skipped.

## Library Use

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
let _messages = harness.prompt("Hello").await?;
# Ok(()) }
```

Use `new_with_hooks`, `new_with_hooks_and_resume`, `new_with_selection`, `subscribe`, `cancel`, `queue_images`, `prompt_with_content`, `model_picker_items`, `set_model`, and `session` when embedding the runtime in a custom application.

## License

MIT. See the workspace [LICENSE](../../LICENSE).
