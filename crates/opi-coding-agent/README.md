# opi-coding-agent

[![Crates.io](https://img.shields.io/crates/v/opi-coding-agent.svg)](https://crates.io/crates/opi-coding-agent)
[![Docs.rs](https://docs.rs/opi-coding-agent/badge.svg)](https://docs.rs/opi-coding-agent)

> The `opi` binary: an interactive and non-interactive terminal coding agent built on `opi-ai`, `opi-agent`, and `opi-tui`.

[Simplified Chinese](README.zh.md) | [opi workspace](../../README.md)

## Status

Current crate version: `0.4.0`.

This crate produces the `opi` CLI and exposes the coding harness as a Rust library. It supports interactive TUI mode, positional-prompt non-interactive mode, NDJSON output, RPC JSONL mode, nine provider prefixes, eight available built-in tools, pi-aligned interactive default tools, conservative non-interactive default tools, image attachments, model/session/branch pickers, shell completion generation, context file loading, session persistence, resume/list/delete session commands, context compaction, configurable keybindings/themes, per-provider proxy config, progressive resource discovery for packages/extensions/skills/fragments/themes, retry, token usage totals, and best-effort cost summaries.

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
| `--rpc` | RPC JSONL mode: bidirectional command/event protocol over stdin/stdout |

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

[extensions]
paths = ["vendor/my-extension"]

[packages]
paths = ["vendor/my-package"]
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

Available built-in tools are `read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`, and `glob`.

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

With no prompt args, `opi` starts the ratatui TUI. It uses `opi-tui` widgets for transcript rendering, input editing, status, markdown, tool calls, edit diffs, themes, keybindings, model/session/branch pickers, and terminal image output.

Slash commands:

| Command | Effect |
|---------|--------|
| `/model` | Open the model picker for the active provider |
| `/session` | Open the session picker |
| `/branch` | Open the branch picker for the active session |
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

### RPC JSONL mode

`--rpc` starts a persistent bidirectional JSONL session over stdin/stdout. This is the recommended embedding mode for IDEs, custom UIs, and external tool integration.

**This is an unstable 0.x protocol.** The schema may change between minor versions. Clients MUST check `schema_version` in the `rpc_ready` header.

```sh
opi --rpc
```

On startup, `opi` emits a `rpc_ready` header:

```json
{"type":"rpc_ready","schema_version":2,"mode":"rpc","version":"0.4.0"}
```

Commands are JSON objects sent to stdin, one per line. Responses and events are JSON objects emitted to stdout, one per line. Diagnostics go to stderr.

#### Commands

| Command | Description |
|---------|-------------|
| `prompt` | Send user prompt; agent events stream asynchronously |
| `continue` | Continue conversation with additional text |
| `steer` | Queue steering message during agent operation |
| `follow_up` | Queue follow-up message for after agent stops |
| `abort` | Cancel current agent operation |
| `set_model` | Switch provider:model |
| `set_thinking_level` | Set reasoning/thinking level |
| `compact` | Trigger manual compaction |
| `session_info` | Query session metadata |
| `quit` | Shut down the RPC session |

All commands support an optional `id` field for request/response correlation.

#### Response format

```json
{"type":"response","id":"req-1","command":"prompt","success":true}
{"type":"response","id":"req-2","command":"set_model","success":false,"error":"model not found"}
{"type":"response","id":"req-3","command":"session_info","success":true,"data":{"model":"anthropic:claude-sonnet-4"}}
```

For `prompt` and `continue`, `success: true` means the command was accepted. Agent events (including errors after acceptance) arrive as async event lines.

#### Error semantics

- **Parse errors**: `{"type":"response","command":"parse","success":false,"error":"..."}`
- **Command rejected**: `{"type":"response","command":"<cmd>","success":false,"error":"..."}`
- **Agent errors after acceptance**: emitted as regular agent events, not as a second response.

#### Cancellation

`abort` cancels the current agent operation via the cancellation token. The agent surfaces a `Cancelled` error through the normal event stream. A second `abort` while idle is a no-op.

#### Example

```python
import subprocess, json

proc = subprocess.Popen(
    ["opi", "--rpc"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
)

def send(cmd):
    proc.stdin.write(json.dumps(cmd) + "\n")
    proc.stdin.flush()

def read_line():
    return json.loads(proc.stdout.readline())

header = read_line()  # rpc_ready
send({"type": "session_info", "id": "1"})
resp = read_line()    # response with session info
send({"type": "quit"})
resp = read_line()    # response: quit success
```

## Context Files

`CodingHarness` discovers `AGENTS.md` and `CLAUDE.md` from the workspace directory upward to the git root, then from the user config directory. Empty files and files larger than 128 KiB are skipped.

## Resources and Packages

The harness discovers resource metadata from user, project, explicit, and package layers and exposes it in the system prompt and RPC/session metadata. Discovery covers:

- Extensions: directories containing `extension.toml`.
- Packages: directories containing `package.toml`; packages may compose extensions, skills, prompt fragments, and themes from conventional subdirectories.
- Skills: directories containing `SKILL.md` with YAML frontmatter.
- Prompt fragments: directories containing `FRAGMENT.md` with YAML frontmatter.
- Themes: directories containing `theme.toml`, resolved before falling back to built-in themes.

User-level resources live under the user config directory (`~/.config/opi/` on Unix, `%APPDATA%\opi\` on Windows). Project-level resources live under `.opi/` in the workspace root. Explicit extension and package paths come from config. Higher-precedence layers override lower-precedence layers; duplicates within the same layer are reported as diagnostics.

## Skills

Skills are progressively discovered from project, user, explicit, and package resources. Each skill is a directory containing a `SKILL.md` file with YAML frontmatter.

**This is an unstable 0.x API.** The skill format and discovery rules may change between minor versions.

### Skill format

A skill directory contains a `SKILL.md`:

```markdown
---
name: my-skill
description: What this skill does and when to use it.
disable-model-invocation: false
---

Full skill instructions go here.
```

Fields:

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Lowercase `a-z`, `0-9`, hyphens. Max 64 characters. |
| `description` | Yes | Max 1024 characters. |
| `disable-model-invocation` | No | Defaults to `false`. When `true`, the skill is excluded from automatic model invocation but still available for human use. |

### Discovery locations

Skills are discovered from multiple layers with precedence-based deduplication (higher precedence wins on name collision):

1. **User-level** (`~/.config/opi/skills/` on Unix, `%APPDATA%\opi\skills\` on Windows) — precedence 0
2. **Project-level** (`.opi/skills/` in workspace root) — precedence 1
3. **Explicit** resource layers supplied by an embedder — precedence 2
4. **Package-composed** resources from discovered packages, using the package layer precedence

Each skill is a subdirectory of a scan location containing a `SKILL.md` file.

### Progressive disclosure

Skill metadata (name, description) is available without loading the full skill body. The complete instructions are loaded on demand only when the skill is invoked. This keeps the initial context small while supporting rich, specialized instructions.

## Prompt Fragments

Prompt fragments (templates) are progressively discovered from project, user, explicit, and package resources. Each fragment is a directory containing a `FRAGMENT.md` file with YAML frontmatter.

**This is an unstable 0.x API.** The fragment format and discovery rules may change between minor versions.

### Fragment format

A fragment directory contains a `FRAGMENT.md`:

```markdown
---
name: translate
description: Translate text between languages.
arguments: text, from=en, to=fr
---

Translate {{text}} from {{from}} to {{to}}.
```

Fields:

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Lowercase `a-z`, `0-9`, hyphens. Max 64 characters. |
| `description` | Yes | Max 1024 characters. |
| `arguments` | No | Comma-separated list. Required: `name`. Optional: `name=default`. |

### Argument expansion

Arguments declared in the frontmatter are referenced as `{{name}}` placeholders in the body. During expansion:

- Required arguments must be provided.
- Optional arguments use their declared default when not provided.
- Undeclared placeholders are left as-is.

### Discovery locations

Fragments use the same precedence-based discovery as skills and extensions (higher precedence wins on name collision):

1. **User-level** (`~/.config/opi/fragments/` on Unix, `%APPDATA%\opi\fragments\` on Windows) — precedence 0
2. **Project-level** (`.opi/fragments/` in workspace root) — precedence 1
3. **Explicit** resource layers supplied by an embedder — precedence 2
4. **Package-composed** resources from discovered packages, using the package layer precedence

## Themes

Themes are discovered from `theme.toml` files in user, project, explicit, and package layers. A theme file contains metadata plus optional color token overrides:

```toml
name = "operator"
description = "Operator theme"

[colors]
role_user = "Green"
status_bg = "#1a1a2e"
```

Unknown tokens and invalid colors produce diagnostics. Missing color tokens inherit from the default theme. The runtime resolves discovered themes before built-in `default` and `monokai`.

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

Use `builder`, `new_with_hooks`, `new_with_hooks_and_resume`, `new_with_selection`, `subscribe`, `cancel`, `queue_images`, `prompt_with_content`, `model_picker_items`, `branch_picker_items`, `set_model`, `resource_metadata`, `resolve_theme`, and `session` when embedding the runtime in a custom application.

## License

MIT. See the workspace [LICENSE](../../LICENSE).
