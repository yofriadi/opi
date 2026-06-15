# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `opi-coding-agent`: `--fork <session-id>` plus interactive `/tree`, `/fork`, and `/clone` session commands that copy the active branch into a new parented session without rewriting the source JSONL file.
- `opi-agent` / `opi-coding-agent`: RPC/SDK `extension_command` support for dispatching correlated custom commands to registered extension registries.
- `opi-coding-agent`: config-driven OpenAI-compatible provider profiles with model metadata, compatibility flags, runtime provider construction, and registry-backed `--list-models` output.
- `opi-web-ui`: `ConversationState` now tracks resource metadata from `session_info` responses and the last successful compaction response payload.
- `opi-coding-agent`: runtime session persistence now writes meaningful `parent_id` links and `leaf` pointers so continuing from a selected branch tip creates a same-file branch path.
- `opi-coding-agent`: `opi package add/remove/list/doctor` now validates package manifests, writes lock entries, and reports installed package diagnostics.
- `opi-coding-agent`: manifest V2 supports `[adapter]` process adapters with the `opi-extension-jsonl-v1` JSONL protocol.
- `opi-coding-agent`: installed package declarations are loaded during runtime startup so adapter tools, commands, hooks, events, state, and cancellation bridge into the extension API.
- `opi-coding-agent`: example adapter packages demonstrate todo state, permission-gate example hooks, and protected path hooks through a runnable process adapter.

### Changed

- `opi-agent`: moved the core loop implementation out of `lib.rs` into an internal `agent_loop` module while preserving the public `opi_agent::agent_loop` export.

### Fixed

- `opi-tui`: `SelectList` and `BranchPicker` now account for selected-row markers and CJK display width when aligning labels with metadata.
- `opi-coding-agent`: `opi package doctor` now rejects invalid manifest V2 adapter declarations and reports lock/source/resource/adapter diagnostics.
- `opi-coding-agent`: Adapter state snapshots are persisted in session JSONL and restored on resume.
- `opi-coding-agent`: Adapter event drops are diagnostic-visible, shutdown allows a bounded graceful exit, local package identity is canonicalized, SSH git source parsing is URL-aware, and relative adapter commands cannot escape package roots.

## [0.5.0] - 2026-06-07

Phase 4: extension system, RPC JSONL protocol, SDK embedding surface,
progressive resource discovery, session branching, streaming proxy,
custom provider registration, and six extension examples.

### Added

- `opi-coding-agent`: RPC JSONL mode with correlated responses, async agent events, session/model/thinking/compaction commands, and tool-selection support.
- `opi-agent`: shared unstable SDK command/response/event types for embedders.
- `opi-agent`: extension API with lifecycle hooks, custom tools, custom commands, custom messages, and extension state.
- `opi-ai`: custom provider/model registry APIs used by CLI model listing and runtime validation.
- `opi-coding-agent`: config-driven discovery for extensions, packages, skills, prompt fragments, and themes, including package-composed resource layers.
- `opi-coding-agent`: interactive `/branch` session branch selection.
- `opi-agent`: streaming proxy primitives with framing, cancellation, backpressure, and secret redaction.
- `opi-web-ui`: unpublished RPC/SDK event parser, conversation state, component models, and HTML rendering helpers.
- `opi-agent`: session branching with tree reconstruction, branch picker, and branch-aware session writer.
- `opi-tui`: branch picker widget with snapshot-tested rendering.
- `opi-coding-agent`: extension examples for MCP adapter, todo, plan mode, sub-agent, protected paths, and permission gate patterns.
- `opi-coding-agent`: progressive discovery for themes, prompt fragments, skills, and package resources.

### Changed

- `opi-coding-agent`: `--list-models`, interactive model picking, and runtime model validation now use provider registry metadata.
- `opi-coding-agent`: example package manifests use the supported flat `package.toml` schema.
- `opi-agent`: `StreamingProxy::run` is synchronous transport-agnostic I/O instead of an async wrapper around blocking reads.

### Fixed

- `opi-coding-agent`: Windows subprocess tests resolve `opi.exe` correctly.
- `opi-web-ui`: RPC response `data` is preserved and updates session/model state.
- `opi-coding-agent`: same-layer duplicate resource/package names now produce explicit errors.
- `opi-coding-agent`: package resource containment checks no longer fall back to unresolved paths when canonicalization fails.
- `opi-agent`: default secret redaction no longer redacts short benign `sk-` or `eyJ`-like strings.
- `opi-agent`: `SdkResponse` now round-trips through JSON and serialization fallback events use `SdkSerializationError`.
- `opi-web-ui`: `ThinkingBlock` is re-exported from the crate root with the other component models.
- `opi-coding-agent`: phase 4 ledger hash check normalized for cross-platform consistency.

### Removed

- `opi-agent`: stale public `Transport` stub.

## [0.4.0] - 2026-06-02

Phase 3: cloud provider expansion (Vertex AI, Azure OpenAI, Bedrock), image
support across the stack, new built-in tools (find, ls), fuzzy picker, terminal
image rendering, shell completions, and proxy support.

### Added

- `opi-ai`: AWS Bedrock provider with SigV4 signing and credential resolution
- `opi-ai`: Azure OpenAI provider with deployment URL and api-key auth
- `opi-ai`: Google Vertex AI provider with OAuth Bearer auth
- `opi-ai`: HTTP/HTTPS proxy support with env-var and per-provider config
- `opi-ai`: image input support for multimodal prompts
- `opi-ai`: shared HttpClient with connection pooling
- `opi-agent`: image tool result support for visual tool output
- `opi-agent`: `prompt_with_content` method for arbitrary content (text + images)
- `opi-coding-agent`: `--list-models` flag to list available models (table or NDJSON)
- `opi-coding-agent`: `--image` flag for non-interactive image attachment
- `opi-coding-agent`: `/image` slash command for TUI image attachment
- `opi-coding-agent`: `find` built-in tool for file search
- `opi-coding-agent`: `ls` built-in tool for directory listing with metadata
- `opi-coding-agent`: shell completion generation for bash, zsh, fish, powershell, elvish
- `opi-coding-agent`: pi-style tool selection and safety hooks
- `opi-coding-agent`: AGENTS.md / CLAUDE.md context file loading
- `opi-coding-agent`: global context file discovery from user config directory
- `opi-coding-agent`: proxy wiring to all provider factory paths
- `opi-coding-agent`: enhanced tool management and path resolution
- `opi-tui`: fuzzy model/session picker with SelectList widget
- `opi-tui`: terminal image rendering with protocol detection (kitty, sixel, iTerm2)

### Performance

- `opi-ai`: shared HttpClient with connection pooling reduces TLS handshake overhead

### Fixed

- `opi-ai`: Bedrock error mapping now parses Retry-After header for 429 responses
- `opi-ai`: Azure OpenAI endpoint validation -- missing endpoint returns config error
- `opi-ai`: Bedrock URL-sourced images rejected with clear unsupported-error message
- `opi-agent`: compaction summary includes image content placeholders
- `opi-coding-agent`: ls tool truncation count now correctly reports omitted entries
- `opi-coding-agent`: session picker sorted newest-first to avoid filesystem ordering flakes
- `opi-coding-agent`: char-aware truncation in session picker to avoid non-ASCII panic
- `opi-coding-agent`: `--list-models --json` uses serde_json for properly escaped output
- `opi-coding-agent`: `--image` files passed through to interactive mode first prompt
- `opi-coding-agent`: session files excluded from crate package
- `opi-tui`: terminal image protocol hardening

## [0.3.0] - 2026-05-25

Phase 2 hardening: multi-provider support (6 LLM providers), session
persistence, context compaction, configurable TUI, and cost tracking.

### Added

- `opi-ai`: OpenAI-compatible chat provider with SSE streaming
- `opi-ai`: OpenAI Responses API provider with streaming
- `opi-ai`: Google Gemini provider with HTTP streaming
- `opi-ai`: Mistral provider profile
- `opi-ai`: OpenRouter provider profile
- `opi-ai`: retry/backoff/rate-limit support with configurable strategies
- `opi-ai`: usage accumulation and cost tracking across turns
- `opi-agent`: session v1 JSONL storage for conversation persistence
- `opi-agent`: compaction engine with trigger and hook support
- `opi-agent`: thinking config passed through to provider requests
- `opi-agent`: enhanced event handling and message management
- `opi-coding-agent`: session list/resume/delete CLI flags
- `opi-coding-agent`: session persistence wired into harness runtime
- `opi-coding-agent`: compaction wired into session coordinator
- `opi-coding-agent`: `--json` NDJSON output mode for non-interactive use
- `opi-coding-agent`: provider factory extended for all 6 providers
- `opi-coding-agent`: usage accumulation wired to TUI status bar
- `opi-coding-agent`: edit tool captures before/after content
- `opi-coding-agent`: workspace path validation for all tools
- `opi-tui`: configurable keybindings with TOML parsing
- `opi-tui`: Theme struct with default and monokai palettes
- `opi-tui`: DiffView widget for edit/patch visualization

### Fixed

- Session runtime tests serialized to avoid env var races

## [0.2.0] - 2026-05-22

Phase 1 MVP: functional Anthropic-based coding assistant with six tools,
basic TUI, TOML config, and mock-provider integration tests.

### Added

- `opi-ai`: message and stream types with 12 `AssistantStreamEvent` variants
- `opi-ai`: `Provider` trait with `stream(Request) -> EventStream`, `Request`,
  `ThinkingConfig`, `ModelInfo`, `ProviderError`
- `opi-ai`: Anthropic SSE provider with hand-written SSE parser and
  `AnthropicMapper` for event translation
- `opi-ai`: provider registry resolving `anthropic:model` specs with capability
  queries
- `opi-ai`: shared `MockProvider` test harness with builder helpers
- `opi-agent`: `Tool` trait with JSON Schema validation via `jsonschema`
- `opi-agent`: `agent_loop` with turn lifecycle, tool batching (parallel/sequential),
  cancellation via `CancellationToken`, and queue polling
- `opi-agent`: `Agent` wrapper with `prompt`, `continue_`, `abort`, `subscribe`
- `opi-agent`: hooks (`AgentHooks`) with `after_tool_call`, `should_stop_after_turn`,
  `prepare_next_turn`, steering and follow-up queues
- `opi-coding-agent`: `ReadTool`, `WriteTool`, `EditTool`, `BashTool` with workspace
  safety boundaries and confirmation policy
- `opi-coding-agent`: `GlobTool`, `GrepTool` with gitignore-aware file search
- `opi-coding-agent`: `SystemPromptBuilder` with layered prompt construction
- `opi-coding-agent`: TOML config loading with CLI > env > project > user > defaults
  precedence
- `opi-coding-agent`: non-interactive mode with exit codes and high-risk tool safety
  policy
- `opi-coding-agent`: interactive TUI mode using ratatui/crossterm
- `opi-tui`: TUI shell with `MessageList`, `InputEditor`, `StatusBar`, `ToolCallView`
- `opi-tui`: `MarkdownView` and `CodeBlock` rendering widgets
- 213 integration and unit tests across all crates

### Fixed

- SSE parser surfaces malformed events instead of silently dropping them
- SSE parser handles CRLF line endings for cross-platform robustness
- `BashTool` uses `cmd.exe` on Windows, `sh` on Unix
- Agent loop emits `ToolExecutionStart` before parallel tool spawning
- `AuthFailed` error variant maps to exit code 3
- Config: explicit `--config` with non-existent file returns error
- Config: `--config` model not overridden by `OPI_MODEL` env var
- Agent loop uses `tokio::select!` for responsive stream cancellation
- Tool call `input` serialized as JSON object, not string

## [0.1.1] - 2026-05-20

### Added

- `opi-implement` skill for structured implementation workflows with
  phased gates, verification tiers, and JSON ledger tracking.
- CI workflows: `ci.yml` (fmt, clippy, test, doc) and `release.yml`
  (cross-platform binary builds on tag push).
- Opi technical specification document (`docs/opi-spec.md`).

### Fixed

- Release skill: keep SHA256SUMS local-only, use version-based artifact
  directory.

### Changed

- `opi-web-ui` marked as `publish = false` (not ready for crates.io).

## [0.1.0] - 2026-05-20

Initial scaffolding release. Establishes the workspace layout and crate
boundaries; functional implementations land in subsequent releases.

### Added

- Cargo workspace with five crates under lockstep versioning:
  - `opi-ai` — unified multi-provider LLM API (module scaffolding for
    `provider`, `stream`, `model`, `config`).
  - `opi-tui` — terminal UI library (module scaffolding for `render`,
    `editor`, `markdown`).
  - `opi-agent` — agent runtime with tool calling and transport
    abstraction (module scaffolding for `tool`, `transport`, `state`).
  - `opi-web-ui` — reusable web chat components (module scaffolding for
    `components`).
  - `opi-coding-agent` — produces the `opi` binary; supports `--version`
    and `--help`.
- `opi-release` skill (`.claude/skills/opi-release/skill.md`) implementing
  a seven-phase release workflow with explicit irreversibility gates.

### Notes

- All crate APIs are placeholders. Calling them will not do anything
  useful yet.
- This release is published as a GitHub Release only; crates.io publish
  is deferred until the crates have real implementations.

[0.5.0]: https://github.com/OdradekAI/opi/releases/tag/v0.5.0
[0.4.0]: https://github.com/OdradekAI/opi/releases/tag/v0.4.0
[0.3.0]: https://github.com/OdradekAI/opi/releases/tag/v0.3.0
[0.2.0]: https://github.com/OdradekAI/opi/releases/tag/v0.2.0
[0.1.1]: https://github.com/OdradekAI/opi/releases/tag/v0.1.1
[0.1.0]: https://github.com/OdradekAI/opi/releases/tag/v0.1.0
