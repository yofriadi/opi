# Phase 2 Hardening Pass Design

**Date**: 2026-05-23
**Status**: Draft
**Trigger**: Three independent audits (GLM-5.1, GPT-5.5, Codex) identified that Phase 2 modules are library/component-ready but not runtime-complete.
**Goal**: Close all Critical and High findings so Phase 2 satisfies "multi-provider + session persistence + compaction + JSON mode all user-path reachable."

---

## 1. Problem Statement

Phase 2 shipped 16 tasks with 537 tests, all passing. However, the audits agree on a systemic pattern: modules are implemented and tested at the library/fixture/component layer, but not wired into the `opi` binary's runtime paths. Specific gaps:

1. **Two providers have HTTP stubs** -- OpenAI Responses and Gemini `Provider::stream()` returns `"HTTP streaming not implemented"`.
2. **Provider factory only handles Anthropic** -- `build_provider()` rejects `openai:*`, `openrouter:*`, `gemini:*`, `mistral:*`, `openai-responses:*`.
3. **Session writer is never called** -- `CodingHarness` does not create or append to JSONL sessions.
4. **`--resume` only prints metadata** -- does not rebuild agent context or continue the conversation.
5. **Compaction engine is disconnected** -- no runtime trigger, no session entry, no prompt injection.
6. **Thinking config is parsed but unused** -- `AgentLoopConfig` has no thinking field; agent loop drops `ThinkingStart/Delta/End`.
7. **Usage/cost not accumulated or displayed** -- no runtime accumulation from provider responses; StatusBar token count is always `None`.
8. **DiffView not wired to edit/patch** -- widget exists in `opi-tui` but is never referenced from `opi-coding-agent`.
9. **Test coverage gaps** -- no wiremock lifecycle tests for non-Anthropic providers; no `--json` subprocess E2E.

---

## 2. Architecture: SessionCoordinator

A new `SessionCoordinator` struct in `opi-coding-agent` owns the session lifecycle, bridging between `CodingHarness`, `SessionWriter`, `CompactionEngine`, and usage accumulation.

```
CodingHarness
  +-- Agent (existing, from opi-agent)
  +-- SessionCoordinator (NEW)
  |     +-- SessionWriter (existing, from opi-agent::session)
  |     +-- CompactionEngine (existing, from opi-agent::compaction)
  |     +-- UsageAccumulator (NEW)
  |     +-- on_turn_end(messages, usage) -> Option<CompactionOutput>
  |     +-- on_compaction(output) -> ()
  |     +-- flush() -> ()
  +-- config -> AgentLoopConfig (extended with thinking field)
```

The coordinator hooks into existing `AgentHooks` callbacks:
- `after_tool_call` and `should_stop_after_turn` are augmented to call `coordinator.on_turn_end()`.
- Compaction triggers are checked after each turn.
- On cancellation/shutdown, `coordinator.flush()` writes pending entries.

### 2.1 SessionCoordinator API

```rust
pub struct SessionCoordinator {
    writer: SessionWriter,
    compaction: CompactionEngine,
    usage: UsageAccumulator,
    session_id: String,
}

impl SessionCoordinator {
    pub fn new(writer: SessionWriter, compaction_config: CompactionConfig) -> Self;

    /// Called after each agent turn. Appends message entries to JSONL.
    /// Returns Some(CompactionOutput) if compaction threshold is met.
    pub fn on_turn_end(&mut self, messages: &[AgentMessage], usage: &Usage) -> Option<CompactionOutput>;

    /// Called after compaction completes. Writes CompactionEntry to JSONL.
    pub fn on_compaction(&mut self, output: &CompactionOutput) -> ();

    /// Flush pending entries (called on shutdown/cancellation).
    pub fn flush(&mut self) -> std::io::Result<()>;

    /// Get current cumulative usage.
    pub fn usage(&self) -> &CumulativeUsage;
}
```

### 2.2 UsageAccumulator

```rust
pub struct UsageAccumulator {
    cumulative: CumulativeUsage,
}

impl UsageAccumulator {
    pub fn accumulate(&mut self, usage: &Usage);
    pub fn total_input_tokens(&self) -> u64;
    pub fn total_output_tokens(&self) -> u64;
    pub fn cumulative(&self) -> &CumulativeUsage;
}
```

---

## 3. Workstreams

### WS1: Provider HTTP Streaming

**Scope**: Implement real HTTP SSE streaming for `OpenAiResponsesProvider` and `GeminiProvider`.

**OpenAI Responses** (`crates/opi-ai/src/openai_responses.rs`):
- Add `stream_http()` async method following the `openai_chat::stream_http()` pattern.
- Endpoint: `POST {base_url}/v1/responses`
- Auth: `Authorization: Bearer {api_key}`
- SSE parsing: reuse existing `parse_sse_frames()` + `ResponsesMapper`
- HTTP error mapping: reuse or extract `map_http_status()` from `openai_chat`
- Add `ReceiverStream` wrapper (same pattern as `openai_chat`)
- Add `CancellationToken` support for cancellation
- Validate `saw_done` at stream end, emit error if missing

**Gemini** (`crates/opi-ai/src/gemini.rs`):
- Add `stream_http()` async method.
- Endpoint: `POST {base_url}/v1beta/models/{model_id}:streamGenerateContent?alt=sse`
- Auth: `x-goog-api-key: {api_key}` (Gemini-specific header, not Bearer)
- SSE parsing: reuse existing `parse_sse_data()` + `GeminiMapper`
- Gemini-specific HTTP error mapping (extract `error` field from response JSON)
- Add `ReceiverStream` wrapper
- Add `CancellationToken` support
- Validate `saw_done` at stream end

**Shared infrastructure** (optional refactoring):
- Consider extracting `ReceiverStream`, `drain_sse_events()`, and `map_http_status()` into a shared `streaming_utils` module in `opi-ai/src/`. This is optional -- only do it if the duplication is bothersome. Three copies of the same 50-line struct is acceptable per project norms.

**Tests**:
- Wiremock lifecycle tests for both providers covering: successful stream, HTTP 401/403/429/500, cancellation mid-stream, no-terminal-event error, tool calls in stream.
- Place in `crates/opi-ai/tests/` alongside existing fixture files.

**Files modified**:
- `crates/opi-ai/src/openai_responses.rs` -- add `stream_http()`, update `stream()`
- `crates/opi-ai/src/gemini.rs` -- add `stream_http()`, update `stream()`
- `crates/opi-ai/tests/openai_responses_lifecycle.rs` -- NEW
- `crates/opi-ai/tests/gemini_lifecycle.rs` -- NEW

---

### WS2: Provider Factory Extension

**Scope**: Extend `build_provider()` and `ProvidersConfig` to support all 6 providers.

**Provider matching** in `crates/opi-coding-agent/src/main.rs`:
```
"anthropic"       -> AnthropicProvider (existing)
"openai"          -> OpenAiChatProvider::new(api_key, base_url, "openai", vec![])
"openrouter"      -> OpenAiChatProvider::new_with_compat(openrouter_compat, extra_headers)
"mistral"         -> OpenAiChatProvider::new_with_compat(mistral_compat, vec![])
"openai-responses" -> OpenAiResponsesProvider::new(api_key, base_url)
"gemini"          -> GeminiProvider::new(api_key, base_url)
```

**Config schema** in `crates/opi-coding-agent/src/config.rs`:
- Add generic `[providers.<id>]` map with `api_key_env` and `base_url` fields.
- Keep `providers.anthropic` as a typed section for backward compat.
- API keys are resolved from environment variables only, never stored in config.

**OpenRouter-specific**:
- Extra headers: `HTTP-Referer` (from config or env `OPENROUTER_REFERER`) and `X-Title` (from config or hardcoded `"opi"`).
- Base URL defaults to `https://openrouter.ai/api/v1`.

**Mistral-specific**:
- Base URL defaults to `https://api.mistral.ai/v1`.

**Tests**:
- Unit tests for `build_provider()` with mock env for each provider type.
- Verify error messages for missing API keys.

**Files modified**:
- `crates/opi-coding-agent/src/main.rs` -- extend `build_provider()`
- `crates/opi-coding-agent/src/config.rs` -- extend `ProvidersConfig`, add `ProviderEntry`
- `crates/opi-coding-agent/tests/provider_factory.rs` -- NEW or extend existing

---

### WS3: Thinking Config Passthrough

**Scope**: Wire `[thinking]` config to the provider request layer.

**Changes**:

1. `crates/opi-agent/src/loop_types.rs` -- Add to `AgentLoopConfig`:
   ```rust
   pub thinking: Option<opi_ai::provider::ThinkingConfig>,
   ```

2. `crates/opi-coding-agent/src/harness.rs` -- Map in constructor:
   ```rust
   thinking: config.thinking.enabled.then_some(
       opi_ai::provider::ThinkingConfig { budget_tokens: config.thinking.budget_tokens }
   ),
   ```

3. `crates/opi-agent/src/lib.rs` -- In the agent loop where `Request` is built:
   ```rust
   request.thinking = config.thinking.clone();
   ```

4. `crates/opi-agent/src/lib.rs` -- Fix `process_stream_event()`:
   Replace `_ => None` with explicit handling of `ThinkingStart`, `ThinkingDelta`, `ThinkingEnd`.
   Accumulate thinking content into a `thinking_blocks: Vec<ThinkingBlock>` on the assistant message.

**Tests**:
- Agent-loop integration test: configure thinking, verify `Request.thinking` is set in the outgoing request, verify thinking content is preserved in the result messages.

**Files modified**:
- `crates/opi-agent/src/loop_types.rs` -- add thinking field
- `crates/opi-agent/src/lib.rs` -- pass thinking to Request, fix process_stream_event
- `crates/opi-coding-agent/src/harness.rs` -- map thinking config
- `crates/opi-agent/tests/thinking_integration.rs` -- NEW

---

### WS4: Session Runtime Wiring

**Scope**: Wire `SessionWriter` into `CodingHarness` so conversations persist as JSONL.

**Changes**:

1. `crates/opi-coding-agent/src/session_coordinator.rs` -- NEW file:
   - `SessionCoordinator` struct as described in Section 2.
   - Creates `SessionWriter` on harness init.
   - `on_turn_end()` appends `MessageEntry` for each user/assistant/tool message in the turn.
   - `flush()` writes pending entries on shutdown.

2. `crates/opi-coding-agent/src/harness.rs`:
   - Add `session: Option<SessionCoordinator>` field to `CodingHarness`.
   - After each `prompt()` / `continue_()` call, invoke `session.on_turn_end()`.
   - On drop or cancellation, call `session.flush()`.

3. `crates/opi-coding-agent/src/session_cli.rs`:
   - Change `--resume` from early-return to context rebuilder:
     - Read JSONL via `SessionReader::read_with_recovery()`.
     - Reconstruct active branch `AgentMessage` chain from entries.
     - Return `ResumedSession` with messages + session ID.
   - `main.rs` change: when `--resume` returns session data, inject messages into `CodingHarness` as initial context, then enter normal interactive/non-interactive flow.

4. Session directory: `$XDG_DATA_HOME/opi/sessions/` or `~/.local/share/opi/sessions/` on Linux, `%APPDATA%/opi/sessions/` on Windows.

**Tests**:
- Integration test: run harness with `MockProvider`, verify JSONL file is created with correct entries.
- Resume test: create session file, resume it, verify agent context matches original messages.
- Crash recovery test: truncate JSONL mid-line, verify resume reports corruption and recovers.

**Files modified**:
- `crates/opi-coding-agent/src/session_coordinator.rs` -- NEW
- `crates/opi-coding-agent/src/harness.rs` -- add session field
- `crates/opi-coding-agent/src/session_cli.rs` -- change resume behavior
- `crates/opi-coding-agent/src/main.rs` -- change resume flow
- `crates/opi-coding-agent/tests/session_runtime.rs` -- NEW
- `crates/opi-coding-agent/tests/resume_integration.rs` -- NEW

---

### WS5: Compaction Runtime Wiring

**Scope**: Trigger compaction in the harness runtime when token thresholds are exceeded.

**Changes**:

1. `crates/opi-coding-agent/src/config.rs` -- Add `[compaction]` section:
   ```toml
   [compaction]
   enabled = true
   threshold_tokens = 100_000
   ```

2. `SessionCoordinator::on_turn_end()` -- After accumulating usage:
   ```rust
   if self.compaction.should_compact(self.usage.total_input_tokens(), reason) {
       let output = self.compaction.compact(entries, reason, &hooks)?;
       self.on_compaction(&output);
       return Some(output);
   }
   ```

3. Compaction summary injection: Convert `CompactionSummary` to a system message that gets injected as the first message in the next provider request. Fix `convert_to_llm` in `AgentHooks` to include `CompactionSummary` (currently filtered by `_ => None`).

4. Write `SessionEntry::Compaction` to JSONL after compaction completes.

**Tests**:
- Integration test: configure low threshold, run multi-turn conversation with `MockProvider`, verify compaction triggers, JSONL contains compaction entry, and next turn has compacted context.
- Test that `CompactionSummary` appears in provider-visible context after compaction.

**Files modified**:
- `crates/opi-coding-agent/src/config.rs` -- add `CompactionConfig`
- `crates/opi-coding-agent/src/session_coordinator.rs` -- add compaction logic
- `crates/opi-agent/src/lib.rs` -- fix `CompactionSummary` filtering in `convert_to_llm`
- `crates/opi-coding-agent/tests/compaction_runtime.rs` -- NEW

---

### WS6: Usage/Cost Accumulation

**Scope**: Accumulate token usage from provider responses and display in TUI.

**Changes**:

1. `UsageAccumulator` in `SessionCoordinator` -- accumulate `Usage` from each `Done` event.

2. `crates/opi-coding-agent/src/interactive.rs`:
   - After each `AgentEvent::Done` or `MessageEnd`, read usage from the message.
   - Update `Shell::token_count(cumulative.total_input_tokens())`.

3. JSON mode: emit `AgentSessionEvent::UsageUpdated` after each turn.

**Pricing table**: Out of scope for this pass. The `UsageAccumulator` tracks raw token counts. Cost calculation with a pricing table is deferred to a future phase. The existing `calculate_cost()` function remains available for consumers who supply their own pricing data.

**Tests**:
- Interactive integration test: run with `MockProvider` that returns usage, verify StatusBar shows token count.
- JSON mode test: verify `UsageUpdated` events in NDJSON output.

**Files modified**:
- `crates/opi-coding-agent/src/session_coordinator.rs` -- `UsageAccumulator` logic
- `crates/opi-coding-agent/src/interactive.rs` -- wire usage to StatusBar
- `crates/opi-coding-agent/tests/usage_accumulation.rs` -- NEW

---

### WS7: DiffView Runtime Wiring

**Scope**: Show diffs when the `edit` tool modifies files.

**Changes**:

1. In the `edit` tool implementation (in `opi-coding-agent`), capture before/after file content.

2. In the interactive event handler, when rendering `ToolExecutionEnd` for an edit tool, construct `DiffView` with before/after content and add it to the message display.

3. Add a `DiffPayload` struct to carry before/after content through the tool result path.

**Tests**:
- Snapshot test: edit tool produces diff rendering in TUI output.

**Files modified**:
- `crates/opi-coding-agent/src/tools/edit.rs` -- capture before/after
- `crates/opi-coding-agent/src/interactive.rs` -- render DiffView for edit results
- `crates/opi-coding-agent/tests/diff_view_integration.rs` -- NEW

---

### WS8: Test Gap Closure

**Scope**: Fill identified test coverage gaps.

**New tests**:

1. `crates/opi-ai/tests/openai_chat_lifecycle.rs` -- Wiremock lifecycle for OpenAI Chat adapter.
2. `crates/opi-ai/tests/openrouter_lifecycle.rs` -- Wiremock lifecycle for OpenRouter with header assertions (`HTTP-Referer`, `X-Title`).
3. `crates/opi-ai/tests/mistral_lifecycle.rs` -- Wiremock lifecycle for Mistral with base URL assertions.
4. `crates/opi-coding-agent/tests/json_mode.rs` -- Add AutoRetry NDJSON framing test.
5. `crates/opi-coding-agent/tests/json_subprocess.rs` -- NEW: subprocess E2E for `--json` mode.
6. Each new provider lifecycle test covers: success, 401, 429 with retry-after, 500, cancellation, no-terminal-event.

**Existing test updates**: None required -- existing tests remain valid.

---

### WS9: Cleanups

**Scope**: Minor cleanups identified by audits.

1. **Legacy `StreamEvent`** (`crates/opi-ai/src/stream.rs`): Remove the public legacy `StreamEvent` enum. It was replaced by `AssistantStreamEvent` in task 1.2 and is no longer used by production code. Verify no references exist before removing.

2. **Ledger evidence refresh** (`.opi-impl-state.json`):
   - Fix task 2.9 `behavioral_tests` path from `thinking_fixtures.rs` to `anthropic_fixtures.rs`.
   - Normalize all `verified_at_commit` to 40-character SHA hashes.
   - Set task 2.13 `last_attempt` to a valid timestamp.

3. **Rate-limit header expansion** (`crates/opi-ai/src/retry.rs`):
   - Add HTTP-date parsing for `Retry-After` header (e.g., `Fri, 23 May 2026 12:00:00 GMT`).
   - Add `x-ratelimit-remaining` header parsing (informational, for future use).
   - Add tests for new header formats.

4. **Keybinding defaults**: Verify defaults match `docs/opi-spec.md` examples. If they diverge, update either the code or the spec to match.

---

## 4. Dependency Graph

```
WS1 (HTTP streaming)     ──┐
WS2 (provider factory)   ──┤── independent, parallelizable
WS3 (thinking config)    ──┘
        │
WS4 (session runtime)    ─── depends on WS2 for provider config, WS3 for thinking in session
        │
WS5 (compaction)         ─── depends on WS4 for session coordinator
WS6 (usage/cost)         ─── depends on WS4 for session coordinator
WS7 (DiffView)           ─── independent
        │
WS8 (test gaps)          ─── ongoing, fills in as each WS completes
WS9 (cleanups)           ─── last
```

---

## 5. Risk Assessment

| Risk | Mitigation |
|------|------------|
| OpenAI Responses SSE format differs from Chat Completions | Existing mapper already handles the format; HTTP layer is generic |
| Gemini auth uses different header scheme | Well-documented (`x-goog-api-key`); test with wiremock |
| SessionCoordinator adds complexity to harness | Thin coordinator delegates to existing tested modules |
| Resume context reconstruction may have edge cases | Use existing `SessionReader::read_with_recovery()`, test with corrupt/truncated files |
| Compaction trigger timing affects user experience | Default threshold is generous (100K tokens); test with low threshold |

---

## 6. Verification

After all workstreams complete, run:

```sh
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Additionally verify:
- `cargo run -p opi-coding-agent -- --model openai:gpt-4o --non-interactive "hello"` fails gracefully without API key
- `cargo run -p opi-coding-agent -- --model gemini:gemini-2.0-flash --non-interactive "hello"` fails gracefully without API key
- Interactive mode shows token count in status bar after a response
- `--resume <id>` restores conversation context and continues
- JSON mode outputs `UsageUpdated` events
- Edit tool shows diff in interactive mode
