# Phase 2 Hardening Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all Critical and High audit findings so Phase 2 is runtime-complete: real HTTP streaming for all providers, session persistence, compaction runtime, thinking passthrough, usage accumulation, provider factory for all 6 providers.

**Architecture:** Add a SessionCoordinator in opi-coding-agent that glues SessionWriter + CompactionEngine + UsageAccumulator into CodingHarness. Implement real HTTP streaming for OpenAI Responses and Gemini following the OpenAI Chat pattern. Extend build_provider() to support all providers. Fix thinking config passthrough and agent loop event handling.

**Tech Stack:** Rust (edition 2024), tokio, reqwest, wiremock (tests), ratatui, serde, proptest

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `crates/opi-ai/src/openai_responses.rs` | Add real HTTP streaming |
| Modify | `crates/opi-ai/src/gemini.rs` | Add real HTTP streaming |
| Create | `crates/opi-ai/tests/openai_responses_lifecycle.rs` | Wiremock lifecycle tests for Responses |
| Create | `crates/opi-ai/tests/gemini_lifecycle.rs` | Wiremock lifecycle tests for Gemini |
| Create | `crates/opi-ai/tests/openai_chat_lifecycle.rs` | Wiremock lifecycle for OpenAI Chat |
| Create | `crates/opi-ai/tests/openrouter_lifecycle.rs` | Wiremock lifecycle for OpenRouter |
| Create | `crates/opi-ai/tests/mistral_lifecycle.rs` | Wiremock lifecycle for Mistral |
| Modify | `crates/opi-coding-agent/src/main.rs` | Extend build_provider() |
| Modify | `crates/opi-coding-agent/src/config.rs` | Add provider map, compaction config |
| Modify | `crates/opi-agent/src/loop_types.rs` | Add thinking to AgentLoopConfig |
| Modify | `crates/opi-agent/src/lib.rs` | Pass thinking to Request, fix process_stream_event |
| Modify | `crates/opi-coding-agent/src/harness.rs` | Map thinking config, add session coordinator |
| Create | `crates/opi-coding-agent/src/session_coordinator.rs` | Session + compaction + usage glue |
| Modify | `crates/opi-coding-agent/src/session_cli.rs` | Real resume (rebuild context) |
| Modify | `crates/opi-coding-agent/src/lib.rs` | Add session_coordinator module |
| Modify | `crates/opi-coding-agent/src/interactive.rs` | Wire usage to StatusBar, wire DiffView |
| Modify | `crates/opi-ai/src/stream.rs` | Remove legacy StreamEvent |
| Modify | `crates/opi-ai/src/retry.rs` | Add HTTP-date parsing for Retry-After |
| Modify | `crates/opi-coding-agent/src/tools/edit.rs` | Capture before/after content |
| Create | `crates/opi-agent/tests/thinking_integration.rs` | Thinking passthrough tests |
| Create | `crates/opi-coding-agent/tests/session_runtime.rs` | Session writing tests |
| Create | `crates/opi-coding-agent/tests/resume_integration.rs` | Resume E2E tests |
| Create | `crates/opi-coding-agent/tests/compaction_runtime.rs` | Compaction trigger tests |
| Create | `crates/opi-coding-agent/tests/usage_accumulation.rs` | Usage accumulation tests |
| Create | `crates/opi-coding-agent/tests/diff_view_integration.rs` | DiffView in tool path tests |
| Create | `crates/opi-coding-agent/tests/provider_factory.rs` | Provider factory tests |
| Modify | `.opi-impl-state.json` | Fix evidence paths |

---

## Task 1: OpenAI Responses HTTP Streaming

**Files:**
- Modify: `crates/opi-ai/src/openai_responses.rs:788-794` (stub stream method)
- Create: `crates/opi-ai/tests/openai_responses_lifecycle.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/opi-ai/tests/openai_responses_lifecycle.rs`:

```rust
//! Wiremock lifecycle tests for the OpenAI Responses provider.
//!
//! Verifies real HTTP streaming: success, auth errors, rate limits,
//! cancellation, and no-terminal-event handling.

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};
use opi_ai::openai_responses::OpenAiResponsesProvider;
use opi_ai::provider::{Provider, Request};
use opi_ai::message::{Message, UserMessage};
use opi_ai::stream::AssistantStreamEvent;
use futures_util::StreamExt;

fn success_sse_body() -> String {
    "event: response.created\ndata: {\"id\":\"resp_1\",\"object\":\"response\",\"model\":\"gpt-4o\",\"status\":\"in_progress\"}\n\n\
     event: response.output_item.added\ndata: {\"type\":\"message\",\"id\":\"msg_1\",\"role\":\"assistant\",\"content\":[]}\n\n\
     event: response.content_part.added\ndata: {\"type\":\"output_text\",\"part\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
     event: response.output_text.delta\ndata: {\"type\":\"output_text\",\"delta\":\"Hello\"}\n\n\
     event: response.output_text.done\ndata: {\"type\":\"output_text\",\"text\":\"Hello\"}\n\n\
     event: response.output_item.done\ndata: {\"type\":\"message\",\"id\":\"msg_1\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello\"}]}\n\n\
     event: response.completed\ndata: {\"id\":\"resp_1\",\"object\":\"response\",\"model\":\"gpt-4o\",\"status\":\"completed\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}\n\n".into()
}

#[tokio::test]
async fn stream_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(success_sse_body())
            .insert_header("content-type", "text/event-stream"))
        .mount(&server)
        .await;

    let provider = OpenAiResponsesProvider::new("test-key".into(), Some(server.uri()));
    let request = Request {
        model: "gpt-4o".into(),
        system: None,
        messages: vec![Message::User(UserMessage { content: "hi".into() })],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let mut stream = provider.stream(request);
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item);
    }

    let done_count = events.iter().filter(|e| matches!(e, Ok(AssistantStreamEvent::Done { .. }))).count();
    assert_eq!(done_count, 1, "expected exactly one Done event, got {done_count}");
}

#[tokio::test]
async fn stream_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(401)
            .set_body_string(r#"{"error":{"message":"invalid api key"}}"#))
        .mount(&server)
        .await;

    let provider = OpenAiResponsesProvider::new("bad-key".into(), Some(server.uri()));
    let request = Request {
        model: "gpt-4o".into(),
        system: None,
        messages: vec![Message::User(UserMessage { content: "hi".into() })],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let mut stream = provider.stream(request);
    let result = stream.next().await;
    assert!(result.is_some(), "expected an error event");
    let event = result.unwrap();
    assert!(event.is_err(), "expected error, got {event:?}");
    let err = event.unwrap_err();
    assert!(matches!(err, opi_ai::provider::ProviderError::AuthFailed(_)),
        "expected AuthFailed, got {err:?}");
}

#[tokio::test]
async fn stream_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(429)
            .insert_header("retry-after", "5")
            .set_body_string(r#"{"error":{"message":"rate limited"}}"#))
        .mount(&server)
        .await;

    let provider = OpenAiResponsesProvider::new("test-key".into(), Some(server.uri()));
    let request = Request {
        model: "gpt-4o".into(),
        system: None,
        messages: vec![Message::User(UserMessage { content: "hi".into() })],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let mut stream = provider.stream(request);
    let result = stream.next().await;
    assert!(result.is_some());
    let err = result.unwrap().unwrap_err();
    assert!(matches!(err, opi_ai::provider::ProviderError::RateLimited { .. }),
        "expected RateLimited, got {err:?}");
}

#[tokio::test]
async fn stream_no_terminal_event() {
    let server = MockServer::start().await;
    // Return an incomplete SSE stream (no response.completed event)
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(
                "event: response.output_text.delta\ndata: {\"type\":\"output_text\",\"delta\":\"Hello\"}\n\n"
            )
            .insert_header("content-type", "text/event-stream"))
        .mount(&server)
        .await;

    let provider = OpenAiResponsesProvider::new("test-key".into(), Some(server.uri()));
    let request = Request {
        model: "gpt-4o".into(),
        system: None,
        messages: vec![Message::User(UserMessage { content: "hi".into() })],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let mut stream = provider.stream(request);
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item);
    }
    let has_stream_error = events.iter().any(|e| {
        matches!(e, Err(opi_ai::provider::ProviderError::StreamError(msg)) if msg.contains("terminal"))
    });
    assert!(has_stream_error, "expected stream error about missing terminal event");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-ai --test openai_responses_lifecycle -- stream_success`
Expected: FAIL (provider returns "HTTP streaming not implemented" error)

- [ ] **Step 3: Implement HTTP streaming for OpenAI Responses**

In `crates/opi-ai/src/openai_responses.rs`, replace the stub `stream()` method and add the supporting types. The implementation follows the exact pattern from `openai_chat.rs`:

1. Add imports at the top of the file (if not already present):
```rust
use futures_util::StreamExt;
```

2. Add `ReceiverStream` struct and `Stream` impl after the `Provider` impl block:
```rust
struct ReceiverStream {
    rx: tokio::sync::mpsc::Receiver<Result<AssistantStreamEvent, ProviderError>>,
}

impl futures_core::Stream for ReceiverStream {
    type Item = Result<AssistantStreamEvent, ProviderError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}
```

3. Add `stream_http()` async method on `OpenAiResponsesProvider`:
```rust
async fn stream_http(
    api_key: String,
    base_url: String,
    body: &serde_json::Value,
    cancel: CancellationToken,
    tx: &tokio::sync::mpsc::Sender<Result<AssistantStreamEvent, ProviderError>>,
) -> Result<(), ProviderError> {
    let client = reqwest::Client::new();
    let req = client
        .post(format!("{base_url}/v1/responses"))
        .header("authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json")
        .body(serde_json::to_string(body).unwrap_or_default());

    let response = req
        .send()
        .await
        .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let headers = response.headers().clone();
        let error_body = response.text().await.unwrap_or_default();
        return Err(map_http_status(status, &error_body, &headers));
    }

    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut mapper = ResponsesMapper::new();

    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => {
                return Ok(());
            }
            chunk = byte_stream.next() => {
                match chunk {
                    Some(c) => c,
                    None => break,
                }
            }
        };

        let chunk = chunk.map_err(|e| ProviderError::StreamError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        for frame in parse_sse_frames(&buffer) {
            let data: &str = &frame.data;
            if data.trim() == "[DONE]" {
                continue;
            }
            match serde_json::from_str::<ResponsesEvent>(data) {
                Ok(event) => {
                    for stream_event in mapper.process(event) {
                        if tx.send(Ok(stream_event)).await.is_err() {
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    let err = ProviderError::StreamError(format!(
                        "malformed SSE data: {e} (data: {})", &data[..data.len().min(80)]
                    ));
                    if tx.send(Err(err)).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
        // Drain consumed frames from buffer
        while let Some(idx) = buffer.find("\n\n") {
            let _ = buffer.drain(..idx + 2);
        }
    }

    if !mapper.saw_done {
        let err = ProviderError::StreamError("stream ended without a terminal event".into());
        let _ = tx.send(Err(err)).await;
    }

    Ok(())
}
```

4. Add `map_http_status()` helper function:
```rust
fn map_http_status(
    status: reqwest::StatusCode,
    body: &str,
    headers: &reqwest::header::HeaderMap,
) -> ProviderError {
    match status.as_u16() {
        401 => ProviderError::AuthFailed(format!("authentication failed: {body}")),
        403 => ProviderError::AuthFailed(format!("access denied: {body}")),
        429 => ProviderError::RateLimited {
            retry_after_ms: crate::retry::parse_retry_after(headers),
        },
        408 | 504 => ProviderError::Timeout,
        code => ProviderError::RequestFailed(format!("HTTP {code}: {body}")),
    }
}
```

5. Replace the stub `stream()` method with the real implementation:
```rust
fn stream(&self, request: Request) -> EventStream {
    let api_key = self.api_key.clone();
    let base_url = self.base_url.clone();
    let body = self.build_request_body(&request);
    let cancel = request.cancel.clone();

    let (tx, rx) = tokio::sync::mpsc::channel(64);

    tokio::spawn(async move {
        if let Err(e) = Self::stream_http(api_key, base_url, &body, cancel, &tx).await {
            let _ = tx.send(Err(e)).await;
        }
    });

    Box::pin(ReceiverStream { rx })
}
```

6. Remove the `#[allow(dead_code)]` attributes from `api_key` and `base_url` fields on the struct since they're now used.

- [ ] **Step 4: Run tests**

Run: `cargo test -p opi-ai --test openai_responses_lifecycle`
Expected: All 4 tests PASS

Run: `cargo test -p opi-ai --test openai_responses_fixtures`
Expected: All existing fixture tests still PASS

- [ ] **Step 5: Commit**

```bash
git add crates/opi-ai/src/openai_responses.rs crates/opi-ai/tests/openai_responses_lifecycle.rs
git commit -m "feat(opi-ai): implement HTTP streaming for OpenAI Responses provider"
```

---

## Task 2: Gemini HTTP Streaming

**Files:**
- Modify: `crates/opi-ai/src/gemini.rs:626-631` (stub stream method)
- Create: `crates/opi-ai/tests/gemini_lifecycle.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/opi-ai/tests/gemini_lifecycle.rs`:

```rust
//! Wiremock lifecycle tests for the Gemini provider.

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};
use opi_ai::gemini::GeminiProvider;
use opi_ai::provider::{Provider, Request};
use opi_ai::message::{Message, UserMessage};
use opi_ai::stream::AssistantStreamEvent;
use futures_util::StreamExt;

fn success_sse_body() -> String {
    "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello\"}],\"role\":\"model\"},\"finishReason\":\"STOP\",\"index\":0}],\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":5,\"totalTokenCount\":15}}\n\n".into()
}

#[tokio::test]
async fn stream_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1beta/models/gemini-2.0-flash:streamGenerateContent"))
        .and(header("x-goog-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(success_sse_body())
            .insert_header("content-type", "text/event-stream"))
        .mount(&server)
        .await;

    let provider = GeminiProvider::new("test-key".into(), Some(server.uri()));
    let request = Request {
        model: "gemini-2.0-flash".into(),
        system: None,
        messages: vec![Message::User(UserMessage { content: "hi".into() })],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let mut stream = provider.stream(request);
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item);
    }

    let done_count = events.iter().filter(|e| matches!(e, Ok(AssistantStreamEvent::Done { .. }))).count();
    assert_eq!(done_count, 1, "expected exactly one Done event");
}

#[tokio::test]
async fn stream_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400)
            .set_body_string(r#"{"error":{"code":401,"message":"API key not valid"}}"#))
        .mount(&server)
        .await;

    let provider = GeminiProvider::new("bad-key".into(), Some(server.uri()));
    let request = Request {
        model: "gemini-2.0-flash".into(),
        system: None,
        messages: vec![Message::User(UserMessage { content: "hi".into() })],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let mut stream = provider.stream(request);
    let result = stream.next().await;
    assert!(result.is_some());
    assert!(result.unwrap().is_err(), "expected error for auth failure");
}

#[tokio::test]
async fn stream_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429)
            .insert_header("retry-after", "3")
            .set_body_string(r#"{"error":{"code":429,"message":"quota exceeded"}}"#))
        .mount(&server)
        .await;

    let provider = GeminiProvider::new("test-key".into(), Some(server.uri()));
    let request = Request {
        model: "gemini-2.0-flash".into(),
        system: None,
        messages: vec![Message::User(UserMessage { content: "hi".into() })],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let mut stream = provider.stream(request);
    let result = stream.next().await;
    assert!(result.is_some());
    let err = result.unwrap().unwrap_err();
    assert!(matches!(err, opi_ai::provider::ProviderError::RateLimited { .. }),
        "expected RateLimited, got {err:?}");
}

#[tokio::test]
async fn stream_no_terminal_event() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hi\"}]}}]}\n\n"
            )
            .insert_header("content-type", "text/event-stream"))
        .mount(&server)
        .await;

    let provider = GeminiProvider::new("test-key".into(), Some(server.uri()));
    let request = Request {
        model: "gemini-2.0-flash".into(),
        system: None,
        messages: vec![Message::User(UserMessage { content: "hi".into() })],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    let mut stream = provider.stream(request);
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item);
    }
    let has_stream_error = events.iter().any(|e| {
        matches!(e, Err(opi_ai::provider::ProviderError::StreamError(msg)) if msg.contains("terminal"))
    });
    assert!(has_stream_error, "expected stream error about missing terminal event");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-ai --test gemini_lifecycle -- stream_success`
Expected: FAIL

- [ ] **Step 3: Implement HTTP streaming for Gemini**

In `crates/opi-ai/src/gemini.rs`, following the same pattern:

1. Add import:
```rust
use futures_util::StreamExt;
```

2. Add `ReceiverStream` struct after the `Provider` impl:
```rust
struct ReceiverStream {
    rx: tokio::sync::mpsc::Receiver<Result<AssistantStreamEvent, ProviderError>>,
}

impl futures_core::Stream for ReceiverStream {
    type Item = Result<AssistantStreamEvent, ProviderError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}
```

3. Add `stream_http()` async method:
```rust
async fn stream_http(
    api_key: String,
    base_url: String,
    model: String,
    body: &serde_json::Value,
    cancel: CancellationToken,
    tx: &tokio::sync::mpsc::Sender<Result<AssistantStreamEvent, ProviderError>>,
) -> Result<(), ProviderError> {
    let client = reqwest::Client::new();
    let url = format!(
        "{base_url}/v1beta/models/{model}:streamGenerateContent?alt=sse"
    );
    let req = client
        .post(&url)
        .header("x-goog-api-key", &api_key)
        .header("content-type", "application/json")
        .body(serde_json::to_string(body).unwrap_or_default());

    let response = req
        .send()
        .await
        .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let headers = response.headers().clone();
        let error_body = response.text().await.unwrap_or_default();
        return Err(map_gemini_error(status, &error_body, &headers));
    }

    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut mapper = GeminiMapper::new();

    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => {
                return Ok(());
            }
            chunk = byte_stream.next() => {
                match chunk {
                    Some(c) => c,
                    None => break,
                }
            }
        };

        let chunk = chunk.map_err(|e| ProviderError::StreamError(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        for data in parse_sse_data(&buffer) {
            match serde_json::from_str::<GenerateContentResponse>(&data) {
                Ok(response) => {
                    if let Some(error) = response.error {
                        let msg = error.message.unwrap_or_else(|| "unknown error".into());
                        let err = ProviderError::RequestFailed(msg);
                        if tx.send(Err(err)).await.is_err() {
                            return Ok(());
                        }
                    } else if let Some(event) = ParsedEvent::from_response(&response) {
                        for stream_event in mapper.process(event) {
                            if tx.send(Ok(stream_event)).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => {
                    let err = ProviderError::StreamError(format!(
                        "malformed SSE data: {e} (data: {})", &data[..data.len().min(80)]
                    ));
                    if tx.send(Err(err)).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
        // Drain consumed data from buffer
        while let Some(idx) = buffer.find("\n\n") {
            let _ = buffer.drain(..idx + 2);
        }
    }

    if !mapper.saw_done {
        let err = ProviderError::StreamError("stream ended without a terminal event".into());
        let _ = tx.send(Err(err)).await;
    }

    Ok(())
}
```

4. Add `map_gemini_error()` helper:
```rust
fn map_gemini_error(
    status: reqwest::StatusCode,
    body: &str,
    headers: &reqwest::header::HeaderMap,
) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::AuthFailed(format!("authentication failed: {body}")),
        429 => ProviderError::RateLimited {
            retry_after_ms: crate::retry::parse_retry_after(headers),
        },
        408 | 504 => ProviderError::Timeout,
        code => ProviderError::RequestFailed(format!("HTTP {code}: {body}")),
    }
}
```

5. Replace the stub `stream()` method:
```rust
fn stream(&self, request: Request) -> EventStream {
    let api_key = self.api_key.clone();
    let base_url = self.base_url.clone();
    let model = request.model.clone();
    let body = self.build_request_body(&request);
    let cancel = request.cancel.clone();

    let (tx, rx) = tokio::sync::mpsc::channel(64);

    tokio::spawn(async move {
        if let Err(e) = Self::stream_http(api_key, base_url, model, &body, cancel, &tx).await {
            let _ = tx.send(Err(e)).await;
        }
    });

    Box::pin(ReceiverStream { rx })
}
```

6. Remove `#[allow(dead_code)]` from `api_key` and `base_url` fields.

Note: The Gemini SSE format uses plain `data:` lines without `event:` prefixes. The existing `parse_sse_data()` already handles this. The `GeminiMapper` and `ParsedEvent::from_response()` (or equivalent -- check the existing `from_data()` method on `ParsedEvent`) handle mapping to `AssistantStreamEvent`. Adapt the parsing logic to match what's already in `stream_from_sse()`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p opi-ai --test gemini_lifecycle`
Expected: All 4 tests PASS

Run: `cargo test -p opi-ai --test gemini_fixtures`
Expected: All existing fixture tests still PASS

- [ ] **Step 5: Commit**

```bash
git add crates/opi-ai/src/gemini.rs crates/opi-ai/tests/gemini_lifecycle.rs
git commit -m "feat(opi-ai): implement HTTP streaming for Gemini provider"
```

---

## Task 3: Provider Factory Extension

**Files:**
- Modify: `crates/opi-coding-agent/src/config.rs:65-85` (ProvidersConfig)
- Modify: `crates/opi-coding-agent/src/main.rs:145-168` (build_provider)
- Create: `crates/opi-coding-agent/tests/provider_factory.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/opi-coding-agent/tests/provider_factory.rs`:

```rust
//! Tests for the provider factory (build_provider function).

use opi_coding_agent::config::OpiConfig;

fn config_with_model(model: &str) -> OpiConfig {
    let mut config = OpiConfig::default();
    config.defaults.model = model.into();
    config
}

// build_provider is private, so we test via the main module's re-export or
// by testing the config resolution + provider construction path.
// Since build_provider is in main.rs (not lib), we test config + provider
// construction directly from the provider crates.

#[test]
fn openai_provider_construction() {
    let provider = opi_ai::openai_chat::OpenAiChatProvider::new("test-key".into(), None);
    assert_eq!(provider.id(), "openai");
}

#[test]
fn openrouter_provider_construction() {
    use opi_ai::openai_chat::{CompatConfig, OpenAiChatProvider};
    let compat = CompatConfig {
        stream_url_suffix: "/chat/completions".into(),
        ..Default::default()
    };
    let provider = OpenAiChatProvider::new_for_profile(
        "test-key".into(),
        "https://openrouter.ai/api/v1".into(),
        "openrouter".into(),
        compat,
        vec![("HTTP-Referer".into(), "https://opi.dev".into())],
        vec![],
    );
    assert_eq!(provider.id(), "openrouter");
}

#[test]
fn gemini_provider_construction() {
    let provider = opi_ai::gemini::GeminiProvider::new("test-key".into(), None);
    assert_eq!(provider.id(), "gemini");
}

#[test]
fn openai_responses_provider_construction() {
    let provider = opi_ai::openai_responses::OpenAiResponsesProvider::new("test-key".into(), None);
    assert_eq!(provider.id(), "openai-responses");
}

#[test]
fn mistral_provider_construction() {
    use opi_ai::openai_chat::{CompatConfig, OpenAiChatProvider};
    let compat = CompatConfig::default();
    let provider = OpenAiChatProvider::new_for_profile(
        "test-key".into(),
        "https://api.mistral.ai/v1".into(),
        "mistral".into(),
        compat,
        vec![],
        vec![],
    );
    assert_eq!(provider.id(), "mistral");
}

#[test]
fn config_providers_default_has_anthropic() {
    let config = OpiConfig::default();
    assert_eq!(config.providers.anthropic.api_key_env, "ANTHROPIC_API_KEY");
}
```

- [ ] **Step 2: Run test to verify it passes (construction tests)**

Run: `cargo test -p opi-coding-agent --test provider_factory`
Expected: All tests PASS (these test existing constructors)

- [ ] **Step 3: Extend ProvidersConfig**

In `crates/opi-coding-agent/src/config.rs`, add generic provider config alongside the existing `anthropic` field. Add to `ProvidersConfig`:

```rust
/// `[providers]` section.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProvidersConfig {
    pub anthropic: AnthropicProviderConfig,
    pub openai: GenericProviderConfig,
    pub openrouter: OpenRouterProviderConfig,
    pub mistral: GenericProviderConfig,
    pub openai_responses: GenericProviderConfig,
    pub gemini: GenericProviderConfig,
}

/// Generic provider config (api_key_env + optional base_url).
#[derive(Debug, Clone, PartialEq)]
pub struct GenericProviderConfig {
    pub api_key_env: String,
    pub base_url: Option<String>,
}

impl Default for GenericProviderConfig {
    fn default() -> Self {
        Self {
            api_key_env: String::new(), // empty = no default
            base_url: None,
        }
    }
}

/// OpenRouter-specific provider config.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenRouterProviderConfig {
    pub api_key_env: String,
    pub base_url: Option<String>,
    pub referer: Option<String>,
}

impl Default for OpenRouterProviderConfig {
    fn default() -> Self {
        Self {
            api_key_env: "OPENROUTER_API_KEY".into(),
            base_url: Some("https://openrouter.ai/api/v1".into()),
            referer: None,
        }
    }
}
```

Also add corresponding TOML deserialization structs in the `TomlProviders` section and update `merge_into` to handle the new fields. Add defaults for `mistral.api_key_env` = `"MISTRAL_API_KEY"`, `gemini.api_key_env` = `"GEMINI_API_KEY"`, `openai.api_key_env` = `"OPENAI_API_KEY"`, `openai_responses.api_key_env` = `"OPENAI_API_KEY"`.

- [ ] **Step 4: Extend build_provider()**

In `crates/opi-coding-agent/src/main.rs`, update `build_provider()`:

```rust
fn build_provider(
    config: &opi_coding_agent::config::OpiConfig,
) -> Result<Box<dyn opi_ai::provider::Provider>, ProviderBuildError> {
    use opi_ai::provider::Provider;

    let spec = &config.defaults.model;
    let (provider_id, _model) = spec.split_once(':').ok_or_else(|| {
        ProviderBuildError::Config(format!(
            "invalid model spec: {spec:?} (expected provider:model)"
        ))
    })?;

    match provider_id {
        "anthropic" => {
            let api_key_env = &config.providers.anthropic.api_key_env;
            let api_key = std::env::var(api_key_env).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {api_key_env} environment variable"
                ))
            })?;
            let base_url = config.providers.anthropic.base_url.clone();
            let provider = opi_ai::anthropic::AnthropicProvider::new(api_key, base_url);
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openai" => {
            let api_key_env = &config.providers.openai.api_key_env;
            let api_key = std::env::var(api_key_env).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {api_key_env} environment variable"
                ))
            })?;
            let base_url = config.providers.openai.base_url.clone();
            let provider = opi_ai::openai_chat::OpenAiChatProvider::new(api_key, base_url);
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openrouter" => {
            let api_key_env = &config.providers.openrouter.api_key_env;
            let api_key = std::env::var(api_key_env).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {api_key_env} environment variable"
                ))
            })?;
            let base_url = config.providers.openrouter.base_url.clone();
            let referer = config.providers.openrouter.referer.clone();
            let mut extra_headers = Vec::new();
            if let Some(ref_str) = referer {
                extra_headers.push(("HTTP-Referer".into(), ref_str));
            }
            extra_headers.push(("X-Title".into(), "opi".into()));
            let compat = opi_ai::openai_chat::CompatConfig::default();
            let provider = opi_ai::openai_chat::OpenAiChatProvider::new_for_profile(
                api_key,
                base_url.unwrap_or_else(|| "https://openrouter.ai/api/v1".into()),
                "openrouter".into(),
                compat,
                extra_headers,
                vec![],
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "mistral" => {
            let api_key_env = &config.providers.mistral.api_key_env;
            let api_key = std::env::var(api_key_env).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {api_key_env} environment variable"
                ))
            })?;
            let base_url = config.providers.mistral.base_url.clone();
            let compat = opi_ai::openai_chat::CompatConfig::default();
            let provider = opi_ai::openai_chat::OpenAiChatProvider::new_for_profile(
                api_key,
                base_url.unwrap_or_else(|| "https://api.mistral.ai/v1".into()),
                "mistral".into(),
                compat,
                vec![],
                vec![],
            );
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "openai-responses" => {
            let api_key_env = &config.providers.openai_responses.api_key_env;
            let api_key = std::env::var(api_key_env).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {api_key_env} environment variable"
                ))
            })?;
            let base_url = config.providers.openai_responses.base_url.clone();
            let provider = opi_ai::openai_responses::OpenAiResponsesProvider::new(api_key, base_url);
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        "gemini" => {
            let api_key_env = &config.providers.gemini.api_key_env;
            let api_key = std::env::var(api_key_env).map_err(|_| {
                ProviderBuildError::Auth(format!(
                    "missing API key: set {api_key_env} environment variable"
                ))
            })?;
            let base_url = config.providers.gemini.base_url.clone();
            let provider = opi_ai::gemini::GeminiProvider::new(api_key, base_url);
            Ok(Box::new(provider) as Box<dyn Provider>)
        }
        other => Err(ProviderBuildError::Config(format!(
            "unknown provider: {other}"
        ))),
    }
}
```

- [ ] **Step 5: Run clippy and tests**

Run: `cargo clippy -p opi-coding-agent --all-targets -- -D warnings`
Run: `cargo test -p opi-coding-agent --test provider_factory`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/opi-coding-agent/src/main.rs crates/opi-coding-agent/src/config.rs crates/opi-coding-agent/tests/provider_factory.rs
git commit -m "feat(opi-coding-agent): extend provider factory for all 6 providers"
```

---

## Task 4: Thinking Config Passthrough

**Files:**
- Modify: `crates/opi-agent/src/loop_types.rs:46-56` (AgentLoopConfig)
- Modify: `crates/opi-agent/src/lib.rs:94-105` (Request construction), `lib.rs:433-473` (process_stream_event)
- Modify: `crates/opi-coding-agent/src/harness.rs:63-67` (config mapping)
- Create: `crates/opi-agent/tests/thinking_integration.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/opi-agent/tests/thinking_integration.rs`:

```rust
//! Integration test for thinking config passthrough through the agent loop.

use std::sync::{Arc, Mutex};
use opi_agent::loop_types::{AgentLoopConfig, AgentLoopContext};
use opi_agent::hooks::AgentHooks;
use opi_agent::message::AgentMessage;
use opi_agent::{agent_loop, AgentError};
use opi_ai::message::{Message, UserMessage};
use opi_ai::provider::{Provider, Request, EventStream, ProviderError, ThinkingConfig};
use opi_ai::stream::{AssistantStreamEvent, Usage};
use opi_ai::message::{AssistantMessage, AssistantContent};

struct ThinkingCaptureProvider {
    requests: Arc<Mutex<Vec<Request>>>,
}

impl ThinkingCaptureProvider {
    fn new(capture: Arc<Mutex<Vec<Request>>>) -> Self {
        Self { requests: capture }
    }
}

impl Provider for ThinkingCaptureProvider {
    fn stream(&self, request: Request) -> EventStream {
        self.requests.lock().unwrap().push(request);
        let msg = AssistantMessage {
            content: vec![AssistantContent::Text { text: "done".into() }],
            usage: Some(Usage::default()),
            ..Default::default()
        };
        let events: Vec<Result<AssistantStreamEvent, ProviderError>> = vec![
            Ok(AssistantStreamEvent::Done {
                reason: opi_ai::stream::StopReason::Stop,
                message: msg,
            }),
        ];
        Box::pin(futures_util::stream::iter(events))
    }

    fn id(&self) -> &str { "test" }

    fn models(&self) -> &[opi_ai::provider::ModelInfo] { &[] }
}

struct PassthroughHooks;
impl AgentHooks for PassthroughHooks {}

#[tokio::test]
async fn thinking_config_passed_to_provider_request() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let provider = ThinkingCaptureProvider::new(captured.clone());

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![],
        messages: vec![AgentMessage::Llm(Message::User(UserMessage { content: "hi".into() }))],
        model: "test-model".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: None,
    };

    let config = AgentLoopConfig {
        max_turns: 1,
        max_tokens: None,
        temperature: None,
        retry: None,
        thinking: Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(10_000),
        }),
    };

    let (tx, _rx) = tokio::sync::mpsc::channel(64);
    let cancel = tokio_util::sync::CancellationToken::new();
    let hooks = PassthroughHooks;

    let _ = agent_loop(context, config, &hooks, tx, cancel).await;

    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert!(req.thinking.enabled, "thinking should be enabled");
    assert_eq!(req.thinking.budget_tokens, Some(10_000));
}

#[tokio::test]
async fn thinking_disabled_by_default() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let provider = ThinkingCaptureProvider::new(captured.clone());

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![],
        messages: vec![AgentMessage::Llm(Message::User(UserMessage { content: "hi".into() }))],
        model: "test-model".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: None,
    };

    let config = AgentLoopConfig {
        max_turns: 1,
        ..Default::default()
    };

    let (tx, _rx) = tokio::sync::mpsc::channel(64);
    let cancel = tokio_util::sync::CancellationToken::new();
    let hooks = PassthroughHooks;

    let _ = agent_loop(context, config, &hooks, tx, cancel).await;

    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].thinking.enabled, "thinking should be disabled by default");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p opi-agent --test thinking_integration -- thinking_config_passed`
Expected: FAIL (AgentLoopConfig has no `thinking` field)

- [ ] **Step 3: Add thinking field to AgentLoopConfig**

In `crates/opi-agent/src/loop_types.rs`, add the field:

```rust
use opi_ai::provider::ThinkingConfig;

/// Configuration for the agent loop.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Maximum number of turns before stopping.
    pub max_turns: u32,
    /// Maximum output tokens per request.
    pub max_tokens: Option<u64>,
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Retry configuration for retryable provider errors.
    pub retry: Option<opi_ai::retry::RetryConfig>,
    /// Thinking/reasoning configuration.
    pub thinking: Option<ThinkingConfig>,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_turns: 50,
            max_tokens: None,
            temperature: None,
            retry: None,
            thinking: None,
        }
    }
}
```

- [ ] **Step 4: Pass thinking config to Request in agent loop**

In `crates/opi-agent/src/lib.rs`, change line 101 from:
```rust
thinking: Default::default(),
```
to:
```rust
thinking: config.thinking.clone().unwrap_or_default(),
```

- [ ] **Step 5: Fix process_stream_event to preserve thinking events**

In `crates/opi-agent/src/lib.rs`, replace the `_ => None` catch-all (line 473) with explicit thinking handling:

```rust
        ThinkingStart { partial, .. } => {
            let msg = AgentMessage::Llm(Message::Assistant(partial.clone()));
            events(AgentEvent::MessageUpdate {
                message: msg,
                assistant_event: Box::new(event.clone()),
            });
            None
        }
        ThinkingDelta { partial, .. } => {
            let msg = AgentMessage::Llm(Message::Assistant(partial.clone()));
            events(AgentEvent::MessageUpdate {
                message: msg,
                assistant_event: Box::new(event.clone()),
            });
            None
        }
        ThinkingEnd { partial, .. } => {
            let msg = AgentMessage::Llm(Message::Assistant(partial.clone()));
            events(AgentEvent::MessageUpdate {
                message: msg,
                assistant_event: Box::new(event.clone()),
            });
            None
        }
        _ => None,
```

- [ ] **Step 6: Map thinking config in harness**

In `crates/opi-coding-agent/src/harness.rs`, change the `AgentLoopConfig` construction (lines 63-67):

```rust
let agent_config = AgentLoopConfig {
    max_turns: config.defaults.max_iterations,
    retry: Some(config.retry.clone()),
    thinking: if config.thinking.enabled {
        Some(opi_ai::provider::ThinkingConfig {
            enabled: true,
            budget_tokens: Some(config.thinking.budget_tokens as u64),
        })
    } else {
        None
    },
    ..Default::default()
};
```

- [ ] **Step 7: Run all tests**

Run: `cargo test -p opi-agent --test thinking_integration`
Expected: All PASS

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/opi-agent/src/loop_types.rs crates/opi-agent/src/lib.rs crates/opi-coding-agent/src/harness.rs crates/opi-agent/tests/thinking_integration.rs
git commit -m "feat(opi-agent): pass thinking config through to provider requests"
```

---

## Task 5: SessionCoordinator and Session Runtime Wiring

**Files:**
- Create: `crates/opi-coding-agent/src/session_coordinator.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs` (add coordinator)
- Modify: `crates/opi-coding-agent/src/lib.rs` (add module)
- Modify: `crates/opi-coding-agent/src/session_cli.rs` (real resume)
- Modify: `crates/opi-coding-agent/src/main.rs` (resume flow)
- Create: `crates/opi-coding-agent/tests/session_runtime.rs`
- Create: `crates/opi-coding-agent/tests/resume_integration.rs`

- [ ] **Step 1: Write session coordinator test**

Create `crates/opi-coding-agent/tests/session_runtime.rs`:

```rust
//! Integration tests for session runtime persistence.

use std::path::PathBuf;
use tempfile::tempdir;
use opi_agent::session::{SessionHeader, SessionWriter, SessionReader};
use opi_agent::message::AgentMessage;
use opi_ai::message::{Message, UserMessage, AssistantMessage, AssistantContent};

#[test]
fn session_writer_creates_and_appends() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test-session.jsonl");

    let header = SessionHeader::new(
        "test-id".into(),
        "2026-05-23T12:00:00Z".into(),
        "/tmp".into(),
        None,
    );

    let mut writer = SessionWriter::create(&path, header).unwrap();

    // Simulate a user turn
    let user_entry = opi_agent::session::SessionEntry::Message(
        opi_agent::session::MessageEntry {
            id: "msg-1".into(),
            parent_id: None,
            timestamp: "2026-05-23T12:00:01Z".into(),
            message: Message::User(UserMessage { content: "hello".into() }),
        }
    );
    writer.append(&user_entry).unwrap();

    // Simulate an assistant turn
    let assistant_entry = opi_agent::session::SessionEntry::Message(
        opi_agent::session::MessageEntry {
            id: "msg-2".into(),
            parent_id: Some("msg-1".into()),
            timestamp: "2026-05-23T12:00:02Z".into(),
            message: Message::Assistant(AssistantMessage {
                content: vec![AssistantContent::Text { text: "Hi there!".into() }],
                ..Default::default()
            }),
        }
    );
    writer.append(&assistant_entry).unwrap();

    drop(writer);

    // Read back and verify
    let (read_header, entries) = SessionReader::read_all(&path).unwrap();
    assert_eq!(read_header.id, "test-id");
    assert_eq!(entries.len(), 2);
}
```

- [ ] **Step 2: Run test to verify it passes (session storage already works)**

Run: `cargo test -p opi-coding-agent --test session_runtime`
Expected: PASS

- [ ] **Step 3: Create SessionCoordinator**

Create `crates/opi-coding-agent/src/session_coordinator.rs`:

```rust
//! Session lifecycle coordinator.
//!
//! Bridges CodingHarness, SessionWriter, CompactionEngine, and
//! UsageAccumulator into a single coordination point.

use std::path::Path;

use opi_agent::compaction::{CompactionConfig, CompactionEngine};
use opi_agent::message::AgentMessage;
use opi_agent::session::{SessionEntry, SessionHeader, SessionWriter};
use opi_ai::stream::{CumulativeUsage, Usage};

pub struct SessionCoordinator {
    writer: SessionWriter,
    compaction: CompactionEngine,
    usage: CumulativeUsage,
    session_id: String,
}

impl SessionCoordinator {
    pub fn new(
        dir: &Path,
        cwd: &str,
        compaction_config: CompactionConfig,
    ) -> std::io::Result<Self> {
        let id = uuid_session_id();
        let timestamp = chrono_now();
        let header = SessionHeader::new(
            id.clone(),
            timestamp,
            cwd.into(),
            None,
        );
        let path = dir.join(format!("{id}.jsonl"));
        std::fs::create_dir_all(dir)?;
        let writer = SessionWriter::create(&path, header)?;
        Ok(Self {
            writer,
            compaction: CompactionEngine::new(compaction_config),
            usage: CumulativeUsage::default(),
            session_id: id,
        })
    }

    /// Called after each agent turn. Appends message entries to JSONL.
    pub fn on_turn_end(&mut self, messages: &[AgentMessage], usage: &Usage) {
        self.usage.accumulate(usage);
        for msg in messages {
            if let AgentMessage::Llm(m) = msg {
                let entry = SessionEntry::Message(opi_agent::session::MessageEntry {
                    id: format!("msg-{}", self.usage.turn_count()),
                    parent_id: None,
                    timestamp: chrono_now(),
                    message: m.clone(),
                });
                let _ = self.writer.append(&entry);
            }
        }
    }

    /// Called after compaction completes.
    pub fn on_compaction(&mut self, output: &opi_agent::compaction::CompactionOutput) {
        let entry = SessionEntry::Compaction(opi_agent::session::CompactionEntry {
            id: format!("compaction-{}", self.usage.turn_count()),
            parent_id: None,
            timestamp: chrono_now(),
            summary: output.summary_text.clone(),
            first_kept_entry_id: output.first_kept_entry_id.clone(),
            tokens_before: output.tokens_before,
            tokens_after: output.tokens_after,
        });
        let _ = self.writer.append(&entry);
    }

    /// Flush pending entries.
    pub fn flush(&mut self) -> std::io::Result<()> {
        // SessionWriter::append already flushes per write, this is a
        // placeholder for any future buffered writes.
        Ok(())
    }

    pub fn usage(&self) -> &CumulativeUsage {
        &self.usage
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn should_compact(&self) -> bool {
        self.compaction.should_compact(
            self.usage.total_input_tokens(),
            opi_agent::compaction::CompactionReason::Threshold,
        )
    }
}

fn uuid_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{ts:x}")
}

fn chrono_now() -> String {
    // Use a simple ISO-ish timestamp without depending on chrono
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_timestamp(secs)
}

fn format_timestamp(secs: u64) -> String {
    // Simple UTC timestamp formatter
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year/month/day from days since epoch
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month + 1, days + 1)
}

fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
```

- [ ] **Step 4: Add module to lib.rs**

In `crates/opi-coding-agent/src/lib.rs`, add:
```rust
pub mod session_coordinator;
```

- [ ] **Step 5: Wire coordinator into harness**

In `crates/opi-coding-agent/src/harness.rs`, add `session` field and wire it:

```rust
use crate::session_coordinator::SessionCoordinator;

pub struct CodingHarness {
    agent: Agent,
    config: OpiConfig,
    system_prompt: String,
    session: Option<SessionCoordinator>,
}
```

In `new()`, create the coordinator:
```rust
let session = SessionCoordinator::new(
    &crate::session_cli::session_dir(),
    &std::env::current_dir().unwrap_or_default().to_string_lossy(),
    opi_agent::compaction::CompactionConfig::default(),
).ok();

Self {
    agent,
    config,
    system_prompt,
    session,
}
```

In `prompt()`, after the agent returns:
```rust
pub async fn prompt(&mut self, text: &str) -> Result<Vec<AgentMessage>, AgentError> {
    let messages = self.agent.prompt(text).await?;
    if let Some(session) = &mut self.session {
        let usage = messages.iter()
            .filter_map(|m| if let AgentMessage::Llm(Message::Assistant(a)) = m { a.usage.as_ref() } else { None })
            .last()
            .cloned()
            .unwrap_or_default();
        session.on_turn_end(&messages, &usage);
    }
    Ok(messages)
}
```

Same for `continue_()`.

- [ ] **Step 6: Implement real resume**

In `crates/opi-coding-agent/src/session_cli.rs`, add a new function:

```rust
/// Reconstruct agent messages from session entries for resume.
pub fn reconstruct_context(entries: &[opi_agent::session::SessionEntry]) -> Vec<opi_agent::message::AgentMessage> {
    entries.iter()
        .filter_map(|entry| match entry {
            opi_agent::session::SessionEntry::Message(msg_entry) => {
                Some(opi_agent::message::AgentMessage::Llm(msg_entry.message.clone()))
            }
            _ => None,
        })
        .collect()
}
```

Update `handle_session_cli` to return the resumed session data instead of just printing. Change the return type or add a new function that returns `Option<ResumedSession>`.

In `crates/opi-coding-agent/src/main.rs`, change the resume path to:
1. Read the session data
2. Reconstruct agent messages
3. Inject into `CodingHarness` initial messages
4. Enter interactive/non-interactive flow normally

This requires passing `initial_messages: Vec<AgentMessage>` to the harness constructor.

- [ ] **Step 7: Run tests**

Run: `cargo test -p opi-coding-agent --test session_runtime`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/opi-coding-agent/src/session_coordinator.rs crates/opi-coding-agent/src/harness.rs crates/opi-coding-agent/src/lib.rs crates/opi-coding-agent/src/session_cli.rs crates/opi-coding-agent/src/main.rs crates/opi-coding-agent/tests/session_runtime.rs
git commit -m "feat(opi-coding-agent): wire session persistence into harness runtime"
```

---

## Task 6: Compaction Runtime Wiring

**Files:**
- Modify: `crates/opi-coding-agent/src/config.rs` (add CompactionConfig)
- Modify: `crates/opi-coding-agent/src/session_coordinator.rs` (compaction trigger)
- Modify: `crates/opi-agent/src/lib.rs` (fix CompactionSummary filtering in convert_to_llm)
- Create: `crates/opi-coding-agent/tests/compaction_runtime.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/opi-coding-agent/tests/compaction_runtime.rs`:

```rust
//! Compaction runtime integration tests.

use opi_agent::compaction::{CompactionConfig, CompactionEngine, CompactionReason};
use opi_agent::message::AgentMessage;
use opi_ai::message::{Message, UserMessage};

#[test]
fn compaction_triggers_at_threshold() {
    let config = CompactionConfig {
        enabled: true,
        threshold_tokens: 100,
    };
    let engine = CompactionEngine::new(config);
    assert!(engine.should_compact(150, CompactionReason::Threshold));
    assert!(!engine.should_compact(50, CompactionReason::Threshold));
}

#[test]
fn compaction_disabled() {
    let config = CompactionConfig {
        enabled: false,
        threshold_tokens: 100,
    };
    let engine = CompactionEngine::new(config);
    assert!(!engine.should_compact(150, CompactionReason::Threshold));
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p opi-coding-agent --test compaction_runtime`
Expected: PASS (CompactionEngine already works)

- [ ] **Step 3: Add [compaction] config section**

In `crates/opi-coding-agent/src/config.rs`, add:

```rust
/// `[compaction]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactionConfigSection {
    pub enabled: bool,
    pub threshold_tokens: u64,
}

impl Default for CompactionConfigSection {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_tokens: 100_000,
        }
    }
}
```

Add to `OpiConfig`:
```rust
pub compaction: CompactionConfigSection,
```

Add TOML deserialization and merge logic following the existing pattern.

- [ ] **Step 4: Fix CompactionSummary filtering**

In `crates/opi-coding-agent/src/harness.rs`, both `CodingAgentHooks::convert_to_llm` and `InteractiveCodingHooks::convert_to_llm` only pass through `AgentMessage::Llm`. Add `CompactionSummary` conversion:

```rust
fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
    let mut result = Vec::new();
    for msg in messages {
        match msg {
            AgentMessage::Llm(m) => {
                result.push(m.clone());
            }
            AgentMessage::CompactionSummary(summary) => {
                result.push(Message::User(opi_ai::message::UserMessage {
                    content: format!(
                        "[Context was compacted. Summary of earlier conversation: {}]",
                        summary.summary
                    ),
                }));
            }
            _ => {}
        }
    }
    Ok(result)
}
```

- [ ] **Step 5: Wire compaction into SessionCoordinator**

In `session_coordinator.rs`, update `on_turn_end` to check compaction:

```rust
pub fn on_turn_end(&mut self, messages: &[AgentMessage], usage: &Usage) -> Option<opi_agent::compaction::CompactionOutput> {
    self.usage.accumulate(usage);
    // Write entries
    for msg in messages {
        if let AgentMessage::Llm(m) = msg {
            let entry = SessionEntry::Message(opi_agent::session::MessageEntry {
                id: format!("msg-{}", self.usage.turn_count()),
                parent_id: None,
                timestamp: chrono_now(),
                message: m.clone(),
            });
            let _ = self.writer.append(&entry);
        }
    }
    // Check compaction
    if self.compaction.should_compact(self.usage.total_input_tokens(), CompactionReason::Threshold) {
        let entries: Vec<_> = messages.iter().enumerate().map(|(i, m)| {
            opi_agent::compaction::Entry {
                id: format!("e-{i}"),
                message: m.clone(),
            }
        }).collect();
        let hooks = opi_agent::compaction::DefaultCompactionHooks;
        if let Ok(output) = self.compaction.compact(&entries, CompactionReason::Threshold, &hooks) {
            self.on_compaction(&output);
            return Some(output);
        }
    }
    None
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p opi-coding-agent --test compaction_runtime`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/opi-coding-agent/src/config.rs crates/opi-coding-agent/src/session_coordinator.rs crates/opi-coding-agent/src/harness.rs crates/opi-coding-agent/tests/compaction_runtime.rs
git commit -m "feat(opi-coding-agent): wire compaction into session coordinator"
```

---

## Task 7: Usage/Cost Accumulation and StatusBar Wiring

**Files:**
- Modify: `crates/opi-coding-agent/src/interactive.rs:86-106` (event handler)
- Create: `crates/opi-coding-agent/tests/usage_accumulation.rs`

- [ ] **Step 1: Write the test**

Create `crates/opi-coding-agent/tests/usage_accumulation.rs`:

```rust
//! Tests for usage accumulation.

use opi_ai::stream::{Usage, CumulativeUsage};

#[test]
fn cumulative_usage_accumulates() {
    let mut cu = CumulativeUsage::default();
    cu.accumulate(&Usage {
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    });
    assert_eq!(cu.total_input_tokens(), 100);
    assert_eq!(cu.total_output_tokens(), 50);
    assert_eq!(cu.turn_count(), 1);

    cu.accumulate(&Usage {
        input_tokens: 200,
        output_tokens: 75,
        cache_read_tokens: 10,
        cache_write_tokens: 5,
    });
    assert_eq!(cu.total_input_tokens(), 300);
    assert_eq!(cu.total_output_tokens(), 125);
    assert_eq!(cu.turn_count(), 2);
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p opi-coding-agent --test usage_accumulation`
Expected: PASS (CumulativeUsage already works)

- [ ] **Step 3: Wire usage to StatusBar in interactive.rs**

In `crates/opi-coding-agent/src/interactive.rs`, add a `total_tokens` field to `TuiState`:

```rust
struct TuiState {
    // ... existing fields ...
    total_tokens: u64,
}
```

Initialize it in the state constructor (line 52):
```rust
total_tokens: 0,
```

In the `MessageEnd` handler (around line 86), accumulate usage:
```rust
AgentEvent::MessageEnd {
    message: AgentMessage::Llm(Message::Assistant(a)),
} => {
    if let Some(usage) = &a.usage {
        s.total_tokens += usage.input_tokens as u64
            + usage.output_tokens as u64
            + usage.cache_read_tokens as u64
            + usage.cache_write_tokens as u64;
    }
    // ... existing content handling ...
}
```

In `build_shell()`, pass token count:
```rust
fn build_shell(s: &TuiState) -> Shell {
    let mut shell = Shell::new(s.model.clone())
        .input_text(s.input_text.clone())
        .state(s.app_state)
        .theme(s.theme.clone());

    if s.total_tokens > 0 {
        shell = shell.token_count(s.total_tokens);
    }

    // ... rest of builder ...
}
```

- [ ] **Step 4: Run tests and clippy**

Run: `cargo clippy -p opi-coding-agent --all-targets -- -D warnings`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/opi-coding-agent/src/interactive.rs crates/opi-coding-agent/tests/usage_accumulation.rs
git commit -m "feat(opi-coding-agent): wire usage accumulation to TUI status bar"
```

---

## Task 8: DiffView Runtime Wiring

**Files:**
- Modify: `crates/opi-coding-agent/src/tools/edit.rs` (capture before/after)
- Modify: `crates/opi-coding-agent/src/interactive.rs` (render diff)
- Create: `crates/opi-coding-agent/tests/diff_view_integration.rs`

- [ ] **Step 1: Check current edit tool implementation**

Read `crates/opi-coding-agent/src/tools/edit.rs` to understand the current structure. The edit tool needs to capture the file content before and after the edit, then expose it through the tool result.

- [ ] **Step 2: Add before/after to edit tool result**

In the edit tool's `execute()` method, before applying the edit, read the current file content. After applying, read the new content. Include both in the tool result metadata.

Add a `DiffPayload` to carry this data:
```rust
pub struct DiffPayload {
    pub path: String,
    pub before: String,
    pub after: String,
}
```

Include it in the tool result's metadata field so the interactive handler can extract it.

- [ ] **Step 3: Render DiffView in interactive handler**

In `crates/opi-coding-agent/src/interactive.rs`, in the `ToolExecutionEnd` handler, check if the tool is "edit" and if diff data is available. If so, create a system message with a diff indicator.

This is a lightweight integration -- the full DiffView rendering in the message list would require extending `TuiMessage` to support diff content, which can be done as:
```rust
// In the ToolExecutionEnd handler:
AgentEvent::ToolExecutionEnd {
    tool_name,
    is_error: false,
    ..
} if tool_name == "edit" => {
    // The tool result will contain before/after data
    // For now, add a placeholder diff message
    s.messages.push(TuiMessage::new(
        TuiRole::Tool,
        "(file edited - diff view coming in full integration)",
    ));
}
```

A complete DiffView integration requires extending the TUI message types to carry diff content. This step wires the basic path; full rendering is iterative.

- [ ] **Step 4: Run tests and commit**

Run: `cargo clippy -p opi-coding-agent --all-targets -- -D warnings`
Expected: PASS

```bash
git add crates/opi-coding-agent/src/tools/edit.rs crates/opi-coding-agent/src/interactive.rs crates/opi-coding-agent/tests/diff_view_integration.rs
git commit -m "feat(opi-coding-agent): wire DiffView into edit tool result path"
```

---

## Task 9: Provider Lifecycle Tests (Test Gap Closure)

**Files:**
- Create: `crates/opi-ai/tests/openai_chat_lifecycle.rs`
- Create: `crates/opi-ai/tests/openrouter_lifecycle.rs`
- Create: `crates/opi-ai/tests/mistral_lifecycle.rs`

- [ ] **Step 1: Write OpenAI Chat lifecycle test**

Create `crates/opi-ai/tests/openai_chat_lifecycle.rs` following the same pattern as `openai_responses_lifecycle.rs` but using the `/v1/chat/completions` endpoint and `OpenAiChatProvider`.

Include tests for:
- `stream_success` -- successful streaming
- `stream_auth_error` -- 401 response
- `stream_rate_limited` -- 429 with retry-after
- `stream_server_error` -- 500 response
- `stream_no_terminal_event` -- incomplete stream

- [ ] **Step 2: Write OpenRouter lifecycle test**

Create `crates/opi-ai/tests/openrouter_lifecycle.rs` using `OpenAiChatProvider::new_for_profile()` with OpenRouter base URL and extra headers.

Include an additional test:
- `stream_sends_extra_headers` -- use wiremock to assert the `HTTP-Referer` and `X-Title` headers are sent

- [ ] **Step 3: Write Mistral lifecycle test**

Create `crates/opi-ai/tests/mistral_lifecycle.rs` using `OpenAiChatProvider::new_for_profile()` with Mistral base URL.

- [ ] **Step 4: Run all lifecycle tests**

Run: `cargo test -p opi-ai --test openai_chat_lifecycle --test openrouter_lifecycle --test mistral_lifecycle`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/opi-ai/tests/openai_chat_lifecycle.rs crates/opi-ai/tests/openrouter_lifecycle.rs crates/opi-ai/tests/mistral_lifecycle.rs
git commit -m "test(opi-ai): add wiremock lifecycle tests for OpenAI Chat, OpenRouter, Mistral"
```

---

## Task 10: Cleanups

**Files:**
- Modify: `crates/opi-ai/src/stream.rs:5-19` (remove legacy StreamEvent)
- Modify: `crates/opi-ai/src/retry.rs:59-84` (add HTTP-date parsing)
- Modify: `.opi-impl-state.json` (fix evidence paths)

- [ ] **Step 1: Remove legacy StreamEvent**

In `crates/opi-ai/src/stream.rs`, verify `StreamEvent` is not referenced anywhere:
```bash
grep -r "StreamEvent" crates/ --include="*.rs" | grep -v "AssistantStreamEvent"
```

If no references exist, remove lines 4-19 (the `StreamEvent` enum and its comment).

- [ ] **Step 2: Add HTTP-date parsing to Retry-After**

In `crates/opi-ai/src/retry.rs`, expand `parse_retry_after()` to handle HTTP-date format in the `Retry-After` header. The current code only parses seconds values.

Add HTTP-date parsing using a simple approach:
```rust
// In parse_retry_after, after the seconds parse attempt fails:
if let Some(val) = headers.get("retry-after")
    && let Ok(s) = val.to_str()
{
    // Try seconds first (existing)
    if let Ok(secs) = s.parse::<f64>() {
        return Some((secs * 1000.0) as u64);
    }
    // Try HTTP-date: "Fri, 23 May 2026 12:00:00 GMT"
    if let Some(delay_ms) = parse_http_date_delay(s) {
        return Some(delay_ms);
    }
}
```

Add `parse_http_date_delay()` that parses the common HTTP-date format and calculates the delay.

- [ ] **Step 3: Fix ledger evidence paths**

In `.opi-impl-state.json`:
- Fix task 2.9 `behavioral_tests` path from `thinking_fixtures.rs` to `anthropic_fixtures.rs`
- Normalize short hashes to 40-char SHA
- Set task 2.13 `last_attempt` to a valid timestamp

- [ ] **Step 4: Run full test suite**

Run: `cargo test --workspace --all-targets`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo fmt --check --all`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/opi-ai/src/stream.rs crates/opi-ai/src/retry.rs .opi-impl-state.json
git commit -m "chore: remove legacy StreamEvent, expand retry-after parsing, fix ledger paths"
```

---

## Task 11: Final Verification

- [ ] **Step 1: Run all workspace gates**

```sh
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Expected: All PASS

- [ ] **Step 2: Verify provider factory**

```sh
cargo run -p opi-coding-agent -- --model openai:gpt-4o --non-interactive "hello"
```
Expected: Exits with auth error (no API key set)

```sh
cargo run -p opi-coding-agent -- --model gemini:gemini-2.0-flash --non-interactive "hello"
```
Expected: Exits with auth error (no API key set)

- [ ] **Step 3: Commit verification**

```bash
git log --oneline -15
```

Verify 11 commits for the hardening pass, all following Conventional Commits format.
