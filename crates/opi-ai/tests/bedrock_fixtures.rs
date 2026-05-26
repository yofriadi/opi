//! Bedrock provider fixture tests (task 3.1).
//!
//! Tests cover: text streaming, tool calls, usage, provider errors, error mapping,
//! model-family routing, credential redaction, and no live AWS calls.

use std::pin::Pin;
use std::sync::Arc;

use futures_core::Stream;
use futures_util::{StreamExt, pin_mut};
use opi_ai::bedrock::BedrockProvider;
use opi_ai::bedrock::event_stream;
use opi_ai::bedrock::sigv4::AwsCredentials;
use opi_ai::http::HttpClient;
use opi_ai::message::{InputContent, Message, ToolDef, UserMessage};
use opi_ai::provider::{Provider, ProviderError, Request};
use opi_ai::stream::{AssistantStreamEvent, StopReason};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_credentials() -> AwsCredentials {
    AwsCredentials {
        access_key_id: "AKIAIOSFODNN7EXAMPLE".into(),
        secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
        session_token: None,
        region: "us-east-1".into(),
    }
}

fn text_stream_request() -> Request {
    Request {
        model: "anthropic.claude-sonnet-4-20250514-v2:0".into(),
        system: Some("You are helpful.".into()),
        messages: vec![Message::User(UserMessage {
            content: vec![InputContent::Text {
                text: "Hello".into(),
            }],
            timestamp_ms: 0,
        })],
        tools: vec![],
        max_tokens: Some(1024),
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: CancellationToken::new(),
    }
}

fn tool_call_request() -> Request {
    Request {
        model: "anthropic.claude-sonnet-4-20250514-v2:0".into(),
        system: None,
        messages: vec![Message::User(UserMessage {
            content: vec![InputContent::Text {
                text: "Read the file".into(),
            }],
            timestamp_ms: 0,
        })],
        tools: vec![ToolDef {
            name: "read".into(),
            description: "Read a file".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"]
            }),
        }],
        max_tokens: Some(1024),
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: CancellationToken::new(),
    }
}

/// Build a Bedrock Converse-Stream response as event-stream bytes.
fn build_bedrock_stream(events: &[(&str, &str)]) -> Vec<u8> {
    let mut buffer = Vec::new();
    for (event_type, json_payload) in events {
        let frame =
            event_stream::build_test_frame(event_type, "application/json", json_payload.as_bytes());
        buffer.extend_from_slice(&frame);
    }
    buffer
}

/// Collect all events from a stream.
async fn collect_events(
    stream: Pin<Box<dyn Stream<Item = Result<AssistantStreamEvent, ProviderError>> + Send>>,
) -> Vec<AssistantStreamEvent> {
    pin_mut!(stream);
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(event) => {
                let is_terminal = event.is_terminal();
                events.push(event);
                if is_terminal {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    events
}

// ---------------------------------------------------------------------------
// Provider construction and metadata
// ---------------------------------------------------------------------------

#[test]
fn provider_id_is_bedrock() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    assert_eq!(provider.id(), "bedrock");
}

#[test]
fn provider_has_models() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    let models = provider.models();
    assert!(
        !models.is_empty(),
        "bedrock provider should list at least one model"
    );
    // Should contain Claude models
    assert!(
        models.iter().any(|m| m.id.contains("claude")),
        "should list Claude models"
    );
}

#[test]
fn models_have_required_fields() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    for model in provider.models() {
        assert!(!model.id.is_empty(), "model id should not be empty");
        assert!(
            !model.display_name.is_empty(),
            "display_name should not be empty"
        );
        assert!(
            model.context_window > 0,
            "context_window should be positive"
        );
        assert!(
            model.max_output_tokens > 0,
            "max_output_tokens should be positive"
        );
    }
}

// ---------------------------------------------------------------------------
// Text streaming from fixture
// ---------------------------------------------------------------------------

#[tokio::test]
async fn text_streaming_from_fixture() {
    let events_data = build_bedrock_stream(&[
        ("messageStart", r#"{"role":"assistant"}"#),
        (
            "contentBlockStart",
            r#"{"start":{"text":{}},"contentBlockIndex":0}"#,
        ),
        (
            "contentBlockDelta",
            r#"{"delta":{"text":"Hello!"},"contentBlockIndex":0}"#,
        ),
        ("contentBlockStop", r#"{"contentBlockIndex":0}"#),
        ("messageStop", r#"{"stopReason":"end_turn"}"#),
        (
            "metadata",
            r#"{"usage":{"inputTokens":10,"outputTokens":5}}"#,
        ),
    ]);

    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));

    let request = text_stream_request();
    let stream = provider.stream_from_fixture(&events_data, request.cancel);
    let events = collect_events(stream).await;

    assert!(!events.is_empty(), "should produce stream events");

    // Should have Start event
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantStreamEvent::Start { .. })),
        "should have Start"
    );

    // Should have text content
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantStreamEvent::TextDelta { .. })),
        "should have TextDelta"
    );

    // Should end with Done
    let last = events.last().expect("should have events");
    assert!(
        matches!(last, AssistantStreamEvent::Done { .. }),
        "should end with Done"
    );
}

// ---------------------------------------------------------------------------
// Tool call from fixture
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tool_call_from_fixture() {
    let events_data = build_bedrock_stream(&[
        ("messageStart", r#"{"role":"assistant"}"#),
        (
            "contentBlockStart",
            r#"{"start":{"toolUse":{"toolUseId":"tool-1","name":"read"}},"contentBlockIndex":0}"#,
        ),
        (
            "contentBlockDelta",
            r#"{"delta":{"toolUse":{"input":"{\"path\":"}},"contentBlockIndex":0}"#,
        ),
        (
            "contentBlockDelta",
            r#"{"delta":{"toolUse":{"input":"\"/tmp/f\"}"}},"contentBlockIndex":0}"#,
        ),
        ("contentBlockStop", r#"{"contentBlockIndex":0}"#),
        ("messageStop", r#"{"stopReason":"tool_use"}"#),
        (
            "metadata",
            r#"{"usage":{"inputTokens":15,"outputTokens":20}}"#,
        ),
    ]);

    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));

    let request = tool_call_request();
    let stream = provider.stream_from_fixture(&events_data, request.cancel);
    let events = collect_events(stream).await;

    // Should have tool call events
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantStreamEvent::ToolCallStart { .. })),
        "should have ToolCallStart"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantStreamEvent::ToolCallDelta { .. })),
        "should have ToolCallDelta"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantStreamEvent::ToolCallEnd { .. })),
        "should have ToolCallEnd"
    );

    // Done should have ToolUse stop reason
    if let Some(AssistantStreamEvent::Done { reason, .. }) = events.last() {
        assert_eq!(*reason, StopReason::ToolUse);
    } else {
        panic!("expected Done event with ToolUse reason");
    }
}

// ---------------------------------------------------------------------------
// Usage tracking
// ---------------------------------------------------------------------------

#[tokio::test]
async fn usage_tracked_from_metadata() {
    let events_data = build_bedrock_stream(&[
        ("messageStart", r#"{"role":"assistant"}"#),
        (
            "contentBlockStart",
            r#"{"start":{"text":{}},"contentBlockIndex":0}"#,
        ),
        ("contentBlockDelta", r#"{"delta":{"text":"hi"}}"#),
        ("contentBlockStop", r#"{}"#),
        ("messageStop", r#"{"stopReason":"end_turn"}"#),
        (
            "metadata",
            r#"{"usage":{"inputTokens":100,"outputTokens":50,"cacheReadInputTokens":10}}"#,
        ),
    ]);

    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));

    let request = text_stream_request();
    let stream = provider.stream_from_fixture(&events_data, request.cancel);
    let events = collect_events(stream).await;

    if let Some(AssistantStreamEvent::Done { message, .. }) = events.last() {
        assert_eq!(message.usage.input_tokens, 100);
        assert_eq!(message.usage.output_tokens, 50);
        assert_eq!(message.usage.cache_read_tokens, 10);
    } else {
        panic!("expected Done event with usage");
    }
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn access_denied_mapped_to_auth_failed() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    let error = provider.map_bedrock_status(403, "Access denied");
    assert!(matches!(error, ProviderError::AuthFailed(_)));
}

#[tokio::test]
async fn throttling_mapped_to_rate_limited() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    let error = provider.map_bedrock_status(429, "Too many requests");
    assert!(matches!(error, ProviderError::RateLimited { .. }));
}

#[tokio::test]
async fn timeout_mapped_correctly() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    let error = provider.map_bedrock_status(504, "Gateway timeout");
    assert!(matches!(error, ProviderError::Timeout));
}

#[tokio::test]
async fn server_error_mapped_to_request_failed() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    let error = provider.map_bedrock_status(500, "Internal error");
    assert!(matches!(error, ProviderError::RequestFailed(_)));
}

// ---------------------------------------------------------------------------
// Model-family routing
// ---------------------------------------------------------------------------

#[test]
fn supported_model_families() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    let families = provider.supported_model_families();
    assert!(
        families.contains(&"anthropic"),
        "should support anthropic family"
    );
}

#[test]
fn unsupported_model_family_returns_error() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    let result = provider.validate_model_id("unknown.family-v1:0");
    assert!(result.is_err(), "unsupported family should return error");
}

#[test]
fn supported_model_family_validates() {
    let provider = BedrockProvider::new(test_credentials(), None, Arc::new(HttpClient::new()));
    let result = provider.validate_model_id("anthropic.claude-sonnet-4-20250514-v2:0");
    assert!(result.is_ok(), "supported family should validate");
}

// ---------------------------------------------------------------------------
// Secret redaction
// ---------------------------------------------------------------------------

#[test]
fn credentials_redacted_in_debug() {
    let creds = AwsCredentials {
        access_key_id: "AKIAIOSFODNN7EXAMPLE".into(),
        secret_access_key: "super-secret-key".into(),
        session_token: Some("secret-token".into()),
        region: "us-east-1".into(),
    };
    let debug_str = format!("{creds:?}");
    assert!(
        !debug_str.contains("super-secret-key"),
        "secret key should not appear in debug output"
    );
    assert!(
        !debug_str.contains("secret-token"),
        "session token should not appear in debug output"
    );
}

#[test]
fn redact_credentials_hides_secrets() {
    let redacted = opi_ai::bedrock::redact_credentials("AKIAIOSFODNN7EXAMPLE", "super-secret-key");
    assert!(!redacted.contains("super-secret-key"));
    assert!(redacted.contains("***"));
}

// ---------------------------------------------------------------------------
// Shared HTTP client reuse
// ---------------------------------------------------------------------------

#[test]
fn bedrock_provider_accepts_shared_client() {
    let client = Arc::new(HttpClient::new());
    let provider = BedrockProvider::new(test_credentials(), None, client.clone());
    assert!(Arc::ptr_eq(&client, provider.http_client()));
}
