# Phase 1 Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all verified Critical and High issues from the Phase 1 dual-audit (Opus 4.7 + GPT 5.5) to bring the codebase to Phase 1 exit readiness.

**Architecture:** Fixes are organized bottom-up by crate dependency order: opi-ai first (provider layer), then opi-agent (loop semantics), then opi-coding-agent (CLI/runtime). Each task is independently testable and committable.

**Tech Stack:** Rust (edition 2024), tokio, reqwest (rustls-tls), serde_json, futures-util, tokio-util

---

## Verified Issues Summary

| ID | Severity | Crate | Issue | Status |
|----|----------|-------|-------|--------|
| C1 | Critical | opi-ai | AnthropicProvider::stream is a stub (empty SSE string) | Confirmed |
| C2 | Critical | opi-agent | Text content lost — assistant_content only accumulates ToolCall | Confirmed |
| C3 | Critical | opi-coding-agent | Interactive CLI is stub, not wired to TUI | Confirmed |
| H1 | High | opi-ai | serialize_messages writes tool_call.arguments as string, not JSON object | Confirmed |
| H2 | High | opi-ai | No provider lifecycle contract test (Start→deltas→Done/Error) | Confirmed |
| H3 | High | opi-agent | Tool batch execution always serial, ignores ExecutionMode | Confirmed |
| H4 | High | opi-agent | ToolResult.terminate not used in loop decisions | Confirmed |
| H5 | High | opi-agent | transform_context / prepare_next_turn hooks not wired | Confirmed |
| H6 | High | opi-coding-agent | --config path accepted but never read | Confirmed |
| H7 | High | opi-coding-agent | --system prompt file not read or injected | Confirmed |
| H8 | High | opi-coding-agent | Interactive mode has no mutating-tool safety policy | Confirmed |
| H9 | High | opi-coding-agent | Auth failure exits with code 2, spec requires 3 | Confirmed |
| M1 | Medium | opi-agent | should_stop_after_turn receives ALL history tool_results | Confirmed |
| M5 | Medium | opi-coding-agent | ReadTool inside_workspace hardcoded to true | Confirmed |
| L4 | Low | opi-coding-agent | Mutex::lock().unwrap() in production code | Confirmed |

## File Map

### opi-ai (Tasks 1–3)
- Modify: `crates/opi-ai/src/anthropic.rs` — real HTTP SSE stream + serialize fix
- Create: `crates/opi-ai/tests/provider_lifecycle.rs` — contract tests
- Modify: `crates/opi-ai/tests/anthropic_fixtures.rs` — serialize body assertions

### opi-agent (Tasks 4–8)
- Modify: `crates/opi-agent/src/lib.rs` — text accumulation, batch execution, terminate, hooks wiring, M1 fix
- Modify: `crates/opi-agent/tests/agent_loop_mock.rs` — text content, batch, terminate tests
- Create: `crates/opi-agent/tests/agent_loop_semantics.rs` — extended semantic tests

### opi-coding-agent (Tasks 9–14)
- Modify: `crates/opi-coding-agent/src/config.rs` — --config path loading
- Modify: `crates/opi-coding-agent/src/main.rs` — --system, auth exit code, interactive TUI wiring
- Modify: `crates/opi-coding-agent/src/harness.rs` — interactive safety hooks
- Modify: `crates/opi-coding-agent/src/runner.rs` — Mutex unwrap removal
- Modify: `crates/opi-coding-agent/src/tool/read.rs` — inside_workspace fix
- Create: `crates/opi-coding-agent/tests/cli_e2e.rs` — process-level CLI tests

---

## Task 1: Fix serialize_messages tool_call input (H1)

**Files:**
- Modify: `crates/opi-ai/src/anthropic.rs:698-704`
- Modify: `crates/opi-ai/tests/anthropic_fixtures.rs`

**Why first:** This is a one-line fix with clear semantics — parse `tool_call.arguments` (a JSON string) into a `serde_json::Value` before embedding it in the request body. Anthropic API requires `input` to be a JSON object, not a string.

- [ ] **Step 1: Write failing test for tool_call input serialization**

In `crates/opi-ai/tests/anthropic_fixtures.rs`, add a test that verifies the serialized request body has `input` as a JSON object:

```rust
#[test]
fn serialize_tool_call_input_is_object() {
    use opi_ai::message::{AssistantContent, AssistantMessage, Message, ToolCall};

    let msg = Message::Assistant(AssistantMessage {
        content: vec![AssistantContent::ToolCall {
            tool_call: ToolCall {
                id: "tc_1".into(),
                name: "read".into(),
                arguments: r#"{"path":"/tmp/foo.txt"}"#.into(),
            },
        }],
        ..Default::default()
    });

    let serialized = opi_ai::anthropic::serialize_messages(&[msg]);
    let input = &serialized[0]["content"][0]["input"];
    assert!(input.is_object(), "input must be a JSON object, got: {input}");
    assert_eq!(input["path"], "/tmp/foo.txt");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-ai -- serialize_tool_call_input_is_object`
Expected: FAIL — `input` is currently a string, not an object.

- [ ] **Step 3: Fix serialize_messages in anthropic.rs**

In `crates/opi-ai/src/anthropic.rs`, change the ToolCall arm (~line 698-704):

```rust
AssistantContent::ToolCall { tool_call } => {
    let input: serde_json::Value =
        serde_json::from_str(&tool_call.arguments).unwrap_or(json!({}));
    serde_json::json!({
        "type": "tool_use",
        "id": tool_call.id,
        "name": tool_call.name,
        "input": input,
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p opi-ai -- serialize_tool_call_input_is_object`
Expected: PASS

- [ ] **Step 5: Run full opi-ai test suite**

Run: `cargo test -p opi-ai`
Expected: All tests pass (no regressions).

- [ ] **Step 6: Commit**

```bash
git add crates/opi-ai/src/anthropic.rs crates/opi-ai/tests/anthropic_fixtures.rs
git commit -m "fix(opi-ai): serialize tool_call input as JSON object, not string"
```

---

## Task 2: Implement real AnthropicProvider::stream HTTP SSE (C1)

**Files:**
- Modify: `crates/opi-ai/src/anthropic.rs:731-734`
- Create: `crates/opi-ai/tests/provider_lifecycle.rs`

**Context:** The current `stream` method creates an empty SSE string. The real implementation must: POST to `https://api.anthropic.com/v1/messages` with streaming enabled, parse SSE events from the response body, map them through the existing `AnthropicMapper`, and respect `Request::cancel` for abort.

- [ ] **Step 1: Write provider lifecycle contract test**

Create `crates/opi-ai/tests/provider_lifecycle.rs`:

```rust
//! Provider lifecycle contract: Start → deltas → exactly one Done|Error.

use futures_util::StreamExt;
use opi_ai::stream::AssistantStreamEvent;

/// Helper to assert stream contract on any EventStream.
async fn assert_lifecycle(mut stream: opi_ai::provider::EventStream) {
    let mut saw_start = false;
    let mut saw_terminal = false;
    let mut delta_count = 0;

    while let Some(item) = stream.next().await {
        let event = item.expect("stream item should be Ok");
        match &event {
            AssistantStreamEvent::Start { .. } => {
                assert!(!saw_start, "duplicate Start");
                assert!(!saw_terminal, "Start after terminal");
                saw_start = true;
            }
            AssistantStreamEvent::TextDelta { .. }
            | AssistantStreamEvent::ToolCallDelta { .. } => {
                assert!(saw_start, "delta before Start");
                assert!(!saw_terminal, "delta after terminal");
                delta_count += 1;
            }
            AssistantStreamEvent::Done { .. } | AssistantStreamEvent::Error { .. } => {
                assert!(saw_start, "terminal before Start");
                assert!(!saw_terminal, "duplicate terminal");
                saw_terminal = true;
            }
            _ => {}
        }
    }
    assert!(saw_start, "no Start event");
    assert!(saw_terminal, "no terminal event (Done or Error)");
}

#[tokio::test]
async fn mock_provider_respects_lifecycle() {
    let provider = opi_ai::test_support::MockProviderBuilder::new()
        .text_response("hello")
        .build();
    let request = opi_ai::provider::Request::default_test();
    let stream = provider.stream(request);
    assert_lifecycle(stream).await;
}
```

- [ ] **Step 2: Run test to verify it passes with MockProvider**

Run: `cargo test -p opi-ai -- mock_provider_respects_lifecycle`
Expected: PASS (MockProvider already emits Start/Done).

- [ ] **Step 3: Implement real HTTP SSE in AnthropicProvider::stream**

Replace the stub in `crates/opi-ai/src/anthropic.rs` (~line 731-734). The implementation must:

1. Build the POST request to `{base_url}/v1/messages` with headers (`x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`).
2. Set `"stream": true` in the JSON body.
3. Use `reqwest::Client::post(...).send().await` to get a streaming response.
4. Parse SSE lines from the response body using a line-based reader.
5. Map parsed SSE events through the existing `AnthropicMapper` (already implemented).
6. Respect `Request::cancel` — select between cancel token and next SSE chunk.
7. On HTTP error status, emit an `Error` terminal event.
8. On stream end without terminal event, emit an `Error` event (H2 fix).

```rust
fn stream(&self, request: Request) -> EventStream {
    let client = self.client.clone();
    let base_url = self.base_url.clone();
    let api_key = self.api_key.clone();
    let cancel = request.cancel.clone();

    let stream = async_stream::stream! {
        let body = build_request_body(&request);
        let url = format!("{base_url}/v1/messages");

        let response = match client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                yield Err(ProviderError::Network(e.to_string()));
                return;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            yield Err(ProviderError::Api {
                status: status.as_u16(),
                message: body_text,
            });
            return;
        }

        let mut sse_buffer = String::new();
        let mut byte_stream = response.bytes_stream();
        let mut saw_terminal = false;

        loop {
            let chunk = tokio::select! {
                biased;
                _ = cancel.cancelled() => { break; }
                chunk = byte_stream.next() => chunk,
            };

            match chunk {
                Some(Ok(bytes)) => {
                    sse_buffer.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(event) = parse_next_sse_event(&mut sse_buffer) {
                        let mapped = self.mapper.map_event(&event);
                        if let Some(ev) = mapped {
                            if matches!(&ev, AssistantStreamEvent::Done { .. }
                                | AssistantStreamEvent::Error { .. }) {
                                saw_terminal = true;
                            }
                            yield Ok(ev);
                        }
                    }
                }
                Some(Err(e)) => {
                    yield Err(ProviderError::Network(e.to_string()));
                    return;
                }
                None => break,
            }
        }

        if !saw_terminal {
            yield Err(ProviderError::Protocol(
                "stream ended without Done or Error event".into(),
            ));
        }
    };

    Box::pin(stream)
}
```

Note: `build_request_body`, `parse_next_sse_event`, and `AnthropicMapper` details depend on existing code structure. The implementer should adapt to the actual helper functions already present.

- [ ] **Step 4: Add ignored live provider test**

In `crates/opi-ai/tests/provider_lifecycle.rs`:

```rust
#[tokio::test]
#[ignore] // Requires OPI_LIVE_TEST=1 and ANTHROPIC_API_KEY
async fn live_anthropic_respects_lifecycle() {
    if std::env::var("OPI_LIVE_TEST").is_err() {
        return;
    }
    let provider = opi_ai::anthropic::AnthropicProvider::from_env()
        .expect("ANTHROPIC_API_KEY must be set");
    let request = opi_ai::provider::Request {
        model: "claude-sonnet-4-20250514".into(),
        system: "Reply with one word.".into(),
        messages: vec![opi_ai::message::Message::user("Hi")],
        max_tokens: 32,
        ..Default::default()
    };
    let stream = provider.stream(request);
    assert_lifecycle(stream).await;
}
```

- [ ] **Step 5: Run full opi-ai test suite**

Run: `cargo test -p opi-ai`
Expected: All non-ignored tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/opi-ai/src/anthropic.rs crates/opi-ai/tests/provider_lifecycle.rs
git commit -m "feat(opi-ai): implement real HTTP SSE streaming for AnthropicProvider"
```

---

## Task 3: Fix agent_loop text content loss (C2)

**Files:**
- Modify: `crates/opi-agent/src/lib.rs:284-315` (process_stream_event)
- Modify: `crates/opi-agent/tests/agent_loop_mock.rs`

**Context:** `process_stream_event` only pushes `ToolCallEnd` into `assistant_content`. Text from `TextDelta`/`Done` is never accumulated. Then line 115 overwrites `assistant_msg.content` with the incomplete `assistant_content`.

Fix: accumulate `AssistantContent::Text` blocks in `assistant_content` during `TextDelta` events (or from the `Done` message).

- [ ] **Step 1: Write failing test for text content preservation**

In `crates/opi-agent/tests/agent_loop_mock.rs`:

```rust
#[tokio::test]
async fn agent_loop_preserves_text_content() {
    // Configure MockProvider to return a text-only response (no tool calls)
    let provider = MockProviderBuilder::new()
        .text_response("Hello, I can help with that.")
        .build();

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![],
        messages: vec![AgentMessage::Llm(Message::user("Help me"))],
        model: "mock:test".into(),
        system: String::new(),
    };

    let events = collect_events();
    let result = agent_loop(context, AgentLoopConfig::default(), &NoOpHooks, events.sink(), CancellationToken::new()).await.unwrap();

    // The assistant message must contain the text
    let assistant = result.iter().find_map(|m| match m {
        AgentMessage::Llm(Message::Assistant(a)) => Some(a),
        _ => None,
    }).expect("should have assistant message");

    let has_text = assistant.content.iter().any(|c| matches!(c, AssistantContent::Text { .. }));
    assert!(has_text, "assistant message must preserve text content, got: {:?}", assistant.content);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-agent -- agent_loop_preserves_text_content`
Expected: FAIL — text content is empty.

- [ ] **Step 3: Fix process_stream_event to accumulate text**

In `crates/opi-agent/src/lib.rs`, modify `process_stream_event`:

```rust
fn process_stream_event(
    event: &opi_ai::stream::AssistantStreamEvent,
    content: &mut Vec<AssistantContent>,
    events: &AgentEventSink,
) -> Option<opi_ai::message::AssistantMessage> {
    use opi_ai::stream::AssistantStreamEvent::*;

    match event {
        Start { partial } => {
            let msg = AgentMessage::Llm(Message::Assistant(partial.clone()));
            events(AgentEvent::MessageStart { message: msg });
            None
        }
        TextDelta { partial, delta } => {
            // Accumulate text into content vector
            match content.last_mut() {
                Some(AssistantContent::Text { text }) => {
                    text.push_str(delta);
                }
                _ => {
                    content.push(AssistantContent::Text {
                        text: delta.clone(),
                    });
                }
            }
            let msg = AgentMessage::Llm(Message::Assistant(partial.clone()));
            events(AgentEvent::MessageUpdate {
                message: msg,
                assistant_event: Box::new(event.clone()),
            });
            None
        }
        ToolCallEnd { tool_call, .. } => {
            content.push(AssistantContent::ToolCall {
                tool_call: tool_call.clone(),
            });
            None
        }
        Done { message, .. } => Some(message.clone()),
        Error { message, .. } => Some(message.clone()),
        _ => None,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p opi-agent -- agent_loop_preserves_text_content`
Expected: PASS

- [ ] **Step 5: Run full opi-agent test suite**

Run: `cargo test -p opi-agent`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/opi-agent/src/lib.rs crates/opi-agent/tests/agent_loop_mock.rs
git commit -m "fix(opi-agent): preserve text content in assistant messages during agent_loop"
```

---

## Task 4: Implement tool batch execution mode (H3)

**Files:**
- Modify: `crates/opi-agent/src/lib.rs:136-177`
- Create: `crates/opi-agent/tests/agent_loop_semantics.rs`

**Context:** Per spec §8.3: "parallel by default; any sequential tool makes the whole batch sequential." Currently all tools execute in a serial `for` loop regardless of `ExecutionMode`.

- [ ] **Step 1: Write failing test for parallel batch execution**

Create `crates/opi-agent/tests/agent_loop_semantics.rs`:

```rust
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::test]
async fn parallel_tools_execute_concurrently() {
    // Two parallel tools that each sleep 50ms
    // If truly parallel, total time < 80ms; if serial, > 100ms
    let call_count = Arc::new(AtomicU32::new(0));
    let slow_tool = SlowParallelTool {
        delay: Duration::from_millis(50),
        call_count: call_count.clone(),
    };

    let provider = MockProviderBuilder::new()
        .tool_call_response(vec![
            ToolCall { id: "t1".into(), name: "slow".into(), arguments: "{}".into() },
            ToolCall { id: "t2".into(), name: "slow".into(), arguments: "{}".into() },
        ])
        .build();

    let start = Instant::now();
    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![Box::new(slow_tool)],
        messages: vec![AgentMessage::Llm(Message::user("go"))],
        model: "mock:test".into(),
        system: String::new(),
    };

    let _ = agent_loop(context, AgentLoopConfig::default(), &NoOpHooks, no_op_events(), CancellationToken::new()).await;

    let elapsed = start.elapsed();
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
    assert!(elapsed < Duration::from_millis(80), "parallel tools took {elapsed:?}, expected < 80ms");
}

#[tokio::test]
async fn sequential_tool_in_batch_forces_serial() {
    // One sequential tool in a batch of two forces serial execution
    let provider = MockProviderBuilder::new()
        .tool_call_response(vec![
            ToolCall { id: "t1".into(), name: "parallel_tool".into(), arguments: "{}".into() },
            ToolCall { id: "t2".into(), name: "sequential_tool".into(), arguments: "{}".into() },
        ])
        .build();

    // ... test that execution is serial when any tool is sequential
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-agent -- parallel_tools_execute_concurrently`
Expected: FAIL — tools execute serially, taking > 100ms.

- [ ] **Step 3: Implement batch execution logic**

In `crates/opi-agent/src/lib.rs`, replace the serial `for tc in &tool_calls` loop:

```rust
// Determine batch execution mode
let batch_is_sequential = tool_calls.iter().any(|tc| {
    tools_map.get(tc.name.as_str())
        .map(|t| t.execution_mode() == ExecutionMode::Sequential)
        .unwrap_or(true) // unknown tools default to sequential
});

let tool_results = if batch_is_sequential {
    // Execute serially
    let mut results = Vec::new();
    for tc in &tool_calls {
        let args: serde_json::Value =
            serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
        events(AgentEvent::ToolExecutionStart {
            tool_call_id: tc.id.clone(),
            tool_name: tc.name.clone(),
            args: args.clone(),
        });
        let result = execute_tool(&tc.id, &tc.name, &args, &tools_map, hooks, &messages, cancel.clone()).await;
        let is_error = result.is_error;
        events(AgentEvent::ToolExecutionEnd {
            tool_call_id: tc.id.clone(),
            tool_name: tc.name.clone(),
            result: serde_json::json!(&result.content),
            is_error,
        });
        results.push((tc, result));
    }
    results
} else {
    // Execute in parallel
    let futures: Vec<_> = tool_calls.iter().map(|tc| {
        let args: serde_json::Value =
            serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
        let tools_map = &tools_map;
        let messages = &messages;
        let cancel = cancel.clone();
        async move {
            let result = execute_tool(&tc.id, &tc.name, &args, tools_map, hooks, messages, cancel).await;
            (tc, result)
        }
    }).collect();
    futures_util::future::join_all(futures).await
};

// Emit events and build messages in source order
for (tc, result) in &tool_results {
    let trm = ToolResultMessage {
        tool_call_id: tc.id.clone(),
        tool_name: tc.name.clone(),
        content: result.content.clone(),
        details: result.details.clone(),
        is_error: result.is_error,
        timestamp_ms: 0,
    };
    tool_results_vec.push(trm.clone());
    messages.push(AgentMessage::Llm(Message::ToolResult(trm)));
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p opi-agent -- parallel_tools_execute_concurrently`
Expected: PASS

- [ ] **Step 5: Run full opi-agent test suite**

Run: `cargo test -p opi-agent`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/opi-agent/src/lib.rs crates/opi-agent/tests/agent_loop_semantics.rs
git commit -m "feat(opi-agent): implement batch execution mode (parallel/sequential)"
```

---

## Task 5: Implement ToolResult.terminate early-stop (H4)

**Files:**
- Modify: `crates/opi-agent/src/lib.rs`
- Modify: `crates/opi-agent/tests/agent_loop_semantics.rs`

**Context:** Per spec: "early stop only when every finalized result in the batch has `terminate`." The `terminate` field exists on `ToolResult` but is never checked.

- [ ] **Step 1: Write failing test for terminate early-stop**

In `crates/opi-agent/tests/agent_loop_semantics.rs`:

```rust
#[tokio::test]
async fn all_terminate_results_cause_early_stop() {
    // Provider returns tool calls; all tools return terminate=true
    // Agent loop should stop after this turn without calling provider again
    let provider = MockProviderBuilder::new()
        .tool_call_response(vec![
            ToolCall { id: "t1".into(), name: "done_tool".into(), arguments: "{}".into() },
        ])
        .then_text_response("should not reach here")
        .build();

    let done_tool = TerminatingTool; // Tool that returns terminate: true

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![Box::new(done_tool)],
        messages: vec![AgentMessage::Llm(Message::user("go"))],
        model: "mock:test".into(),
        system: String::new(),
    };

    let result = agent_loop(context, AgentLoopConfig::default(), &NoOpHooks, no_op_events(), CancellationToken::new()).await.unwrap();

    // Should NOT contain the "should not reach here" text
    let has_second_response = result.iter().any(|m| match m {
        AgentMessage::Llm(Message::Assistant(a)) => a.content.iter().any(|c| match c {
            AssistantContent::Text { text } => text.contains("should not reach here"),
            _ => false,
        }),
        _ => false,
    });
    assert!(!has_second_response, "agent should have stopped after terminate");
}

#[tokio::test]
async fn partial_terminate_does_not_stop() {
    // Two tools: one returns terminate=true, one returns terminate=false
    // Agent loop should NOT early-stop
    // ... similar setup but with mixed terminate values
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-agent -- all_terminate_results_cause_early_stop`
Expected: FAIL — agent continues to next turn.

- [ ] **Step 3: Add terminate check after tool execution**

In `crates/opi-agent/src/lib.rs`, after collecting tool results from the batch, add:

```rust
// Check terminate: early stop only if ALL results have terminate=true
let all_terminate = tool_results.iter().all(|(_, r)| r.terminate);
if all_terminate && !tool_results.is_empty() {
    events(AgentEvent::TurnEnd {
        message: agent_msg,
        tool_results: tool_results_vec,
    });
    events(AgentEvent::AgentEnd { messages: messages.clone() });
    return Ok(messages);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p opi-agent -- terminate`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/opi-agent/src/lib.rs crates/opi-agent/tests/agent_loop_semantics.rs
git commit -m "feat(opi-agent): implement ToolResult.terminate early-stop semantics"
```

---

## Task 6: Wire transform_context and prepare_next_turn hooks (H5)

**Files:**
- Modify: `crates/opi-agent/src/lib.rs`
- Modify: `crates/opi-agent/tests/agent_loop_semantics.rs`

**Context:** The `AgentHooks` trait defines `transform_context` (called before provider request to transform messages) and `prepare_next_turn` (called before next iteration to inject steering). Neither is called in the loop.

- [ ] **Step 1: Write failing test for transform_context**

```rust
#[tokio::test]
async fn transform_context_is_called_before_provider() {
    let transform_called = Arc::new(AtomicBool::new(false));
    let hooks = TransformTrackingHooks {
        called: transform_called.clone(),
    };

    let provider = MockProviderBuilder::new()
        .text_response("ok")
        .build();

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![],
        messages: vec![AgentMessage::Llm(Message::user("test"))],
        model: "mock:test".into(),
        system: String::new(),
    };

    let _ = agent_loop(context, AgentLoopConfig::default(), &hooks, no_op_events(), CancellationToken::new()).await;
    assert!(transform_called.load(Ordering::SeqCst), "transform_context must be called");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-agent -- transform_context_is_called`
Expected: FAIL

- [ ] **Step 3: Wire transform_context before provider call**

In `crates/opi-agent/src/lib.rs`, before `hooks.convert_to_llm(&messages)`:

```rust
// Transform context before conversion
let transformed = hooks.transform_context(messages.clone(), cancel.clone()).await?;
let llm_messages = hooks.convert_to_llm(&transformed)?;
```

- [ ] **Step 4: Wire prepare_next_turn before next iteration**

After the `TurnEnd` event and `should_stop_after_turn` check, before the next loop iteration:

```rust
// Prepare next turn
let next_turn_ctx = PrepareNextTurnContext {
    messages: messages.clone(),
    turn: turn_idx as u32 + 1,
};
if let Some(update) = hooks.prepare_next_turn(next_turn_ctx).await {
    if let Some(inject) = update.inject_messages {
        messages.extend(inject);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p opi-agent`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/opi-agent/src/lib.rs crates/opi-agent/tests/agent_loop_semantics.rs
git commit -m "feat(opi-agent): wire transform_context and prepare_next_turn hooks"
```

---

## Task 7: Fix should_stop_after_turn to use current turn only (M1)

**Files:**
- Modify: `crates/opi-agent/src/lib.rs:185-191`
- Modify: `crates/opi-agent/tests/agent_loop_semantics.rs`

**Context:** The `recent_trs` vector collects ALL ToolResult messages from history. It should only pass the current turn's tool results.

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn should_stop_receives_only_current_turn_results() {
    // Hook that records what tool_results it receives
    let received = Arc::new(Mutex::new(Vec::new()));
    let hooks = StopTrackingHooks { received: received.clone() };

    // Provider: turn 1 calls tool A, turn 2 calls tool B, then text
    let provider = MockProviderBuilder::new()
        .tool_call_response(vec![ToolCall { id: "t1".into(), name: "a".into(), arguments: "{}".into() }])
        .tool_call_response(vec![ToolCall { id: "t2".into(), name: "b".into(), arguments: "{}".into() }])
        .text_response("done")
        .build();

    // ... run agent_loop

    let calls = received.lock().unwrap();
    // First call should have 1 result (tool A)
    assert_eq!(calls[0].len(), 1);
    assert_eq!(calls[0][0].tool_name, "a");
    // Second call should have 1 result (tool B), NOT 2
    assert_eq!(calls[1].len(), 1);
    assert_eq!(calls[1][0].tool_name, "b");
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — second call receives both tool A and tool B results.

- [ ] **Step 3: Fix to pass only current turn results**

Replace the `recent_trs` collection (lines 185-191) with:

```rust
let stop_ctx = ShouldStopAfterTurnContext {
    messages: messages.clone(),
    tool_results: tool_results_vec.clone(), // already collected for this turn
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p opi-agent`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/opi-agent/src/lib.rs crates/opi-agent/tests/agent_loop_semantics.rs
git commit -m "fix(opi-agent): pass only current-turn tool_results to should_stop_after_turn"
```

---

## Task 8: Implement --config path loading (H6)

**Files:**
- Modify: `crates/opi-coding-agent/src/config.rs`
- Modify: `crates/opi-coding-agent/tests/config_loading.rs`

**Context:** `ConfigSource.config_path` is accepted from CLI but `resolve_config` never reads it. Per spec §9.1.1, CLI arguments have highest precedence.

- [ ] **Step 1: Write failing test for --config path**

In `crates/opi-coding-agent/tests/config_loading.rs`:

```rust
#[test]
fn config_path_is_loaded_and_has_highest_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let config_file = dir.path().join("custom.toml");
    std::fs::write(&config_file, r#"
[general]
model = "anthropic:custom-model"
"#).unwrap();

    let source = ConfigSource {
        config_path: Some(config_file),
        ..Default::default()
    };

    let config = resolve_config(source).unwrap();
    assert_eq!(config.model, "anthropic:custom-model");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-coding-agent -- config_path_is_loaded`
Expected: FAIL — config_path is ignored, model is default.

- [ ] **Step 3: Implement config_path loading in resolve_config**

In `crates/opi-coding-agent/src/config.rs`, add loading from `config_path` with CLI precedence:

```rust
pub fn resolve_config(source: ConfigSource) -> Result<Config, ConfigError> {
    let mut config = Config::default();

    // Layer 5: built-in defaults (already set)
    // Layer 4: user config
    if let Some(user_config) = load_user_config()? {
        config.merge(user_config);
    }
    // Layer 3: project config
    if let Some(project_config) = load_project_config()? {
        config.merge(project_config);
    }
    // Layer 2: env vars
    config.apply_env_vars();
    // Layer 1 (highest): CLI config file, then CLI individual args
    if let Some(path) = &source.config_path {
        let cli_config = load_config_from_path(path)?;
        config.merge(cli_config);
    }
    // CLI individual args override even --config file
    if let Some(model) = &source.model {
        config.model = model.clone();
    }

    Ok(config)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p opi-coding-agent -- config`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/opi-coding-agent/src/config.rs crates/opi-coding-agent/tests/config_loading.rs
git commit -m "fix(opi-coding-agent): implement --config path loading with CLI precedence"
```

---

## Task 9: Implement --system prompt file injection (H7)

**Files:**
- Modify: `crates/opi-coding-agent/src/main.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/tests/system_prompt.rs`

**Context:** `--system <PATH>` is defined in CLI struct but never read or passed to `SystemPromptBuilder::user_system()`.

- [ ] **Step 1: Write failing test**

In `crates/opi-coding-agent/tests/system_prompt.rs`:

```rust
#[test]
fn system_prompt_includes_user_system_file() {
    let dir = tempfile::tempdir().unwrap();
    let system_file = dir.path().join("custom_system.md");
    std::fs::write(&system_file, "You are a helpful code reviewer.").unwrap();

    let prompt = SystemPromptBuilder::new()
        .user_system_file(&system_file)
        .build();

    assert!(prompt.contains("You are a helpful code reviewer."));
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — `user_system_file` method doesn't exist yet.

- [ ] **Step 3: Add user_system_file to SystemPromptBuilder**

```rust
pub fn user_system_file(mut self, path: &Path) -> Self {
    match std::fs::read_to_string(path) {
        Ok(content) => self.user_system = Some(content),
        Err(e) => {
            tracing::warn!("failed to read system prompt file {}: {e}", path.display());
        }
    }
    self
}
```

- [ ] **Step 4: Wire --system in main.rs**

In `main.rs`, pass `cli.system` to the harness builder:

```rust
let harness = CodingHarness::builder()
    .config(config)
    .system_prompt_file(cli.system.as_deref())
    .build()?;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p opi-coding-agent -- system_prompt`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add crates/opi-coding-agent/src/main.rs crates/opi-coding-agent/src/harness.rs crates/opi-coding-agent/src/prompt.rs crates/opi-coding-agent/tests/system_prompt.rs
git commit -m "feat(opi-coding-agent): implement --system prompt file reading and injection"
```

---

## Task 10: Add interactive mode safety policy (H8)

**Files:**
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Create: `crates/opi-coding-agent/tests/interactive_safety.rs`

**Context:** Per spec: "Interactive mode SHOULD require confirmation for [write, edit, bash]." Currently the interactive `CodingAgentHooks` has no `before_tool_call` logic — all tools are auto-allowed.

- [ ] **Step 1: Write test for interactive safety hooks**

Create `crates/opi-coding-agent/tests/interactive_safety.rs`:

```rust
#[tokio::test]
async fn interactive_hooks_deny_mutating_tools_by_default() {
    let hooks = InteractiveCodingHooks::new(/* no auto-allow */);

    let ctx = BeforeToolCallContext {
        tool_call_id: "t1".into(),
        tool_name: "bash".into(),
        args: serde_json::json!({"command": "rm -rf /"}),
        messages: vec![],
    };

    let result = hooks.before_tool_call(ctx).await;
    assert!(matches!(result, BeforeToolCallResult::Deny { .. }));
}

#[tokio::test]
async fn interactive_hooks_allow_read_tools() {
    let hooks = InteractiveCodingHooks::new();

    let ctx = BeforeToolCallContext {
        tool_call_id: "t1".into(),
        tool_name: "read".into(),
        args: serde_json::json!({"path": "/tmp/foo.txt"}),
        messages: vec![],
    };

    let result = hooks.before_tool_call(ctx).await;
    assert!(matches!(result, BeforeToolCallResult::Allow));
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — `InteractiveCodingHooks` doesn't exist.

- [ ] **Step 3: Implement InteractiveCodingHooks**

In `crates/opi-coding-agent/src/harness.rs`:

```rust
pub struct InteractiveCodingHooks {
    auto_allow_mutating: bool,
}

impl InteractiveCodingHooks {
    pub fn new() -> Self {
        Self { auto_allow_mutating: false }
    }

    pub fn with_auto_allow(mut self) -> Self {
        self.auto_allow_mutating = true;
        self
    }

    fn is_mutating_tool(name: &str) -> bool {
        matches!(name, "write" | "edit" | "bash")
    }
}

impl AgentHooks for InteractiveCodingHooks {
    fn before_tool_call(
        &self,
        ctx: BeforeToolCallContext,
    ) -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>> {
        let allow = self.auto_allow_mutating || !Self::is_mutating_tool(&ctx.tool_name);
        Box::pin(async move {
            if allow {
                BeforeToolCallResult::Allow
            } else {
                BeforeToolCallResult::Deny {
                    reason: format!("interactive mode requires confirmation for '{}'", ctx.tool_name),
                }
            }
        })
    }

    // ... delegate other methods to default
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p opi-coding-agent -- interactive_safety`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/opi-coding-agent/src/harness.rs crates/opi-coding-agent/tests/interactive_safety.rs
git commit -m "feat(opi-coding-agent): add interactive mode safety policy for mutating tools"
```

---

## Task 11: Fix auth failure exit code (H9)

**Files:**
- Modify: `crates/opi-coding-agent/src/main.rs`
- Modify: `crates/opi-coding-agent/tests/non_interactive.rs`

**Context:** Per spec, exit code 3 = authentication failure. Currently auth failures (missing API key) exit with code 2 (config error).

- [ ] **Step 1: Write failing test**

In `crates/opi-coding-agent/tests/non_interactive.rs`:

```rust
#[test]
fn missing_api_key_exits_with_code_3() {
    // Run opi with no API key configured
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_opi"))
        .arg("--non-interactive")
        .arg("hello")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPI_API_KEY")
        .output()
        .expect("failed to run opi");

    assert_eq!(output.status.code(), Some(3), "auth failure should exit with code 3");
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — exits with code 2.

- [ ] **Step 3: Distinguish auth errors from config errors in main.rs**

```rust
// In the error handling path:
match build_provider(&config) {
    Ok(provider) => provider,
    Err(e) if e.is_auth_failure() => {
        eprintln!("authentication error: {e}");
        std::process::exit(3);
    }
    Err(e) => {
        eprintln!("configuration error: {e}");
        std::process::exit(2);
    }
}
```

Add an `is_auth_failure` method or pattern match on the error type to detect missing/invalid API keys.

- [ ] **Step 4: Run tests**

Run: `cargo test -p opi-coding-agent -- missing_api_key`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/opi-coding-agent/src/main.rs crates/opi-coding-agent/tests/non_interactive.rs
git commit -m "fix(opi-coding-agent): exit with code 3 on auth failure per spec"
```

---

## Task 12: Fix ReadTool inside_workspace (M5) and Mutex::unwrap (L4)

**Files:**
- Modify: `crates/opi-coding-agent/src/tool/read.rs:96`
- Modify: `crates/opi-coding-agent/src/runner.rs:92,107`
- Modify: `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`

**Context:** Two quick fixes bundled: (1) `inside_workspace` is hardcoded `true` instead of computed; (2) `Mutex::lock().unwrap()` in production code should handle poison gracefully.

- [ ] **Step 1: Write failing test for inside_workspace**

```rust
#[tokio::test]
async fn read_tool_reports_outside_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    let outside_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(outside_file.path(), "secret").unwrap();

    let tool = ReadTool::new(workspace.path().to_path_buf());
    let args = serde_json::json!({ "path": outside_file.path().to_str().unwrap() });
    let result = tool.execute(args, CancellationToken::new()).await;

    let details = result.details.unwrap();
    assert_eq!(details["inside_workspace"], false);
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — always returns `true`.

- [ ] **Step 3: Fix inside_workspace computation**

In `crates/opi-coding-agent/src/tool/read.rs`:

```rust
let canonical_path = std::fs::canonicalize(&args.path).unwrap_or(args.path.clone().into());
let canonical_root = std::fs::canonicalize(&workspace_root).unwrap_or(workspace_root.clone());
let inside_workspace = canonical_path.starts_with(&canonical_root);

let details = serde_json::json!({
    "workspace_root": workspace_root.to_string_lossy(),
    "path": args.path,
    "inside_workspace": inside_workspace,
});
```

- [ ] **Step 4: Fix Mutex::lock().unwrap() in runner.rs**

Replace:
```rust
tp.lock().unwrap().push(delta.clone());
```
With:
```rust
if let Ok(mut guard) = tp.lock() {
    guard.push(delta.clone());
}
```

And:
```rust
let stdout = text_parts.lock().unwrap().join("");
```
With:
```rust
let stdout = text_parts.lock().map(|g| g.join("")).unwrap_or_default();
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p opi-coding-agent`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add crates/opi-coding-agent/src/tool/read.rs crates/opi-coding-agent/src/runner.rs crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs
git commit -m "fix(opi-coding-agent): compute inside_workspace correctly, handle mutex poison"
```

---

## Task 13: Wire interactive CLI to TUI (C3)

**Files:**
- Modify: `crates/opi-coding-agent/src/main.rs:42-45`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/tests/interactive_mock.rs`

**Context:** This is the largest task. The interactive path currently prints a stub message. It needs to: initialize terminal raw mode, create TUI Shell, wire AgentEvent stream to TUI state updates, handle input, and support Ctrl+C cancellation.

Note: Full TUI event loop with rich rendering is Phase 2 scope. For Phase 1 exit, the minimum is: accept input → send to agent → display streaming text output → accept next input. This can use a simplified loop that doesn't require full Shell/MarkdownView integration.

- [ ] **Step 1: Write mock interactive E2E test**

In `crates/opi-coding-agent/tests/interactive_mock.rs`, add/modify:

```rust
#[tokio::test]
async fn interactive_harness_accepts_and_displays_response() {
    let provider = MockProviderBuilder::new()
        .text_response("Hello! How can I help?")
        .build();

    let harness = CodingHarness::builder()
        .provider(Box::new(provider))
        .tools(default_tools())
        .build();

    // Simulate a prompt
    let response = harness.prompt("Hi there").await.unwrap();

    // Should have received the text response
    let text = response.iter().find_map(|m| match m {
        AgentMessage::Llm(Message::Assistant(a)) => {
            a.content.iter().find_map(|c| match c {
                AssistantContent::Text { text } => Some(text.clone()),
                _ => None,
            })
        }
        _ => None,
    });
    assert_eq!(text.as_deref(), Some("Hello! How can I help?"));
}
```

- [ ] **Step 2: Implement minimal interactive loop in main.rs**

Replace the stub:

```rust
} else {
    // Interactive mode
    let harness = build_harness(&config, InteractiveCodingHooks::new())?;

    // Minimal interactive loop for Phase 1
    let stdin = std::io::stdin();
    let mut input = String::new();

    println!("opi {} - AI coding agent (type 'exit' to quit)", env!("CARGO_PKG_VERSION"));

    loop {
        eprint!("> ");
        input.clear();
        if stdin.read_line(&mut input).is_err() || input.trim() == "exit" {
            break;
        }
        let prompt = input.trim().to_string();
        if prompt.is_empty() {
            continue;
        }

        match harness.prompt(&prompt).await {
            Ok(messages) => {
                for msg in &messages {
                    if let AgentMessage::Llm(Message::Assistant(a)) = msg {
                        for content in &a.content {
                            if let AssistantContent::Text { text } = content {
                                println!("{text}");
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("error: {e}"),
        }
    }
}
```

This is intentionally minimal — it satisfies "accepts a prompt, streams output, displays results" without a full TUI event loop. Full ratatui integration is Phase 2.

- [ ] **Step 3: Run interactive_mock tests**

Run: `cargo test -p opi-coding-agent -- interactive`
Expected: All pass.

- [ ] **Step 4: Run full workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/opi-coding-agent/src/main.rs crates/opi-coding-agent/src/harness.rs crates/opi-coding-agent/tests/interactive_mock.rs
git commit -m "feat(opi-coding-agent): implement minimal interactive CLI loop"
```

---

## Task 14: Final verification and CI gates

**Files:**
- No new files — verification only

- [ ] **Step 1: Run full CI gate suite**

```bash
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Expected: All pass with zero warnings.

- [ ] **Step 2: Verify binary runs**

```bash
cargo run -p opi-coding-agent -- --version
cargo run -p opi-coding-agent -- --help
```

Expected: Version prints correctly. Help shows --config, --system, --non-interactive flags.

- [ ] **Step 3: Verify non-interactive mode with mock (smoke test)**

```bash
ANTHROPIC_API_KEY="" cargo run -p opi-coding-agent -- --non-interactive "hello"
echo $?  # Should be 3 (auth failure)
```

- [ ] **Step 4: Commit any remaining fixups**

If any lint/format issues arose from earlier tasks, fix and commit.

---

## Execution Order and Dependencies

```
Task 1 (H1: serialize fix)     — independent, quick win
Task 2 (C1: real HTTP SSE)     — depends on Task 1 (serialize correctness)
Task 3 (C2: text content)      — independent of Tasks 1-2
Task 4 (H3: batch execution)   — independent, builds on opi-agent
Task 5 (H4: terminate)         — depends on Task 4 (batch semantics)
Task 6 (H5: hooks wiring)      — depends on Task 4 (loop changes)
Task 7 (M1: stop turn filter)  — depends on Task 4 (same code area)
Task 8 (H6: --config)          — independent
Task 9 (H7: --system)          — independent, can parallel with 8
Task 10 (H8: safety policy)    — depends on Task 6 (hooks infra)
Task 11 (H9: exit code)        — independent
Task 12 (M5+L4: quick fixes)   — independent
Task 13 (C3: interactive CLI)  — depends on Task 10 (safety hooks)
Task 14 (verification)         — depends on all above
```

Recommended parallel groups:
- **Group A** (opi-ai): Tasks 1 → 2
- **Group B** (opi-agent): Tasks 3, 4 → 5 → 6 → 7
- **Group C** (opi-coding-agent): Tasks 8, 9, 11, 12 (all independent)
- **Group D** (integration): Task 10 → 13 → 14

Groups A, B, C can run in parallel. Group D depends on B and C completing.
