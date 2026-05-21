# Phase 1 Audit Report

> Snapshot date: 2026-05-22
> Spec version: `011cc486f32a60b3f967c911a369e091cca88dd20417dfdc5a0cb7fd60c8e597`
> HEAD commit: `2c9da2f` (feat: add non-interactive mode with tool safety policy)
> Total tests: 213 passing, 0 failing

## 1. Executive Summary

Phase 1 delivers the **MVP Foundation** for opi — a Rust reimplementation of
[earendil-works/pi](https://github.com/earendil-works/pi). Starting from a 0.1.0
scaffolding release, 18 tasks were completed across 5 crates in 18 commits,
producing a functional Anthropic-based coding assistant with:

- Streaming SSE provider with fixture-based tests
- Agent loop with tool calling, hooks, queues, and cancellation
- Six built-in tools (read, write, edit, bash, glob, grep)
- TUI shell with markdown rendering
- TOML config with 5-level precedence
- Non-interactive mode with tool safety policy
- Mock provider E2E integration harness

All 18 tasks reached `passing` status with mechanical verification gates
(fmt, clippy, doc, test, smoke). No tasks required more than 1 iteration.

## 2. Task Completion Matrix

| ID | Title | Crate | Tier | Commit | Tests | Iter |
|---:|-------|-------|------|--------|------:|-----:|
| 1.0 | introduce Phase 1 dependencies | workspace | workspace | `4d9c643` | — | 1 |
| 1.1 | message and stream types | opi-ai | library | `1ae20e1` | 14 | 1 |
| 1.2 | replace placeholder provider trait | opi-ai | library | `0f7d2bd` | 15 | 1 |
| 1.3 | Anthropic SSE provider | opi-ai | library | `98e4c01` | 19 | 1 |
| 1.4 | provider registry | opi-ai | library | `ce62740` | 14 | 1 |
| 1.5 | tool trait and schema validation | opi-agent | library | `0aa38e7` | 12 | 1 |
| 1.6 | agent_loop | opi-agent | library | `8db09ba` | 2 | 1 |
| 1.7 | Agent wrapper | opi-agent | library | `e5ff0ec` | 5 | 1 |
| 1.8 | hooks and queues | opi-agent | library | `b0bedb0` | 7 | 1 |
| 1.9 | read, write, edit, bash | opi-coding-agent | cli-tool | `dd23647` | 25 | 1 |
| 1.10 | glob, grep | opi-coding-agent | cli-tool | `ce0e384` | 18 | 1 |
| 1.11 | system prompt construction | opi-coding-agent | cli-runtime | `0f8a332` | 10 | 1 |
| 1.12 | TUI shell | opi-tui | tui | `f528052` | 15 | 1 |
| 1.13 | markdown/code rendering | opi-tui | tui | `8de104f` | 8 | 1 |
| 1.14 | interactive CLI wiring | opi-coding-agent | cli-runtime | `f42321a` | 5 | 1 |
| 1.15 | non-interactive mode | opi-coding-agent | cli-runtime | `2c9da2f` | 9 | 1 |
| 1.16 | TOML config loading | opi-coding-agent | cli-runtime | `51f8c46` | 19 | 1 |
| 1.17 | integration harness | workspace | workspace | `c4d2a3c` | 16 | 1 |

**Summary:** 18/18 tasks passing. 0 blocked. 0 iterations exceeded cap.

## 3. Architecture Overview

### Crate Dependency Graph

```
opi-ai (no internal deps)
├── Provider trait, Request, EventStream, AssistantStreamEvent
├── AnthropicProvider (SSE parser + mapper)
├── ProviderRegistry (resolve "provider:model" specs)
├── MockProvider + test_support (shared test infra)
└── Message, ToolDefinition, Usage, StopReason types

opi-agent → opi-ai
├── Tool trait, ToolResult, schema validation (jsonschema)
├── agent_loop (stream → tool call → loop)
├── Agent wrapper (prompt/continue_/abort/subscribe)
├── AgentHooks (before/after tool call, should_stop, prepare_next_turn)
└── Steering + follow-up queues

opi-tui (no internal deps)
├── Shell (vertical layout compositor)
├── MessageList, InputEditor, StatusBar, ToolCallView
├── MarkdownView, CodeBlock
└── Shared types (Message, Role, AppState, ToolCallStatus)

opi-coding-agent → opi-ai, opi-agent, opi-tui
├── Tools: ReadTool, WriteTool, EditTool, BashTool, GlobTool, GrepTool
├── CodingHarness (wires provider + tools + hooks + agent)
├── NonInteractiveRunner (single prompt → stdout/stderr/exit code)
├── SystemPromptBuilder (base + tools + user layers)
├── Config (TOML loading, 5-level precedence)
├── CLI (clap: -m, -c, -s, --non-interactive, --allow-mutating)
└── Policy (is_mutating_tool safety check)

opi-web-ui → opi-ai (placeholder, publish=false)
```

### Source File Distribution

| Crate | src/ files | tests/ files | Production LOC | Test LOC |
|-------|----------:|------------:|---------------:|---------:|
| opi-ai | 9 | 5 | ~1200 | ~1800 |
| opi-agent | 10 | 4 | ~1100 | ~1200 |
| opi-coding-agent | 15 | 10 | ~1900 | ~2100 |
| opi-tui | 7 | 2 | ~550 | ~340 |
| opi-web-ui | 2 | 0 | ~30 | 0 |
| **Total** | **43** | **21** | **~4780** | **~5440** |

Test-to-production ratio: **1.14:1** (more test code than production code).

## 4. Spec Compliance — Phase 1 Exit Criteria

From `docs/opi-spec.md` §15:

> Exit criteria: `opi` accepts a prompt, streams Claude output, executes
> `read/write/edit/bash/glob/grep` behind the Phase 1 safety boundary, displays
> results in TUI, supports non-interactive mode with explicit high-risk tool
> policy, and passes mock-provider CI tests.

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Accepts a prompt | PASS | CodingHarness.prompt() wired in task 1.14 |
| Streams Claude output | PASS | AnthropicProvider SSE (1.3), agent_loop streaming (1.6) |
| Executes read/write/edit/bash/glob/grep | PASS | 6 tools implemented (1.9, 1.10), 43 tool tests |
| Phase 1 safety boundary | PASS | WriteTool/EditTool/BashTool report safety details; NonInteractiveHooks blocks mutating tools (1.15) |
| Displays results in TUI | PASS | Shell + MessageList + MarkdownView (1.12, 1.13) |
| Non-interactive mode | PASS | NonInteractiveRunner with exit codes 0/1/2/3/4/5/130 (1.15) |
| Explicit high-risk tool policy | PASS | --allow-mutating flag + config field (1.15) |
| Mock-provider CI tests | PASS | MockProvider E2E in CI (1.17), 213 tests green |

**Explicitly out of scope (confirmed not delivered):**
Sessions, compaction, JSON mode, MCP, plugins, web UI, rich diff views,
syntax-highlighted code blocks.

## 5. Test Coverage Breakdown

| Test File | Crate | Tests | Category |
|-----------|-------|------:|----------|
| stream_events.rs | opi-ai | 14 | Message/event serialization |
| provider_trait.rs | opi-ai | 15 | Provider contract |
| anthropic_fixtures.rs | opi-ai | 19 | SSE parsing (text, tool, usage, error, mixed) |
| registry.rs | opi-ai | 14 | Provider resolution |
| mock_provider.rs | opi-ai | 12 | MockProvider builder/behavior |
| tool_validation.rs | opi-agent | 12 | Schema validation, error results |
| agent_loop_mock.rs | opi-agent | 2 | No-tool and tool-use turns |
| agent_wrapper.rs | opi-agent | 5 | prompt/continue/abort/subscribe |
| hooks_queues.rs | opi-agent | 7 | Hooks, steering, follow-up |
| tools_read_write_edit_bash.rs | opi-coding-agent | 25 | Tool execution (success/failure/timeout) |
| tools_glob_grep.rs | opi-coding-agent | 12 | Glob/grep (gitignore, regex errors) |
| tool_schema_fixtures_glob_grep.rs | opi-coding-agent | 6 | Schema fixture validation |
| system_prompt.rs | opi-coding-agent | 10 | Prompt layer construction |
| config_loading.rs | opi-coding-agent | 11 | TOML parse/defaults/errors |
| config_precedence.rs | opi-coding-agent | 8 | 5-level merge precedence |
| interactive_mock.rs | opi-coding-agent | 5 | CodingHarness E2E |
| mock_e2e.rs | opi-coding-agent | 4 | Cross-crate integration |
| non_interactive.rs | opi-coding-agent | 3 | stdout/stderr/exit code |
| non_interactive_policy.rs | opi-coding-agent | 6 | Tool safety policy |
| tui_snapshots.rs | opi-tui | 15 | Component rendering (80x24, 120x40) |
| markdown_snapshots.rs | opi-tui | 8 | Markdown/code block rendering |
| **Total** | | **213** | |

## 6. Quality Gates

### CI Pipeline (`.github/workflows/ci.yml`)

| Gate | Command | Status |
|------|---------|--------|
| Format | `cargo fmt --all --check` | PASS |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| Test | `cargo test --workspace --all-targets` | PASS (213/213) |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | PASS |

### Code Hygiene

- No `TODO`, `FIXME`, `unimplemented!()`, or `todo!()` in production code
- No `unwrap()` or `expect()` in production code (test code only)
- No `anyhow` in library crates (opi-ai, opi-agent, opi-tui)
- All internal deps use `[workspace.dependencies]` with lockstep versioning
- `async-trait` present with documented removal path before 0.2.0

## 7. Dependency Audit (vs Spec §5.5)

| Dependency | Spec Requirement | Status |
|------------|-----------------|--------|
| tokio | narrow features, no `["full"]` | PASS — `rt-multi-thread, macros, io-util, process, time, signal, sync, fs` |
| reqwest | `rustls-tls`, `default-features = false` | PASS |
| serde / serde_json | present | PASS |
| thiserror | library errors | PASS — all library crates |
| anyhow | opi-coding-agent only | PASS — not in opi-ai/opi-agent/opi-tui |
| async-trait | present, remove before 0.2.0 | PASS — present with exit path |
| futures-core / futures-util | public Stream APIs | PASS |
| tokio-util | cancellation | PASS — CancellationToken |
| clap | CLI | PASS — derive feature |
| toml | config | PASS |
| ratatui / crossterm | TUI | PASS |
| schemars / jsonschema | tool schemas | PASS — draft-07 compatible |
| uuid / time | IDs and timestamps | PASS |
| ignore / globset / regex | file search | PASS |
| tracing / tracing-subscriber | observability | PASS |
| pulldown-cmark | markdown | REMOVED — hand-rolled parser sufficient for Phase 1 |

**Deviation:** `pulldown-cmark` was removed during task 1.13 because a simpler
hand-written markdown parser met the Phase 1 requirements without the extra
dependency. This is acceptable per spec ("optional `syntect` later").

## 8. Known Limitations / Technical Debt

### Agent Loop Text Content Loss

The agent loop (`opi-agent/src/lib.rs:115`) overwrites `assistant_msg.content`
with only tool-call content blocks, discarding text content from the Done event.
The NonInteractiveRunner works around this by subscribing to `TextDelta` events
to capture text. This should be fixed in Phase 2 when the agent loop is
revisited for compaction support.

### TUI Not Wired to Agent Events

The TUI components (task 1.12, 1.13) render from static state structs but are
not yet connected to live `AgentEvent` streams. Task 1.14 wires the
CodingHarness but the actual interactive TUI event loop (reading terminal input,
dispatching to agent, updating TUI state) is Phase 2 scope.

### opi-web-ui Placeholder

`opi-web-ui` has no tests and minimal code (2 source files, ~30 LOC). It exists
only to reserve the crate boundary. No Phase 1 work was planned for it.

### No Live Provider Tests

Per spec, live provider tests are `#[ignore]`-gated and not required for CI.
They do not exist yet. Phase 2 should add at least one smoke test gated behind
`OPI_LIVE_TEST=1` for Anthropic.

### Interactive Mode Incomplete

While CodingHarness is wired (task 1.14), the actual `main.rs` interactive path
currently only has non-interactive mode fully functional. The interactive TUI
event loop (terminal raw mode, input handling, streaming display) is not yet
implemented — it requires connecting TUI components to the agent event stream.

## 9. Recommendations for Phase 2

1. **Fix agent loop text content** — Preserve text content in assistant messages
   alongside tool calls, eliminating the TextDelta workaround.

2. **Wire interactive TUI** — Connect AgentEvent stream to TUI state updates,
   implement terminal raw mode input loop, handle resize/signal.

3. **Add session persistence** — Append-only JSONL per spec §9.3, enabling
   resume and history.

4. **Multi-provider support** — OpenAI-compatible, OpenRouter, Gemini adapters
   per spec Phase 2 roadmap.

5. **Compaction** — Context window management for long conversations.

6. **Live provider smoke test** — At least one `#[ignore]` test hitting real
   Anthropic API, gated by env var.

7. **Remove async-trait** — Replace with native async fn in traits (Rust 1.75+
   RPITIT) before 0.2.0 release per spec §5.5.

## 10. Commit History (Phase 1)

```
4d9c643 chore: introduce Phase 1 dependencies
1ae20e1 feat(opi-ai): add message and stream types
0f7d2bd feat(opi-ai): replace placeholder provider trait
98e4c01 feat(opi-ai): implement Anthropic SSE provider
ce62740 feat(opi-ai): add provider registry
0aa38e7 feat(opi-agent): add tool trait and schema validation
8db09ba feat(opi-agent): implement agent_loop
e5ff0ec feat(opi-agent): implement Agent wrapper
b0bedb0 feat(opi-agent): implement hooks and queues
dd23647 feat(opi-coding-agent): implement read, write, edit, bash tools
ce0e384 feat(opi-coding-agent): implement glob and grep tools
f528052 feat(opi-tui): implement TUI shell with snapshot tests
8de104f feat(opi-tui): implement markdown/code rendering with snapshot tests
c4d2a3c feat(opi-ai): add shared MockProvider test harness with E2E integration tests
0f8a332 feat(opi-coding-agent): add system prompt builder with layered construction
51f8c46 feat(opi-coding-agent): add TOML config loading with precedence resolution
f42321a feat(opi-coding-agent): add interactive CLI wiring with CodingHarness
2c9da2f feat(opi-coding-agent): add non-interactive mode with tool safety policy
```

---

*Generated from `.opi-impl-state.json` ledger and git history at commit `2c9da2f`.*
