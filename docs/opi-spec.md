# Opi Technical Specification

> Opi is a Rust reimplementation of the [pi](https://github.com/earendil-works/pi) AI agent toolkit. It preserves pi's runtime semantics while using Rust-native APIs, storage formats, and release practices.

## 0. Document Control

| Field | Value |
|---|---|
| Status | Draft |
| Spec version | 0.2-draft |
| Last updated | 2026-05-20 |
| Repository | `https://github.com/OdradekAI/opi` |
| Upstream studied | `pi` 0.75.3 at `.repo/pi-0.75.3/` |
| Current implementation | `opi` 0.1.0 scaffolding |
| Next milestone | 0.2.0 Phase 1 MVP |

This document is normative for the current design. Changes that alter public APIs, event protocols, session storage, release behavior, or phase boundaries SHOULD update this file in the same change.

Normative terms:

- **MUST** means required for conformance.
- **SHOULD** means expected unless a documented reason says otherwise.
- **MAY** means optional extension behavior.

## 1. Executive Summary

Opi mirrors pi's package structure with five Rust crates:

- `opi-ai`: provider-agnostic LLM streaming.
- `opi-agent`: agent loop, stateful agent, hooks, tools, queues, and session primitives.
- `opi-tui`: terminal UI components.
- `opi-coding-agent`: the `opi` CLI binary.
- `opi-web-ui`: unpublished future web UI placeholder.

The repository is currently a 0.1.0 scaffolding release. Workspace layout, CI, release workflow, crate boundaries, and the binary name exist; functional implementations do not. Phase 1 turns these placeholders into a useful Anthropic-based coding assistant with six tools, a basic TUI, TOML config, and mock-provider integration tests.

The central design rule:

> Preserve pi's behavior where users and integrators depend on it. Do not preserve pi's TypeScript APIs, npm extension ABI, config files, or session files by default.

## 2. Design Philosophy

| Principle | pi 0.75.3 | opi design |
|---|---|---|
| Minimal core | `CONTRIBUTING.md` says features outside core belong in extensions | Phase 1-3 avoid plugin/MCP/web scope creep |
| Layered runtime | `agentLoop` -> `Agent` -> `AgentHarness` / `AgentSession` | `agent_loop` -> `Agent` -> `Harness` / `CodingHarness` |
| Streaming first | `AssistantMessageEventStream` and agent event streams | `Stream<Item = Result<Event, Error>>` with terminal events |
| Provider agnostic | API, provider, and model are separate concepts | `Provider` trait, registry, provider adapters |
| Agent vs LLM messages | `AgentMessage[] -> transformContext -> convertToLlm -> Message[]` | app messages in `opi-agent`, provider messages in `opi-ai` |
| Tool isolation | TypeBox schema at LLM boundary | typed Rust tool inputs, generated JSON Schema at the LLM boundary |
| Errors in band | provider failures become `error` stream events | provider/runtime failures surface as events, not panics |
| Append-only sessions | crash-safe JSONL session files | opi versioned tree JSONL inspired by pi |
| Lockstep release | all packages share a version | all crates share `workspace.package.version` |

### 2.1 Non-Goals

Opi is not a line-by-line port. Rust's enums, traits, ownership, and cancellation primitives should shape the implementation.

Opi is not API-compatible with pi. TypeScript declaration merging, `jiti` extension loading, and npm package exports do not map cleanly to Rust crates and a static binary.

Opi is not required to read pi config or pi session files in Phase 1. A migration command MAY be added later, but runtime compatibility is not assumed.

Opi is not an extensibility platform in its MVP. MCP is a later protocol adapter after core tools and permissions are stable; WASM plugins, subprocess plugins, RPC, multi-agent orchestration, and web UI work are later extension surfaces.

## 3. Relationship to pi

Pi is the behavioral reference. The following behavior should be treated as inherited design, not incidental implementation detail.

### 3.1 Semantics Opi MUST Preserve

| Area | Required behavior | Upstream anchor |
|---|---|---|
| Agent event order | `agent_start`, `turn_start`, message events, tool events, `turn_end`, `agent_end` | `packages/agent/README.md` |
| Provider stream lifecycle | `start`, content deltas, content end events, then `done` or `error` | `packages/ai/src/types.ts` |
| Errors in stream | failures after request start are stream errors and final failed assistant messages | `StreamFunction` contract |
| Message conversion | app messages are transformed before provider conversion | `AgentMessage` / `convertToLlm` |
| Tool batching | parallel by default; any sequential tool makes the whole batch sequential | pi agent README |
| Tool result order | completion events may be completion order; persisted tool-result messages follow assistant source order | pi agent README |
| Tool termination | early stop only when every finalized result in the batch has `terminate` | pi agent README |
| Tool hooks | before hook can block; after hook replaces fields without deep merge | pi hook result types |
| `shouldStopAfterTurn` | runs after `turn_end`, before steering/follow-up polling | pi agent README |
| Steering queue | delivered after current assistant turn and tool calls, before next provider call | pi agent README and RPC docs |
| Follow-up queue | delivered only when the agent would otherwise stop | pi agent README and RPC docs |
| Session durability | append-only writes and recovery from incomplete final line | pi session manager |

### 3.2 Rust-Native Redesigns

| pi mechanism | opi replacement | Rationale |
|---|---|---|
| TypeScript unions and declaration merging | Rust enums plus explicit extension variants | exhaustive matching and safer evolution |
| TypeBox schemas | `schemars`-generated JSON Schema plus `jsonschema` validation | dynamic provider boundary, static tool code |
| dynamic provider imports | feature flags plus explicit registration | predictable binaries and cross-compilation |
| `jiti` TypeScript extensions | deferred Rust-compatible plugin story | avoids Node dependency and unstable ABI in MVP |
| pi `settings.json` / `auth.json` | TOML config and explicit credential resolution | Rust ecosystem convention and comments |
| pi session v3 | opi session v1 tree JSONL | retain branch/compaction semantics without TS-specific entries |
| custom TUI renderer | `ratatui` + `crossterm` | active Rust terminal stack |

### 3.3 Feature Parity Matrix

| pi capability | Opi phase | Compatibility target |
|---|---:|---|
| package/crate layout | Phase 0 done | structural parity |
| binary | Phase 0 placeholder, Phase 1 useful | `opi`, not `pi` |
| provider streaming | Phase 1 | semantic parity |
| Anthropic provider | Phase 1 | semantic parity |
| `agentLoop` / `Agent` | Phase 1 | semantic parity |
| read/write/edit/bash/glob/grep tools | Phase 1 | behavior parity |
| interactive TUI | Phase 1 | user-facing parity |
| OpenAI-compatible/OpenRouter/OpenAI/Gemini/Mistral | Phase 2 | provider contract parity |
| sessions/resume | Phase 2 | opi format |
| compaction | Phase 2 | semantic parity |
| JSON event mode | Phase 2 | versioned opi NDJSON |
| image support | Phase 3 | semantic parity |
| permissions | Phase 3 | opi UX |
| MCP client adapter | Phase 3 | protocol adapter after core tools and permissions |
| extensions/RPC/web UI | Phase 4 | opi-specific design |

## 4. Current Baseline

### 4.1 Version 0.1.0

| Area | Current state |
|---|---|
| Workspace | five crates under one Cargo workspace |
| Versioning | lockstep `0.1.0` |
| Edition | Rust 2024 |
| Internal dependencies | `opi-agent -> opi-ai`, `opi-web-ui -> opi-ai`, `opi-coding-agent -> opi-ai + opi-agent + opi-tui` |
| External dependencies | `tokio`, `serde`, `serde_json`, `thiserror`, `async-trait` |
| Binary | `opi` supports `--version` and `--help` |
| CI | `fmt`, `clippy`, `test`, `doc` |
| Release CI | six platform binary workflow |
| crates.io | deferred until real implementations exist |

### 4.2 Stub API Drift

Current APIs are placeholders and MAY be broken before 0.2.0.

| Crate | Current placeholder | Target |
|---|---|---|
| `opi-ai` | `Provider::complete(&[Value]) -> Vec<StreamEvent>` | `Provider::stream(Request) -> EventStream` |
| `opi-ai` | simple `StreamEvent` enum | start/delta/end/done/error protocol |
| `opi-agent` | `Tool::execute(Value) -> Value` | schema validation, cancellation, updates, execution modes |
| `opi-agent` | `AgentState` struct with raw values | `Agent`, queues, state enum, subscribers |
| `opi-agent` | `transport` stub | reserve for Phase 4 or remove before stable API |
| `opi-tui` | empty renderer/editor stubs | ratatui components |
| `opi-coding-agent` | minimal handwritten CLI | `clap` CLI and runtime wiring |
| `opi-web-ui` | placeholder `ChatWidget` | deferred unpublished crate |

### 4.3 Phase 0 Completion

Phase 0 is complete:

- five-crate workspace;
- lockstep versioning;
- placeholder modules and re-exports;
- CI gates;
- six-platform release workflow;
- `opi --version` and `opi --help`;
- GitHub Release only, crates.io deferred.

## 5. Workspace and Dependencies

### 5.1 Layout

```text
opi/
|-- Cargo.toml
|-- crates/
|   |-- opi-ai/
|   |-- opi-agent/
|   |-- opi-coding-agent/
|   |-- opi-tui/
|   `-- opi-web-ui/
|-- docs/
|-- .github/workflows/
`-- .claude/skills/opi-release/
```

The earlier draft's root `config/` directory is not present. Built-in themes or syntax assets should live in the owning crate until a real shared asset need appears.

### 5.2 Dependency Graph

```text
opi-ai           (no internal deps)
opi-tui          (no internal deps)
opi-agent        -> opi-ai
opi-web-ui       -> opi-ai
opi-coding-agent -> opi-ai, opi-agent, opi-tui
```

Internal dependencies MUST be declared in root `[workspace.dependencies]` and referenced by consumers with `{ workspace = true }`.

### 5.3 Crate Roles

| Crate | Type | Publish target | Role |
|---|---|---|---|
| `opi-ai` | library | crates.io from 0.2.0 target | provider protocols, model metadata, provider-facing messages |
| `opi-agent` | library | crates.io from 0.2.0 target | loop, agent, hooks, tools, queues, sessions |
| `opi-tui` | library | crates.io from 0.2.0 target | terminal rendering library |
| `opi-coding-agent` | binary | crates.io from 0.2.0 target | `opi` CLI application |
| `opi-web-ui` | library | not published | future browser UI placeholder |

### 5.4 Why There Is No `opi-types`

Types belong to the crate that owns their semantics:

- provider-facing `Message`, `ToolDef`, `ModelInfo`, and `Usage` belong in `opi-ai`;
- runtime `AgentMessage`, `AgentEvent`, `Tool`, and `SessionEntry` belong in `opi-agent`;
- CLI config belongs in `opi-coding-agent`;
- visual state belongs in `opi-tui`.

A shared types crate would become a hub dependency. If a type crosses a crate boundary, the lower semantic owner should expose it directly. Public enums expected to grow SHOULD use `#[non_exhaustive]` before API stabilization.

### 5.5 Dependency Plan

Phase 1 dependencies SHOULD be introduced with the narrowest feature set that can
ship the MVP. Prefer explicit features, optional heavy functionality, and later
phase additions over broad defaults.

| Category | Crate | Status | Rationale |
|---|---|---|---|
| async runtime | `tokio` | present, narrow features | networking, process IO, signals, timers; avoid `features = ["full"]` unless a concrete need appears |
| serialization | `serde`, `serde_json` | present | provider/session protocols |
| library errors | `thiserror` | present | typed error handling |
| async traits | `async-trait` | present, use sparingly | existing scaffolding convenience; target APIs should prefer boxed futures/streams when clearer |
| HTTP/SSE | `reqwest` with `rustls-tls` | Phase 1, narrow features | provider streaming without OpenSSL; use `default-features = false` and enable only required HTTP/JSON/stream features |
| streams | `futures-core`, internal stream helpers as needed | Phase 1 | public stream APIs should expose `futures-core::Stream`; keep helpers such as `futures-util` internal |
| cancellation | `tokio-util` | Phase 1 | cooperative cancellation |
| CLI | `clap` | Phase 1 | stable options and completions |
| config | `toml` | Phase 1 | human-editable config |
| TUI | `ratatui`, `crossterm` | Phase 1 | cross-platform terminal UI |
| schema | `schemars`, `jsonschema` | Phase 1, tool boundary first | typed tool schemas plus runtime validation at the model/tool boundary; avoid broad protocol validation until schemas stabilize |
| IDs/time | `uuid`, `time` | Phase 1 | session IDs and timestamps without `chrono`'s extra surface |
| file search | `ignore`, `globset`, `regex` | Phase 1 | gitignore-aware glob and grep behavior |
| tracing | `tracing`, `tracing-subscriber` | Phase 1/2 | observability |
| markdown/code | `pulldown-cmark`, optional `syntect` later | Phase 1/2 | basic markdown first; syntax highlighting must be optional or later so it does not threaten binary size targets |
| diff | `similar` | Phase 2 | patch visualization; do not add before a real diff view ships |

## 6. Architecture

### 6.1 Layers

```text
opi-coding-agent
  CLI, built-in tools, config, prompts, permissions, app-level session UX

CodingHarness / Harness
  session persistence, compaction, app hooks, model/thinking state, queues

Agent
  stateful runtime wrapper, subscriptions, cancellation, prompt/continue API

agent_loop
  pure LLM -> tool -> LLM loop, no persistence or UI policy

opi-ai Provider
  provider HTTP, SSE parsing, model metadata, provider-facing messages
```

`agent_loop` MUST be testable with mock providers and mock tools without disk or terminal state. `Agent` adds state, cancellation, queues, and event subscription. `Harness` composes sessions, compaction, and app hooks.

### 6.2 Harness Boundary

Pi 0.75.3 has both reusable `AgentHarness` code and the coding agent's `AgentSession`. Opi should not accidentally duplicate that split.

- `opi-agent` SHOULD own generic harness primitives needed by non-CLI consumers.
- `opi-coding-agent` SHOULD own coding-specific behavior: built-in file tools, project context, permission prompts, CLI config, and app-level session commands.
- If a feature is required by both library consumers and the CLI, it belongs in `opi-agent`; otherwise it stays in `opi-coding-agent`.

### 6.3 Runtime Flow

```text
user input
  -> CLI parses mode and config
  -> CodingHarness loads or creates session
  -> system prompt is built from base prompt, tools, project context, summaries
  -> Agent receives prompt, steer, follow-up, or continue request
  -> agent_loop transforms AgentMessage to provider Message
  -> provider streams assistant events
  -> agent emits message updates
  -> tool calls are validated and executed
  -> tool result messages are appended in assistant source order
  -> should_stop_after_turn runs
  -> steering queue is polled
  -> follow-up queue is polled only if the agent would otherwise stop
  -> session entries are appended
  -> subscribers settle after agent_end
```

### 6.4 Boundary Rules

- Provider adapters MUST NOT execute tools.
- Tools MUST NOT call providers directly unless the tool is explicitly an integration.
- TUI components MUST consume events and snapshots; they MUST NOT own loop policy.
- Session storage MUST NOT be required for `agent_loop` tests.
- CLI shortcuts MUST NOT leak into `opi-agent` unless they describe reusable runtime behavior.

## 7. Protocols and Data Models

Opi has four related protocols. They MUST stay distinct.

| Protocol | Owner | Purpose |
|---|---|---|
| Provider stream events | `opi-ai` | normalize provider chunks into assistant deltas |
| Agent events | `opi-agent` | loop/message/tool lifecycle for UI and tests |
| Agent session events | harness / `opi-coding-agent` | queues, compaction, retry, session metadata |
| Session entries | storage | persisted records used to rebuild context |

### 7.1 Provider-Facing Messages

```rust
#[non_exhaustive]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    ToolResult(ToolResultMessage),
}

pub struct UserMessage {
    pub content: Vec<InputContent>,
    pub timestamp_ms: i64,
}

pub struct AssistantMessage {
    pub content: Vec<AssistantContent>,
    pub api: ApiKind,
    pub provider: String,
    pub model: String,
    pub response_model: Option<String>,
    pub response_id: Option<String>,
    pub usage: Usage,
    pub stop_reason: StopReason,
    pub error_message: Option<String>,
    pub timestamp_ms: i64,
}

pub struct ToolResultMessage {
    pub tool_call_id: String,
    pub tool_name: String,
    pub content: Vec<OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub timestamp_ms: i64,
}
```

Stop reasons SHOULD stay close to pi: `stop`, `length`, `tool_use`, `error`, `aborted`.

### 7.2 Agent Messages

```rust
#[non_exhaustive]
pub enum AgentMessage {
    Llm(opi_ai::Message),
    CompactionSummary(CompactionSummaryMessage),
    BranchSummary(BranchSummaryMessage),
    Custom(CustomAgentMessage),
}
```

Before each provider call:

1. `transform_context` works at `AgentMessage` level.
2. `convert_to_llm` converts to `Vec<opi_ai::Message>` and filters session/UI-only messages.

Unknown custom messages MUST NOT panic the runtime.

### 7.3 Provider Stream Events

```rust
#[non_exhaustive]
pub enum AssistantStreamEvent {
    Start { partial: AssistantMessage },
    TextStart { content_index: usize, partial: AssistantMessage },
    TextDelta { content_index: usize, delta: String, partial: AssistantMessage },
    TextEnd { content_index: usize, content: String, partial: AssistantMessage },
    ThinkingStart { content_index: usize, partial: AssistantMessage },
    ThinkingDelta { content_index: usize, delta: String, partial: AssistantMessage },
    ThinkingEnd { content_index: usize, content: String, partial: AssistantMessage },
    ToolCallStart { content_index: usize, partial: AssistantMessage },
    ToolCallDelta { content_index: usize, delta: String, partial: AssistantMessage },
    ToolCallEnd { content_index: usize, tool_call: ToolCall, partial: AssistantMessage },
    Done { reason: StopReason, message: AssistantMessage },
    Error { reason: StopReason, message: AssistantMessage },
}
```

Every provider stream MUST emit `Start` before deltas and terminate with exactly one `Done` or `Error`. Once a request has started, request/model/runtime failures SHOULD become `Error` events with final assistant messages instead of out-of-band failures.

### 7.4 Agent Events

```rust
#[non_exhaustive]
pub enum AgentEvent {
    AgentStart,
    AgentEnd { messages: Vec<AgentMessage> },
    TurnStart,
    TurnEnd { message: AgentMessage, tool_results: Vec<opi_ai::ToolResultMessage> },
    MessageStart { message: AgentMessage },
    MessageUpdate { message: AgentMessage, assistant_event: AssistantStreamEvent },
    MessageEnd { message: AgentMessage },
    ToolExecutionStart { tool_call_id: String, tool_name: String, args: serde_json::Value },
    ToolExecutionUpdate { tool_call_id: String, tool_name: String, args: serde_json::Value, partial_result: serde_json::Value },
    ToolExecutionEnd { tool_call_id: String, tool_name: String, result: serde_json::Value, is_error: bool },
}
```

`MessageUpdate` is assistant-only. `AgentEnd` means no more loop events will be emitted, but awaited subscribers MAY still be settling.

### 7.5 Session Events

```rust
#[non_exhaustive]
pub enum AgentSessionEvent {
    Agent(AgentEvent),
    QueueUpdate { steering: Vec<String>, follow_up: Vec<String> },
    CompactionStart { reason: CompactionReason },
    CompactionEnd { reason: CompactionReason, result: Option<CompactionResult>, aborted: bool, will_retry: bool, error_message: Option<String> },
    AutoRetryStart { attempt: u32, max_attempts: u32, delay_ms: u64, error_message: String },
    AutoRetryEnd { success: bool, attempt: u32, final_error: Option<String> },
    SessionInfoChanged { session_id: String, name: Option<String> },
    ThinkingLevelChanged { level: ThinkingLevel },
}
```

`--json` mode emits one JSON object per line. The event protocol MUST include a schema version before downstream tooling treats it as stable.

### 7.6 Queues

```rust
pub enum QueueMode {
    All,
    OneAtATime,
}
```

Steering messages are delivered after the current assistant turn and its tool calls complete, before the next provider request. Follow-up messages are delivered only when the agent has no tool calls and no steering messages and would otherwise stop. If `should_stop_after_turn` returns true, the loop exits before polling either queue.

## 8. Crate Specifications

### 8.1 `opi-ai`

`opi-ai` owns provider-facing message types, model metadata, provider registry, credential helpers, and streaming adapters.

```rust
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn models(&self) -> &[ModelInfo];
    fn stream(&self, request: Request) -> EventStream;
}

pub type EventStream =
    Pin<Box<dyn Stream<Item = Result<AssistantStreamEvent, ProviderError>> + Send>>;
```

`stream` returns a stream handle. Cancellation is propagated through `Request::cancel` or an equivalent token. Dropping the stream SHOULD cancel the underlying HTTP request.

```rust
pub struct Request {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub thinking: ThinkingConfig,
    pub stop_sequences: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub cancel: CancellationToken,
}
```

Provider priority:

| Provider | API style | Phase | Reason |
|---|---|---:|---|
| Anthropic | Messages SSE | 1 | MVP target and pi's default model family |
| OpenAI-compatible chat | SSE | 2 | broad compatibility across OpenAI-style services |
| OpenRouter | OpenAI-compatible router | 2 | fast model coverage expansion and routing diagnostics |
| OpenAI Responses | SSE | 2 | separate event mapping |
| Google Gemini | streaming generateContent | 2 | major non-OpenAI family |
| Mistral | chat SSE | 2 | provider matrix expansion |
| AWS Bedrock | response stream / SigV4 | 3 | enterprise auth complexity |
| Azure OpenAI | OpenAI-compatible | 3 | deployment-name differences |
| Google Vertex | OAuth/service account | 3 | enterprise auth complexity |

Credential precedence:

1. explicit CLI/config override;
2. provider-specific environment variable;
3. local auth storage when implemented;
4. ambient cloud credential chain.

Secrets MUST NOT be logged, persisted in sessions, or included in diagnostics.

### 8.2 `opi-agent`

`opi-agent` is usable without the `opi` binary.

```rust
pub trait Tool: Send + Sync {
    fn definition(&self) -> opi_ai::ToolDef;

    fn execute(
        &self,
        call_id: &str,
        arguments: serde_json::Value,
        signal: CancellationToken,
        on_update: Option<UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>>;

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Parallel
    }
}

pub enum ExecutionMode {
    Sequential,
    Parallel,
}

pub struct ToolResult {
    pub content: Vec<opi_ai::OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub terminate: bool,
}
```

Built-in tools SHOULD define typed Rust argument structs deriving `Deserialize` and `schemars::JsonSchema`. `ToolDef` exposes the generated JSON Schema to providers, while dynamic input from the model is validated with `jsonschema` before deserialization. `serde_json::Value` is acceptable at protocol boundaries and for diagnostics, but tool business logic should not remain Value-driven.

Argument validation happens after `ToolExecutionStart` and before `before_tool_call`. Validation failure becomes an error tool result.

Execution rules:

- global sequential means all calls run sequentially;
- global parallel means calls run concurrently unless any target tool is sequential;
- if any tool in a batch is sequential, the entire batch runs sequentially;
- persisted tool-result messages are ordered by assistant source order.

Hook surface:

```rust
pub trait AgentHooks: Send + Sync {
    fn transform_context(&self, messages: Vec<AgentMessage>, signal: CancellationToken)
        -> Pin<Box<dyn Future<Output = Result<Vec<AgentMessage>, AgentError>> + Send>>;

    fn convert_to_llm(&self, messages: &[AgentMessage])
        -> Result<Vec<opi_ai::Message>, AgentError>;

    fn before_tool_call(&self, ctx: BeforeToolCallContext)
        -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>>;

    fn after_tool_call(&self, ctx: AfterToolCallContext)
        -> Pin<Box<dyn Future<Output = AfterToolCallResult> + Send>>;

    fn should_stop_after_turn(&self, ctx: ShouldStopAfterTurnContext)
        -> Pin<Box<dyn Future<Output = bool> + Send>>;

    fn prepare_next_turn(&self, ctx: PrepareNextTurnContext)
        -> Pin<Box<dyn Future<Output = Option<AgentLoopTurnUpdate>> + Send>>;
}
```

`after_tool_call` uses field replacement semantics and MUST NOT deep-merge `content` or `details`.

The low-level loop:

```rust
pub async fn agent_loop(
    context: AgentLoopContext,
    config: AgentLoopConfig,
    hooks: &dyn AgentHooks,
    events: AgentEventSink,
    cancel: CancellationToken,
) -> Result<Vec<AgentMessage>, AgentError>;
```

`Agent` wraps the loop with state, prompt/continue methods, abort, steering and follow-up queues, subscriber management, and idle settlement. Continuing requires the last context message to be user or tool result.

The current `transport` stub is reserved for Phase 4 RPC/proxy transport. Before 0.2.0 it must either be hidden as unstable or removed.

### 8.3 `opi-tui`

Phase 1 components:

| Component | Phase | Purpose |
|---|---:|---|
| `MessageList` | 1 | streaming conversation display |
| `InputEditor` | 1 | multi-line prompt input |
| `StatusBar` | 1 | model, state, token/cost summary placeholder |
| `ToolCallView` | 1 | tool call arguments and status |
| `MarkdownView` | 1 | basic markdown text |
| `CodeBlock` | 1/2 | syntax-highlighted code blocks |
| `DiffView` | 2 | edit and patch visualization |
| `SelectList` | 3 | session/model picker |

The TUI target is user-visible behavior, not renderer compatibility with pi: low flicker, responsive streaming, resize safety, Windows compatibility, and graceful degradation on small terminals.

Phase 1 should remain a minimal usable TUI: streaming messages, prompt input, status, and tool-call visibility. Themes, fuzzy pickers, rich diff views, and syntax highlighting beyond basic fenced-code presentation belong in later phases or optional features.

### 8.4 `opi-coding-agent`

The binary owns CLI parsing, config loading, provider registry construction, built-in tools, system prompt construction, session UX, permission prompts, and runtime modes.

| Tool | Mode | Phase | Scope |
|---|---|---:|---|
| `read` | parallel | 1 | read file content with optional line range |
| `write` | sequential | 1 | create or replace file |
| `edit` | sequential | 1 | exact string replacement or structured patch |
| `bash` | sequential | 1 | subprocess command with timeout and streamed output |
| `glob` | parallel | 1 | gitignore-aware file discovery by glob |
| `grep` | parallel | 1 | gitignore-aware regex search over file contents |

Phase 1 MUST include a minimal safety boundary for high-risk tools. `write`,
`edit`, and `bash` must show the proposed path or command, effective cwd,
environment policy, timeout, and whether the target is inside the workspace
before execution. Interactive mode SHOULD require confirmation for these
high-risk operations. Non-interactive mode MUST provide an explicit opt-in
policy before running mutating file tools or shell commands. Full reusable
permission profiles remain Phase 3 scope.

CLI target:

```text
opi [OPTIONS] [PROMPT]

Options:
  -m, --model <SPEC>       Model, e.g. anthropic:claude-sonnet-4
  -c, --config <PATH>      Config file path
  -s, --system <PATH>      System prompt file
      --list-models        List available models
      --non-interactive    Single prompt mode
  -v, --verbose            Enable debug tracing
  -V, --version            Print version
  -h, --help               Print help
```

Phase 2 adds `--resume`, `--list-sessions`, and `--json` after session storage
and JSON event schemas have contract tests.

Prompt layers:

1. base coding-agent instructions;
2. tool descriptions from `ToolDef`;
3. user system prompt file;
4. `OPI.md` project context, starting Phase 3;
5. compaction summaries, starting Phase 2;
6. skills/prompt fragments, starting Phase 4.

### 8.5 `opi-web-ui`

`opi-web-ui` is unpublished and deferred. When implemented, it should consume the same event schemas as JSON/RPC modes rather than depending on TUI internals.

## 9. Configuration and Storage

### 9.1 Config

```toml
[defaults]
model = "anthropic:claude-sonnet-4"
max_iterations = 50
tool_timeout_ms = 30000
theme = "default"

[thinking]
enabled = true
budget_tokens = 10000

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

[compaction]
enabled = true
reserve_tokens = 8000
keep_recent_tokens = 4000

[keybindings]
submit = "enter"
abort = "ctrl+c"
new_line = "shift+enter"
```

Malformed config files SHOULD fail clearly. Silent fallback is allowed for missing optional files, not invalid user config.

Phase 1 config loading only needs defaults, provider credentials, model
selection, timeouts, theme selection, and high-risk tool policy. Compaction,
session, and advanced keybinding settings MAY be accepted as reserved fields,
but they must not imply those Phase 2 features are active.

### 9.2 Directory Layout

```text
~/.config/opi/config.toml
~/.config/opi/themes/
~/.local/share/opi/sessions/
~/.local/share/opi/auth/
```

Windows SHOULD use `%APPDATA%\opi\` for config-like data and `%LOCALAPPDATA%\opi\` for cache-like data.

### 9.3 Session Format

Session persistence starts in Phase 2, not Phase 1. The target format is
append-only, versioned JSONL. The first line is a header:

```json
{"type":"session","version":1,"id":"018f...","timestamp":"2026-05-20T12:00:00Z","cwd":"/repo","parent_session":null}
```

Subsequent lines are tree entries:

```json
{"type":"message","id":"a1b2c3d4","parent_id":null,"timestamp":"2026-05-20T12:00:01Z","message":{"role":"user","content":[{"type":"text","text":"Read src/main.rs"}]}}
{"type":"message","id":"b2c3d4e5","parent_id":"a1b2c3d4","timestamp":"2026-05-20T12:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"I'll inspect it."}],"stop_reason":"tool_use"}}
{"type":"compaction","id":"c3d4e5f6","parent_id":"b2c3d4e5","timestamp":"2026-05-20T13:00:00Z","summary":"The session inspected CLI scaffolding.","first_kept_entry_id":"b2c3d4e5","tokens_before":45000,"tokens_after":8000}
```

Entry types:

| Type | Purpose | LLM context |
|---|---|---|
| `message` | user, assistant, tool result, or custom message | yes |
| `model_change` | selected provider/model changed | no |
| `thinking_level_change` | thinking level changed | no |
| `compaction` | summary plus first kept entry | yes |
| `branch_summary` | parent branch summary | yes |
| `label` | user marker | no |
| `session_info` | name and metadata | no |
| `custom` | extension state | no |
| `custom_message` | extension-provided context | configurable |
| `leaf` | current branch pointer | no |

Crash recovery MAY ignore an incomplete final line. Corrupt middle entries SHOULD be reported; automatic skipping of middle entries should require explicit recovery mode.

### 9.4 Why Not pi Session v3

Opi keeps pi's branch and compaction ideas but not its file format because pi stores TypeScript-specific extension data, opi has independent config/plugin plans, and accidental partial compatibility would be misleading. A future migration command MAY translate pi v3 sessions into opi v1 sessions.

### 9.5 Compaction

Compaction starts in Phase 2 after session storage exists.

Triggers:

- manual;
- threshold-based;
- context overflow recovery.

Results MUST record summary, `first_kept_entry_id`, tokens before/after, reason, and whether the summary came from core or a hook/extension. If compaction fails during overflow recovery, the agent MUST surface a visible error rather than silently dropping history.

## 10. CLI and Runtime Surfaces

Interactive mode is the default when stdin is a TTY. It owns terminal initialization, rendering, input editing, cancellation keys, and permission prompts.

Non-interactive mode takes one prompt from argv or stdin, streams assistant text to stdout, writes diagnostics to stderr, and exits with explicit status.

Suggested exit codes:

| Code | Meaning |
|---:|---|
| 0 | success |
| 1 | general runtime failure |
| 2 | invalid CLI usage or config |
| 3 | authentication failure |
| 4 | provider/network failure after retries |
| 5 | unrecovered tool failure |
| 130 | interrupted by user |

JSON mode is Phase 2 scope. It emits one `AgentSessionEvent` JSON object per line to stdout after the event schema has contract tests. Human-readable logs go to stderr. Phase 2 JSON mode SHOULD stay close to pi's event model but MUST include an opi schema version.

RPC mode is Phase 4. It should use strict JSONL framing: one command per line on stdin, correlated responses by optional `id`, and async events on stdout.

## 11. Cross-Cutting Runtime Concerns

### 11.1 Error Handling

| Layer | Approach |
|---|---|
| `opi-ai` | typed `ProviderError` plus stream `Error` terminal events |
| `opi-agent` | typed `AgentError`, `ToolError`, `SessionError` |
| `opi-tui` | typed terminal/render errors |
| `opi-coding-agent` | top-level reporting and mapped exit codes |

Library crates MUST avoid `unwrap` and `expect` except in tests or provably safe static initialization.

### 11.2 Cancellation and Backpressure

- Cancellation uses `tokio_util::sync::CancellationToken` or an equivalent cooperative token.
- Ctrl+C in interactive mode aborts the active operation first; repeated Ctrl+C MAY exit.
- Provider streams should use bounded channels to propagate backpressure.
- Tool subprocesses MUST be killed or deliberately detached on cancellation.

### 11.3 Observability

`tracing` spans SHOULD cover provider calls, SSE parsing, agent turns, tool execution, session append/load, compaction, and retry scheduling. Secrets and raw provider payloads MUST be redacted by default.

### 11.4 Performance Targets

| Metric | Target | Verification |
|---|---:|---|
| startup to first prompt | less than 100 ms | CLI init benchmark without network |
| first token display overhead | provider delta plus less than 50 ms | mock streaming provider |
| TUI frame rate | 30 FPS target | terminal snapshot/perf fixture |
| idle memory | less than 50 MB | release smoke measurement |
| release binary size | less than 20 MB target | release artifact check |

## 12. Testing Strategy

| Level | Owner | Required coverage |
|---|---|---|
| unit | every crate | message conversion, schema validation, config parsing, path handling |
| provider contract | `opi-ai` | SSE fixtures, terminal events, error mapping |
| mock loop integration | `opi-agent` | canned provider events and mock tools |
| session round-trip | `opi-agent` | JSONL append/load, tree reconstruction, compaction |
| tool tests | `opi-coding-agent` | temp-dir file tools, command timeout/cancellation |
| CLI E2E | `opi-coding-agent` | `--help`, `--version`, non-interactive mock run, exit codes |
| TUI snapshot | `opi-tui` | deterministic render output at fixed sizes |
| JSON contract | `opi-coding-agent` | NDJSON schema and line framing |
| live provider | `opi-ai` | ignored tests gated by env vars |
| fuzz/property | selected crates | JSONL loader, provider parser, tool argument schemas |

Phase 1 MUST include a mock provider harness. Live provider tests are not sufficient because they are slow, paid, flaky, and credential-dependent.
Session round-trip, JSON contract, and session-loader fuzz/property tests become
required when the corresponding Phase 2 features are implemented.

Current CI gates:

- `cargo fmt --all --check`;
- `cargo clippy --workspace --all-targets`;
- `cargo test --workspace --all-targets`;
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.

## 13. Security and Risk

### 13.1 Threat Model

Opi runs local tools with the user's privileges. The main risks are dangerous local commands, secret leakage, sensitive session files, and credential mishandling.

### 13.2 Requirements

- API keys MUST NOT be logged or written to sessions.
- Sessions MUST be documented as sensitive.
- `bash` MUST have timeout, cwd control, environment policy, cancellation behavior, and visible command text.
- File tools MUST resolve paths deliberately and record whether paths are inside or outside the workspace.
- Path traversal MAY be allowed, but permission policy SHOULD be able to restrict it.
- Provider HTTP MUST use TLS by default.
- Phase 1 MUST include minimal confirmation and auditability for `write`, `edit`, and `bash`; richer reusable permission profiles are Phase 3 scope.
- Permission prompts MUST exist before high-risk tools are considered production ready.

Structured arguments reduce shell injection risk, but invoking a shell still executes model-supplied command text. The mitigation is permission, auditability, timeout, cwd/env control, and careful command construction.

### 13.3 Risk Register

| Risk | Impact | Likelihood | Mitigation |
|---|---|---:|---|
| Provider API drift | high | medium | fixture tests and narrow adapters |
| Anthropic-only MVP disappoints parity expectations | medium | medium | publish clear phase scope |
| Session schema stabilizes too early | high | medium | keep v1 unstable until contract tests pass |
| Bash tool performs destructive actions | high | high | sequential mode, permissions, visible command, timeout |
| Secrets leak to logs/session | high | medium | redaction tests and secret types |
| Windows TUI issues | medium | medium | crossterm tests and Windows smoke checks |
| Premature crates.io publish | high | medium | gate first publish on real implementation, docs, and contract tests; defer crates.io if those gates miss 0.2.0 |
| Extension scope bloats core | medium | high | minimal-core rule |
| Duplicate session stacks | high | medium | explicit Harness vs CodingHarness ownership |

## 14. Release and Versioning

All crates share one workspace version.

| Version | Milestone | Publish |
|---|---|---|
| 0.1.0 | scaffolding | GitHub Release only |
| 0.2.0 | Phase 1 MVP | GitHub Release; crates.io only if publish gates pass except `opi-web-ui` |
| 0.3.0 | Phase 2 persistence and providers | GitHub + crates.io |
| 0.4.0+ | Phase 3 hardening | GitHub + crates.io |

The first crates.io publish is gated by quality, not by the version number alone.
It MAY happen at 0.2.0 if all published crates expose real, documented behavior
rather than placeholder public APIs, public docs build cleanly, contract tests
cover the shipped provider/tool/runtime boundaries, and the release skill's
checks pass. If those gates are not met, crates.io publishing SHOULD move to a
later 0.2.x or 0.3.0 release while GitHub binary releases continue. Because the
binary crate depends on internal library crates, those libraries should publish
together in dependency order; `opi-web-ui` remains unpublished until it has a
concrete implementation. All 0.x public APIs are unstable unless explicitly
documented otherwise.

The release process SHOULD follow `.claude/skills/opi-release/skill.md`: pre-flight, version bump, changelog, checks, tag/draft release, crates.io publish, finalize. crates.io publishing is irreversible except yanking; rollback should use new commits and tag management, not force-pushed public history.

Release CI builds:

- `opi-linux-x64.tar.gz`;
- `opi-linux-arm64.tar.gz`;
- `opi-darwin-x64.tar.gz`;
- `opi-darwin-arm64.tar.gz`;
- `opi-windows-x64.zip`;
- `opi-windows-arm64.zip`.

`SHA256SUMS.txt` SHOULD be uploaded with release artifacts. Windows ARM64 is a
Tier 2 target and should be treated as non-blocking for Phase 1 MVP releases if
the target-specific build flakes while Tier 1 targets pass.

## 15. Implementation Roadmap

### Phase 0 - Scaffolding Baseline

Status: complete in 0.1.0.

| Task | Status |
|---|---|
| five-crate workspace | done |
| lockstep versioning | done |
| placeholder modules and re-exports | done |
| CI gates | done |
| six-platform release workflow | done |
| `opi --version` and `--help` | done |
| GitHub Release only, crates.io deferred | done |

### Phase 1 - MVP Foundation

Target: 0.2.0.

Goal: Anthropic-only coding agent with core loop, six tools, minimal safety
boundaries for mutating tools and shell execution, basic TUI, TOML config, and
mock-provider tests.

| # | Task | Crate | Definition of done |
|---|---|---|---|
| 1.0 | introduce Phase 1 dependencies | workspace | manifests include needed deps with minimal features and without unused-dep warnings |
| 1.1 | message and stream types | `opi-ai` | serialize where needed; terminal stream events tested |
| 1.2 | replace placeholder provider trait | `opi-ai` | `stream(Request)` replaces `complete` |
| 1.3 | Anthropic SSE provider | `opi-ai` | fixtures cover text, tool call, usage, error |
| 1.4 | provider registry | `opi-ai` | resolves `anthropic:model` and capabilities |
| 1.5 | tool trait and schema validation | `opi-agent` | invalid args become error tool result |
| 1.6 | `agent_loop` | `opi-agent` | mock tests cover no-tool and tool-use turns |
| 1.7 | `Agent` wrapper | `opi-agent` | prompt, continue, abort, subscribe tested |
| 1.8 | hooks and queues | `opi-agent` | before/after, should-stop, steering, follow-up tested |
| 1.9 | `read`, `write`, `edit`, `bash` | `opi-coding-agent` | temp-dir tests cover success, failure, timeout/cancellation, cwd/env reporting, and minimal confirmation policy |
| 1.10 | `glob`, `grep` | `opi-coding-agent` | tests cover ignored dirs and regex errors |
| 1.11 | system prompt construction | `opi-coding-agent` | prompt includes tool defs and system layer |
| 1.12 | TUI shell | `opi-tui` | fixed-size render snapshots |
| 1.13 | markdown/code rendering | `opi-tui` | markdown and fenced code snapshots |
| 1.14 | interactive CLI wiring | `opi-coding-agent` | runs against mock provider |
| 1.15 | non-interactive mode | `opi-coding-agent` | stdout/stderr/exit-code tests |
| 1.16 | TOML config loading | `opi-coding-agent` | missing defaults and malformed errors tested |
| 1.17 | integration harness | cross-crate | mock-provider E2E runs in CI |

Exit criteria: `opi` accepts a prompt, streams Claude output, executes
`read/write/edit/bash/glob/grep` behind the Phase 1 safety boundary, displays
results in TUI, supports non-interactive mode with explicit high-risk tool
policy, and passes mock-provider CI tests. Sessions, compaction, JSON mode, MCP,
plugins, web UI, rich diff views, and syntax-highlighted code blocks are not
Phase 1 exit criteria.

### Phase 2 - Multi-Provider and Persistence

Target: 0.3.0.

| # | Task | Crate |
|---|---|---|
| 2.1 | OpenAI-compatible chat provider | `opi-ai` |
| 2.2 | OpenRouter provider profile | `opi-ai` |
| 2.3 | OpenAI Responses provider | `opi-ai` |
| 2.4 | Google Gemini provider | `opi-ai` |
| 2.5 | Mistral provider | `opi-ai` |
| 2.6 | opi session v1 JSONL storage and contract tests | `opi-agent` |
| 2.7 | session list/resume/delete | `opi-coding-agent` |
| 2.8 | compaction | `opi-agent` / `opi-coding-agent` |
| 2.9 | thinking/reasoning support | `opi-ai` |
| 2.10 | usage and cost tracking | `opi-ai` |
| 2.11 | diff view | `opi-tui` |
| 2.12 | themes | `opi-tui` |
| 2.13 | keybindings | `opi-tui` |
| 2.14 | `--json` NDJSON mode | `opi-coding-agent` |
| 2.15 | retry/backoff/rate limits | `opi-ai` |
| 2.16 | session contract tests | `opi-agent` |

Exit criteria: sessions survive restart, multiple providers pass contract fixtures, long conversations compact before overflow, and JSON mode has schema tests.

### Phase 3 - Production Hardening

Target: 0.4.0+.

| # | Task | Crate |
|---|---|---|
| 3.1 | AWS Bedrock provider | `opi-ai` |
| 3.2 | Azure OpenAI provider | `opi-ai` |
| 3.3 | Google Vertex provider | `opi-ai` |
| 3.4 | image input | `opi-ai` |
| 3.5 | image tool results | `opi-agent` |
| 3.6 | terminal image rendering | `opi-tui` |
| 3.7 | `OPI.md` context loading | `opi-coding-agent` |
| 3.8 | reusable permission profiles and policy system | `opi-coding-agent` |
| 3.9 | MCP client adapter | `opi-agent` |
| 3.10 | shell completions | `opi-coding-agent` |
| 3.11 | fuzzy model/session picker | `opi-tui` |
| 3.12 | proxy support | `opi-ai` |
| 3.13 | connection pooling tuning | `opi-ai` |

Cross-platform binary releases are not listed here because release CI is already part of Phase 0.

Exit criteria: enterprise providers work, risky tools have permissions, release artifacts are repeatable, and interactive UX is robust for daily use.

### Phase 4 - Extensibility

| # | Task | Crate |
|---|---|---|
| 4.1 | extension trait design | `opi-agent` |
| 4.2 | extension loading strategy | `opi-coding-agent` |
| 4.3 | session branching UI | `opi-agent` / `opi-tui` |
| 4.4 | multi-agent orchestration | `opi-agent` |
| 4.5 | skills and prompt fragments | `opi-coding-agent` |
| 4.6 | web UI implementation | `opi-web-ui` |
| 4.7 | RPC JSONL mode | `opi-coding-agent` |
| 4.8 | WASM or subprocess plugin runtime | `opi-agent` or a new plugin crate after interface review |
| 4.9 | streaming proxy | `opi-agent` or new crate |

Exit criteria: third parties can extend opi without patching core crates.

## 16. Decision Log

| # | Decision | Choice | Reason |
|---|---|---|---|
| ADR-001 | Workspace shape | five crates mirroring pi packages | preserves conceptual boundaries |
| ADR-002 | Versioning | lockstep workspace version | simplifies compatibility and release order |
| ADR-003 | No shared types crate | types live with semantic owner | avoids hub dependency |
| ADR-004 | pi compatibility | semantic parity, not API/file parity | Rust-native implementation |
| ADR-005 | MVP provider | Anthropic only | first release remains testable |
| ADR-006 | Provider SDKs | direct HTTP adapters | streaming control and fewer unstable deps |
| ADR-007 | Stream protocol | start/delta/end/done/error | aligns with pi and UI partial state |
| ADR-008 | Agent layering | loop -> Agent -> Harness | testability and separation |
| ADR-009 | Agent vs LLM messages | keep separate | custom messages should not leak to providers |
| ADR-010 | Tool boundary | typed args plus generated JSON Schema | dynamic LLM boundary, typed internals, runtime validation |
| ADR-011 | Tool execution | parallel default with sequential override | matches pi and avoids races |
| ADR-012 | Session format | opi tree JSONL | branch semantics without TS format lock-in |
| ADR-013 | Config format | TOML | comments and Rust ecosystem fit |
| ADR-014 | TUI | ratatui/crossterm | cross-platform Rust terminal stack |
| ADR-015 | Plugin strategy | MCP adapter before dynamic plugins | protocol integration is lower risk than arbitrary runtime extension |
| ADR-016 | Web UI | unpublished until core stable | avoids premature WASM commitment |
| ADR-017 | Transport stub | reserve for Phase 4 or remove | avoids undocumented public surface |
| ADR-018 | crates.io timing | quality-gated first publish | publish only after placeholder APIs are hidden or replaced and release gates pass |

## 17. Non-Functional Requirements

Tier 1 targets:

- `x86_64-unknown-linux-gnu`;
- `aarch64-unknown-linux-gnu`;
- `x86_64-apple-darwin`;
- `aarch64-apple-darwin`;
- `x86_64-pc-windows-msvc`.

Tier 2 target: `aarch64-pc-windows-msvc`.

Rustls is preferred over OpenSSL for portable binary builds.

Accessibility requirements:

- respect `NO_COLOR`;
- expose essential state in non-interactive and JSON modes;
- do not rely only on color for errors, tools, or diffs;
- provide exit codes suitable for scripts.

Maintainability requirements:

- document public APIs with examples once implemented;
- include tests before phase tasks are marked done;
- track spec/code drift in changelog or issues when unavoidable;
- split large modules by responsibility.

## 18. Future Considerations

The architecture should not preclude MCP tools, remote tool execution, streaming proxy services, editor integrations, pi session migration, plugin runtimes, or web chat surfaces. These are not MVP requirements.

## 19. Glossary

| Term | Definition |
|---|---|
| Provider | LLM backend such as Anthropic, OpenAI, Gemini, or Bedrock |
| API kind | wire protocol family, such as Anthropic Messages or OpenAI Chat Completions |
| Model | provider model with capabilities and limits |
| Agent loop | pure loop that sends context, receives assistant output, executes tools, and repeats |
| Agent | stateful wrapper around the loop |
| Harness | composition layer for sessions, compaction, and app hooks |
| CodingHarness | coding-agent-specific harness |
| AgentMessage | app-level message that may include custom/session-only data |
| Message | provider-facing user/assistant/tool-result message |
| Stream event | provider-level assistant delta or terminal event |
| Agent event | runtime lifecycle/message/tool event |
| Session event | queue/compaction/retry/session event |
| Session entry | persisted JSONL tree record |
| Steering | message injected while agent is running before next provider call |
| Follow-up | message queued until agent would otherwise stop |
| Compaction | summarizing older context while preserving recent state |
| Tool | model-callable capability with JSON Schema parameters |

## 20. References

- [pi source](https://github.com/earendil-works/pi)
- `.repo/pi-0.75.3/CONTRIBUTING.md`
- `.repo/pi-0.75.3/packages/agent/README.md`
- `.repo/pi-0.75.3/packages/agent/src/types.ts`
- `.repo/pi-0.75.3/packages/ai/src/types.ts`
- `.repo/pi-0.75.3/packages/coding-agent/src/core/session-manager.ts`
- `.repo/pi-0.75.3/packages/coding-agent/docs/json.md`
- `.repo/pi-0.75.3/packages/coding-agent/docs/rpc.md`
- `Cargo.toml`
- `CHANGELOG.md`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.claude/skills/opi-release/skill.md`
- [Anthropic Messages API](https://docs.anthropic.com/en/api/messages)
- [ratatui](https://ratatui.rs/)
- [MCP specification](https://modelcontextprotocol.io/)
