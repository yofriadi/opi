//! Agent retry integration tests (task 2.15).
//!
//! Tests that agent_loop retries on retryable errors (RateLimited, Timeout),
//! emits AutoRetryStart/End events, respects max_attempts, and does not
//! retry non-retryable errors (AuthFailed).

use std::sync::{Arc, Mutex};

use opi_agent::agent_loop;
use opi_agent::event::{AgentEvent, AgentEventSink};
use opi_agent::hooks::AgentHooks;
use opi_agent::loop_types::{AgentError, AgentLoopConfig, AgentLoopContext};
use opi_agent::message::AgentMessage;
use opi_ai::message::{InputContent, Message, UserMessage};
use opi_ai::provider::ProviderError;
use opi_ai::retry::RetryConfig;
use opi_ai::test_support::{self, MockProvider, MockResponse};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct NoopHooks;

impl AgentHooks for NoopHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        let mut result = Vec::new();
        for msg in messages {
            if let AgentMessage::Llm(m) = msg {
                result.push(m.clone());
            }
        }
        Ok(result)
    }
}

fn user_msg(text: &str) -> AgentMessage {
    AgentMessage::Llm(Message::User(UserMessage {
        content: vec![InputContent::Text { text: text.into() }],
        timestamp_ms: 0,
    }))
}

fn make_context(provider: MockProvider) -> AgentLoopContext {
    AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![],
        messages: vec![user_msg("hello")],
        model: "mock-model".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: None,
    }
}

fn make_config(retry: Option<RetryConfig>) -> AgentLoopConfig {
    AgentLoopConfig {
        max_turns: 10,
        max_tokens: None,
        temperature: None,
        retry,
    }
}

fn collect_events() -> (Arc<Mutex<Vec<AgentEvent>>>, AgentEventSink) {
    let log = Arc::new(Mutex::new(Vec::<AgentEvent>::new()));
    let l = log.clone();
    let sink = Box::new(move |e: AgentEvent| {
        l.lock().unwrap().push(e);
    }) as AgentEventSink;
    (log, sink)
}

fn fast_retry_config() -> RetryConfig {
    RetryConfig {
        max_attempts: 3,
        initial_delay_ms: 10,
        max_delay_ms: 100,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retry_on_rate_limited_then_succeed() {
    let provider = MockProvider::new_with_errors(
        "mock",
        vec![
            MockResponse::Error(ProviderError::RateLimited {
                retry_after_ms: Some(100),
            }),
            MockResponse::Events(test_support::text_response("success after retry")),
        ],
    );

    let (log, sink) = collect_events();
    let result = agent_loop(
        make_context(provider),
        make_config(Some(fast_retry_config())),
        &NoopHooks,
        sink,
        tokio_util::sync::CancellationToken::new(),
    )
    .await;

    assert!(
        result.is_ok(),
        "should succeed after retry: {:?}",
        result.err()
    );
    let events = log.lock().unwrap().clone();

    let starts: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::AutoRetryStart { .. }))
        .collect();
    assert_eq!(starts.len(), 1, "should have one AutoRetryStart");

    let ends: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::AutoRetryEnd { success: true, .. }))
        .collect();
    assert_eq!(ends.len(), 1, "should have one AutoRetryEnd(success=true)");
}

#[tokio::test]
async fn no_retry_on_auth_error() {
    let provider = MockProvider::new_with_errors(
        "mock",
        vec![MockResponse::Error(ProviderError::AuthFailed(
            "bad key".into(),
        ))],
    );

    let (log, sink) = collect_events();
    let result = agent_loop(
        make_context(provider),
        make_config(Some(fast_retry_config())),
        &NoopHooks,
        sink,
        tokio_util::sync::CancellationToken::new(),
    )
    .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentError::AuthFailed(msg) => assert!(msg.contains("bad key")),
        other => panic!("expected AuthFailed, got {other:?}"),
    }

    let events = log.lock().unwrap().clone();
    let retry_events: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                AgentEvent::AutoRetryStart { .. } | AgentEvent::AutoRetryEnd { .. }
            )
        })
        .collect();
    assert!(
        retry_events.is_empty(),
        "auth error should not trigger retry"
    );
}

#[tokio::test]
async fn retry_exhausted_returns_error() {
    let provider = MockProvider::new_with_errors(
        "mock",
        vec![
            MockResponse::Error(ProviderError::RateLimited {
                retry_after_ms: Some(10),
            }),
            MockResponse::Error(ProviderError::RateLimited {
                retry_after_ms: Some(20),
            }),
            MockResponse::Error(ProviderError::RateLimited {
                retry_after_ms: Some(40),
            }),
        ],
    );

    let config = RetryConfig {
        max_attempts: 2,
        initial_delay_ms: 10,
        max_delay_ms: 100,
    };

    let (log, sink) = collect_events();
    let result = agent_loop(
        make_context(provider),
        make_config(Some(config)),
        &NoopHooks,
        sink,
        tokio_util::sync::CancellationToken::new(),
    )
    .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentError::Provider(msg) => {
            assert!(msg.contains("rate limited"), "got: {msg}");
        }
        other => panic!("expected Provider error, got {other:?}"),
    }

    let events = log.lock().unwrap().clone();
    let starts: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::AutoRetryStart { .. }))
        .collect();
    assert_eq!(starts.len(), 2, "should have 2 AutoRetryStart events");

    let fails: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::AutoRetryEnd { success: false, .. }))
        .collect();
    assert_eq!(fails.len(), 1, "should have 1 AutoRetryEnd(success=false)");
}

#[tokio::test]
async fn retry_on_timeout_then_succeed() {
    let provider = MockProvider::new_with_errors(
        "mock",
        vec![
            MockResponse::Error(ProviderError::Timeout),
            MockResponse::Events(test_support::text_response("after timeout")),
        ],
    );

    let (log, sink) = collect_events();
    let result = agent_loop(
        make_context(provider),
        make_config(Some(fast_retry_config())),
        &NoopHooks,
        sink,
        tokio_util::sync::CancellationToken::new(),
    )
    .await;

    assert!(result.is_ok());
    let events = log.lock().unwrap().clone();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::AutoRetryStart { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::AutoRetryEnd { success: true, .. }))
    );
}

#[tokio::test]
async fn no_retry_when_config_is_none() {
    let provider = MockProvider::new_with_errors(
        "mock",
        vec![MockResponse::Error(ProviderError::RateLimited {
            retry_after_ms: None,
        })],
    );

    let (log, sink) = collect_events();
    let result = agent_loop(
        make_context(provider),
        make_config(None),
        &NoopHooks,
        sink,
        tokio_util::sync::CancellationToken::new(),
    )
    .await;

    assert!(result.is_err());
    let events = log.lock().unwrap().clone();
    let retry_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::AutoRetryStart { .. }))
        .collect();
    assert!(retry_events.is_empty(), "no retry when config is None");
}

#[tokio::test]
async fn retry_auto_retry_start_fields() {
    let provider = MockProvider::new_with_errors(
        "mock",
        vec![
            MockResponse::Error(ProviderError::RateLimited {
                retry_after_ms: Some(5000),
            }),
            MockResponse::Events(test_support::text_response("ok")),
        ],
    );

    let (log, sink) = collect_events();
    let result = agent_loop(
        make_context(provider),
        make_config(Some(fast_retry_config())),
        &NoopHooks,
        sink,
        tokio_util::sync::CancellationToken::new(),
    )
    .await;

    assert!(result.is_ok());
    let events = log.lock().unwrap().clone();
    let start = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::AutoRetryStart {
                attempt,
                max_attempts,
                delay_ms,
                error_message,
            } => Some((*attempt, *max_attempts, *delay_ms, error_message.clone())),
            _ => None,
        })
        .expect("should have AutoRetryStart");

    assert_eq!(start.0, 1, "attempt should be 1 (first retry)");
    assert_eq!(start.1, 3, "max_attempts should be 3");
    assert!(start.2 > 0, "delay_ms should be positive");
    assert!(
        start.3.contains("rate limited"),
        "error_message should describe the error"
    );
}
