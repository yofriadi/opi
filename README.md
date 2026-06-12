# opi

[![CI](https://github.com/OdradekAI/opi/actions/workflows/ci.yml/badge.svg)](https://github.com/OdradekAI/opi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> Rust AI agent toolkit that reimplements ideas from [earendil-works/pi](https://github.com/earendil-works/pi) as a terminal-first coding agent and reusable agent crates.

[Simplified Chinese](README.zh.md) | [Changelog](CHANGELOG.md) | [Spec](docs/opi-spec.md)

## Status

Current workspace version: `0.4.0`.

`opi` is a working terminal coding agent. It includes an interactive ratatui TUI, text and NDJSON non-interactive modes, RPC JSONL mode, eight built-in tools, image attachments, model/session/branch pickers, shell completion generation, layered TOML config, per-provider proxy config, multi-provider streaming, JSONL session persistence, context compaction, retry/backoff, configurable keybindings/themes, token usage accumulation, and best-effort cost summaries.

Extensibility surfaces are present and still unstable 0.x APIs: shared SDK/RPC command types, extension hooks/tools/state for embedders, layered resource discovery for extensions, packages, skills, prompt fragments, and themes, custom provider/model registration, and a streaming proxy. `opi-web-ui` remains `publish = false`; it is not a standalone browser app, but it provides reusable RPC/SDK event parsing, conversation state, component models, and HTML rendering.

## Relationship to pi

`opi` borrows pi's ideas and design boundaries, but it is not API-compatible with pi and does not read pi config or session files by default.

| Area | pi direction | opi treatment |
|------|--------------|---------------|
| Product surface | Minimal terminal coding harness | Terminal-first Rust coding agent and reusable Rust crates |
| Core coding tools | Default `read`, `write`, `edit`, `bash` | Same interactive default tool set |
| Read-only navigation | `read`, `grep`, `find`, `ls` | Same core read-only set; `glob` is an extra convenience and core workflows should not depend on it |
| Extensibility | Extensions, skills, prompt templates, themes, packages | RPC/SDK, extension APIs, resource discovery, skills, prompt fragments, themes, packages, and custom provider/model registration are implemented as unstable 0.x surfaces |
| Workflow-heavy features | MCP, sub-agents, plan mode, todos, and permission gates stay outside core | Keep them as extension/package examples instead of built-in core policy |
| Config and sessions | `.pi` JSON settings and pi session files | TOML config and opi JSONL sessions |
| Web UI | Available in pi's package set | Unpublished reusable component/state/rendering crate; no standalone browser app yet |

## Workspace

Cargo workspace with lockstep versioning. Every crate inherits `version`, `edition`, `license`, `repository`, and `authors` from `[workspace.package]`.

| Crate | Published | Description |
|-------|-----------|-------------|
| [`opi-ai`](crates/opi-ai) | yes | Provider-neutral LLM API, streaming events, image content, registry, retry, HTTP pooling/proxy, usage and cost helpers |
| [`opi-agent`](crates/opi-agent) | yes | Agent loop, tool execution, hooks, events, queues, sessions, compaction, SDK types, extension API, and streaming proxy primitives |
| [`opi-tui`](crates/opi-tui) | yes | Ratatui widgets, transcript rendering, diff view, select/branch pickers, terminal images, themes, keybindings |
| [`opi-coding-agent`](crates/opi-coding-agent) | yes | The `opi` binary and embeddable coding harness |
| [`opi-web-ui`](crates/opi-web-ui) | no (`publish = false`) | RPC/SDK event parser, conversation state, component models, and HTML rendering helpers |

Internal dependency shape:

```text
opi-ai (no internal deps)
opi-tui (no internal deps)
opi-agent -> opi-ai
opi-web-ui (no internal deps, publish = false)
opi-coding-agent -> opi-ai + opi-agent + opi-tui -> opi binary
```

## Install

The executable is named `opi` and is produced by the `opi-coding-agent` crate.

```sh
cargo install opi-coding-agent
opi --version
```

Pre-built binaries for Linux, macOS, and Windows on x64 and arm64 are attached to [GitHub Releases](https://github.com/OdradekAI/opi/releases).

## Quick Start

Set credentials for the provider you want to use:

```sh
export ANTHROPIC_API_KEY=sk-ant-...
# or OPENAI_API_KEY, OPENROUTER_API_KEY, MISTRAL_API_KEY, GEMINI_API_KEY
# or AWS credentials for Bedrock, AZURE_OPENAI_API_KEY, VERTEX_ACCESS_TOKEN
```

Or log in to OpenAI Codex subscription (no API key needed):

```sh
opi login
# or device-code flow: opi login --device
# check status: opi login status
# logout: opi logout
```
Run the interactive TUI:

```sh
opi
```

Run one prompt and print assistant text to stdout:

```sh
opi "List the Rust crates in this workspace."
```

Emit NDJSON events for automation:

```sh
opi --json "Summarize the latest session state."
```

Attach images to the initial prompt:

```sh
opi --image screenshot.png "Review this UI."
opi --image before.png --image after.png "Compare these images."
```

Pick a model with `provider:model` syntax:

```sh
opi -m anthropic:claude-sonnet-4-5-20250514 "Explain crates/opi-agent/src/lib.rs"
opi -m openai:gpt-4o "Review the public API shape."
opi -m openai-responses:gpt-4o-mini "Find documentation gaps."
opi -m openrouter:openai/gpt-4o-mini "List TODO comments."
opi -m mistral:codestral-latest "Explain the tool modules."
opi -m gemini:gemini-2.5-flash "Summarize the README files."
opi -m bedrock:anthropic.claude-sonnet-4-20250514-v2:0 "Summarize this repo."
opi -m azure:my-deployment "Use my Azure OpenAI deployment."
opi -m vertex:gemini-2.5-flash "Use Vertex AI."
opi -m openai-codex:gpt-5.5 "Explain crates/opi-agent/src/lib.rs"
```

## Supported Providers

Provider support is implemented in `opi-ai` and wired into `opi-coding-agent`.

| Model prefix | Backend | Default credentials |
|--------------|---------|---------------------|
| `anthropic:` | Anthropic Messages SSE | `ANTHROPIC_API_KEY` |
| `openai:` | OpenAI Chat Completions SSE | `OPENAI_API_KEY` |
| `openai-responses:` | OpenAI Responses SSE | `OPENAI_API_KEY` |
| `openai-codex:` | OpenAI Codex Responses SSE | OAuth login (`opi login` credentials) |
| `openrouter:` | OpenAI-compatible OpenRouter profile | `OPENROUTER_API_KEY` |
| `mistral:` | OpenAI-compatible Mistral profile | `MISTRAL_API_KEY` |
| `gemini:` | Gemini `streamGenerateContent` SSE | `GEMINI_API_KEY` |
| `bedrock:` | AWS Bedrock Converse streaming with SigV4 | AWS env vars or shared AWS config/credentials |
| `azure:` | Azure OpenAI Chat Completions deployment | `AZURE_OPENAI_API_KEY` plus config endpoint |
| `vertex:` | Google Vertex AI Gemini streaming | `VERTEX_ACCESS_TOKEN` plus config project/location |

Use `opi --list-models` to list models advertised by configured providers. Add `--json` for machine-readable output.

## Built-in Tools

Tools are implemented by `opi-coding-agent` and exposed through the `opi-agent::Tool` trait.

| Tool | Args | Execution | Mutating |
|------|------|-----------|----------|
| `read` | `path`, optional `offset`, `limit` | parallel | no |
| `ls` | `path`, optional `max_entries`, `max_depth` | parallel | no |
| `glob` | `pattern` | parallel | no |
| `find` | `pattern`, optional `path` | parallel | no |
| `grep` | `pattern` | parallel | no |
| `write` | `path`, `content` | sequential | yes |
| `edit` | `path`, `old_string`, `new_string` | sequential | yes |
| `bash` | `command`, optional `timeout_secs` | sequential | yes |

Path policy is mode-aware. Writes, edits, and non-interactive file tools are workspace-root scoped by default. Interactive `read` can resolve absolute paths and paths outside the workspace for inspection, and file tool details report the workspace root, resolved path, and whether the path is inside the workspace. Mutating tools require `--allow-mutating` or `defaults.allow_mutating_tools = true` in non-interactive/RPC runs, so unattended and edge-device runs stay read-only unless the caller explicitly opts into writes or shell execution.

Tool selection flags:

```sh
opi --tools read,grep "Inspect the code without edits."
opi --no-tools "Answer using only conversation context."
opi --no-builtin-tools "Run without built-in tools."
```

## Configuration

Config layers merge user config, project config, and explicit config files. Model precedence is:

1. `--model`
2. `OPI_MODEL` when `--config` was not passed
3. `model` in `--config <FILE>`
4. `<CWD>/.opi/config.toml`
5. User config (`%APPDATA%\opi\config.toml` on Windows, `~/.config/opi/config.toml` on Unix)
6. Built-in defaults

Example:

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

[extensions]
paths = ["vendor/my-extension"]

[packages]
paths = ["vendor/my-package"]
```

When a provider proxy is not configured, `opi` falls back to standard `HTTP_PROXY`, `HTTPS_PROXY`, and `NO_PROXY` environment variables.

## Interactive Mode

With no prompt args, `opi` starts the ratatui TUI. Useful commands inside the input box:

| Command | Effect |
|---------|--------|
| `/model` | Open the model picker for the active provider |
| `/session` | Open the session picker and resume a stored session |
| `/branch` | Open the branch picker for the active session |
| `/image <path>` | Queue an image for the next prompt |
| `exit` or `quit` | Exit the TUI |

Default keybindings are `enter` to submit, `escape` to abort/exit, and `alt+enter` for a new line. They can be changed in `[keybindings]`.

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

Session files store a header plus message, compaction, and leaf entries. Resume reconstructs the active branch and preserves compaction summaries. `--json` emits session events, retry events, compaction events, thinking-level events, and a final session summary with token totals and optional cost totals.

## Context Files

The coding harness discovers `AGENTS.md` and `CLAUDE.md` from the workspace directory upward to the git root, then from the user config directory. Files over 128 KiB and empty files are ignored. `OPI.md` is intentionally not loaded.

## RPC, SDK, and Extensions

`opi --rpc` starts a persistent JSONL command/event session over stdin/stdout. It emits an initial `rpc_ready` header with `schema_version = 2`; commands include `prompt`, `continue`, `abort`, `steer`, `follow_up`, `set_model`, `set_thinking_level`, `compact`, `session_info`, and `quit`. Responses are correlated by optional `id`, while accepted prompt output streams as async agent events.

The shared SDK types live in `opi_agent::sdk`. The extension API in `opi-agent` supports lifecycle hooks, custom tools, custom commands, custom agent messages/state, and custom provider/model registration for embedders. The CLI discovers configured resource metadata from user, project, package, and explicit paths and exposes it in prompts/RPC metadata. It does not dynamically load arbitrary Rust code from disk.

Packages can compose extensions, skills, prompt fragments, and themes from flat `package.toml` manifests. Skills and prompt fragments use progressive disclosure: metadata is discovered up front, while bodies are loaded only when needed. Themes can be discovered from `theme.toml` resources and are resolved before falling back to built-in `default` and `monokai`. Duplicate resource names within the same discovery layer are errors; higher-precedence layers override lower-precedence layers.

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

- Session/model/completion commands are handled early and exit.
- Non-interactive mode is selected by prompt args, `--non-interactive`, or `--json`; it builds a provider and runs `NonInteractiveRunner`.
- Interactive mode is the default with no prompt args; it builds a `CodingHarness` with interactive hooks and launches the ratatui TUI.

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
- `opi_agent::sdk`: shared SDK/RPC command and event types for programmatic embedding.

## Not Built In

- Production sub-agent, permission-gate, plan/todo, and MCP workflows. The repository includes package/example scaffolds, but these are not built-in core product workflows.
- Runtime expansion of prompt fragments as interactive slash commands.
- Dynamic Rust plugin loading from arbitrary extension paths.
- A standalone browser-hosted web app.

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
- See `AGENTS.md` for repository working rules used by humans and agents.

Bug reports and PRs are welcome at <https://github.com/OdradekAI/opi/issues>.

## License

MIT (c) OdradekAI. See [LICENSE](LICENSE).
