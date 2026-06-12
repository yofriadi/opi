use futures_util::StreamExt;
use opi_ai::http::HttpClient;
use opi_ai::provider::Request;
use opi_ai::stream::AssistantStreamEvent;
use opi_ai::{OpenAiCodexProvider, Provider};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_request(cancel: CancellationToken) -> Request {
    Request {
        model: "openai-codex:gpt-5.5".to_string(),
        messages: vec![opi_ai::message::Message::User(
            opi_ai::message::UserMessage {
                content: vec![opi_ai::message::InputContent::Text {
                    text: "Hello".to_string(),
                }],
                timestamp_ms: 0,
            },
        )],
        system: Some("system-prompt".to_string()),
        max_tokens: Some(100),
        temperature: Some(0.7),
        tools: vec![],
        thinking: opi_ai::provider::ThinkingConfig {
            enabled: true,
            budget_tokens: Some(2048),
        },
        stop_sequences: vec![],
        metadata: None,
        cancel,
    }
}

fn text_sse_fixture() -> &'static str {
    "event: response.created\n\
     data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\",\"status\":\"in_progress\",\"model\":\"gpt-5.5\",\"output\":[]}}\n\n\
     event: response.output_item.added\n\
     data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"message\",\"status\":\"in_progress\",\"role\":\"assistant\",\"content\":[]}}\n\n\
     event: response.content_part.added\n\
     data: {\"type\":\"response.content_part.added\",\"output_index\":0,\"content_index\":0,\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\n\
     event: response.output_text.delta\n\
     data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"content_index\":0,\"delta\":\"Hello Codex\"}\n\n\
     event: response.output_text.done\n\
     data: {\"type\":\"response.output_text.done\",\"output_index\":0,\"content_index\":0,\"text\":\"Hello Codex\"}\n\n\
     event: response.output_item.done\n\
     data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"status\":\"completed\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello Codex\"}]}}\n\n\
     event: response.completed\n\
     data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"status\":\"completed\",\"model\":\"gpt-5.5\",\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello Codex\"}]}],\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}}\n\n"
}

#[test]
fn test_provider_identity() {
    let client = Arc::new(HttpClient::new());
    let provider = OpenAiCodexProvider::new(
        "test-token".to_string(),
        "test-account".to_string(),
        "https://chatgpt.com/backend-api".to_string(),
        client.clone(),
    );
    assert_eq!(provider.id(), "openai-codex");
    assert_eq!(provider.models().len(), 4);
    assert_eq!(provider.models()[0].id, "gpt-5.3-codex-spark");

    // Test base_url normalization
    let provider_slash = OpenAiCodexProvider::new(
        "test-token".to_string(),
        "test-account".to_string(),
        "https://chatgpt.com/backend-api/".to_string(),
        client,
    );
    let debug_str = format!("{:?}", provider_slash);
    assert!(debug_str.contains("base_url: \"https://chatgpt.com/backend-api\""));
}

#[test]
fn test_debug_redaction() {
    let client = Arc::new(HttpClient::new());
    let provider = OpenAiCodexProvider::new(
        "secret-access-token".to_string(),
        "test-account".to_string(),
        "https://chatgpt.com/backend-api".to_string(),
        client,
    );
    let debug_str = format!("{:?}", provider);
    assert!(!debug_str.contains("secret-access-token"));
    assert!(debug_str.contains("[REDACTED]"));
    assert!(debug_str.contains("test-account"));
}

#[tokio::test]
async fn test_stream_success_and_headers() {
    let server = MockServer::start().await;

    // We check for exact headers
    Mock::given(method("POST"))
        .and(path("/codex/responses"))
        .and(header("authorization", "Bearer test-token"))
        .and(header("chatgpt-account-id", "test-account"))
        .and(header("originator", "pi"))
        .and(header("openai-beta", "responses=experimental"))
        .and(header("accept", "text/event-stream"))
        .and(header("content-type", "application/json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(text_sse_fixture())
                .insert_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let client = Arc::new(HttpClient::new());
    let provider = OpenAiCodexProvider::new(
        "test-token".to_string(),
        "test-account".to_string(),
        server.uri(),
        client,
    );

    let mut stream = provider.stream(make_request(CancellationToken::new()));
    let mut events = Vec::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(event) => {
                let is_terminal = event.is_terminal();
                events.push(event);
                if is_terminal {
                    break;
                }
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    assert!(
        events
            .iter()
            .any(|e| matches!(e, AssistantStreamEvent::Start { .. })),
        "should have Start event"
    );

    let done = events
        .iter()
        .find(|e| matches!(e, AssistantStreamEvent::Done { .. }))
        .expect("should have Done event");

    if let AssistantStreamEvent::Done { reason, message } = done {
        assert_eq!(*reason, opi_ai::stream::StopReason::Stop);
        let text: String = message
            .content
            .iter()
            .filter_map(|c| match c {
                opi_ai::message::AssistantContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "Hello Codex");
    }

    // Verify headers from the received request
    let received = server.received_requests().await.unwrap();
    assert!(
        !received.is_empty(),
        "MockServer should have received a request"
    );
    let req = &received[0];

    let session_id = req
        .headers
        .get("session-id")
        .expect("missing session-id header")
        .to_str()
        .unwrap();
    let x_client_id = req
        .headers
        .get("x-client-request-id")
        .expect("missing x-client-request-id header")
        .to_str()
        .unwrap();
    assert_eq!(session_id, x_client_id);
    assert!(
        uuid::Uuid::parse_str(session_id).is_ok(),
        "session-id should be a valid UUID"
    );

    let user_agent = req
        .headers
        .get("user-agent")
        .expect("missing user-agent header")
        .to_str()
        .unwrap();
    assert!(
        user_agent.starts_with("opi ("),
        "User-Agent should start with 'opi (', got: {}",
        user_agent
    );
    assert!(
        user_agent.ends_with(')'),
        "User-Agent should end with ')', got: {}",
        user_agent
    );
    assert!(
        user_agent.contains(';'),
        "User-Agent should contain ';', got: {}",
        user_agent
    );

    let expected_release = get_expected_release();
    assert!(
        user_agent.contains(&expected_release),
        "User-Agent should contain the real release version: {}, got: {}",
        expected_release,
        user_agent
    );
}

fn get_expected_release() -> String {
    if cfg!(target_os = "windows") {
        if let Ok(output) = std::process::Command::new("cmd")
            .args(["/c", "ver"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(start) = stdout.find("[Version ") {
                let rest = &stdout[start + 9..];
                if let Some(end) = rest.find(']') {
                    return rest[..end].to_string();
                }
            }
        }
        "10.0.0".to_string()
    } else {
        if let Ok(output) = std::process::Command::new("uname").arg("-r").output() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                return stdout;
            }
        }
        if cfg!(target_os = "macos") {
            "25.5.0".to_string()
        } else {
            "6.1.0".to_string()
        }
    }
}
