//! Behavioral tests for task 1.1: message and stream types.
//!
//! DoD: "serialize where needed; terminal stream events tested"

use opi_ai::{
    ApiKind,
    message::{
        AssistantContent, AssistantMessage, InputContent, Message, OutputContent, ToolCall,
        ToolDef, ToolResultMessage, UserMessage,
    },
    stream::{AssistantStreamEvent, StopReason, Usage},
};

fn sample_assistant_message() -> AssistantMessage {
    AssistantMessage {
        content: vec![AssistantContent::Text {
            text: "hello".into(),
        }],
        api: ApiKind::Anthropic,
        provider: "anthropic".into(),
        model: "claude-sonnet-4-5-20250514".into(),
        response_model: None,
        response_id: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp_ms: 1000,
    }
}

// --- Terminal stream event tests ---

#[test]
fn done_event_is_terminal() {
    let msg = sample_assistant_message();
    let event = AssistantStreamEvent::Done {
        reason: StopReason::Stop,
        message: msg,
    };
    assert!(event.is_terminal());
}

#[test]
fn error_event_is_terminal() {
    let msg = sample_assistant_message();
    let event = AssistantStreamEvent::Error {
        reason: StopReason::Error,
        message: msg,
    };
    assert!(event.is_terminal());
}

#[test]
fn start_event_is_not_terminal() {
    let msg = sample_assistant_message();
    let event = AssistantStreamEvent::Start { partial: msg };
    assert!(!event.is_terminal());
}

#[test]
fn text_delta_is_not_terminal() {
    let msg = sample_assistant_message();
    let event = AssistantStreamEvent::TextDelta {
        content_index: 0,
        delta: "hi".into(),
        partial: msg,
    };
    assert!(!event.is_terminal());
}

// --- Serialization round-trip tests ---

#[test]
fn user_message_round_trips_through_json() {
    let msg = Message::User(UserMessage {
        content: vec![InputContent::Text {
            text: "Hello, world".into(),
        }],
        timestamp_ms: 42,
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn assistant_message_round_trips_through_json() {
    let msg = Message::Assistant(sample_assistant_message());
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn tool_result_message_round_trips_through_json() {
    let msg = Message::ToolResult(ToolResultMessage {
        tool_call_id: "call_123".into(),
        tool_name: "read_file".into(),
        content: vec![OutputContent::Text {
            text: "file contents".into(),
        }],
        details: None,
        is_error: false,
        truncated: false,
        timestamp_ms: 99,
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn tool_result_error_message_round_trips() {
    let msg = Message::ToolResult(ToolResultMessage {
        tool_call_id: "call_err".into(),
        tool_name: "bash".into(),
        content: vec![OutputContent::Text {
            text: "command failed".into(),
        }],
        details: Some(serde_json::json!({"exit_code": 1})),
        is_error: true,
        truncated: false,
        timestamp_ms: 100,
    });
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn done_event_serializes() {
    let msg = sample_assistant_message();
    let event = AssistantStreamEvent::Done {
        reason: StopReason::Stop,
        message: msg,
    };
    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains("\"done\""));
}

#[test]
fn error_event_serializes() {
    let msg = sample_assistant_message();
    let event = AssistantStreamEvent::Error {
        reason: StopReason::Error,
        message: msg,
    };
    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains("\"error\""));
}

// --- Stop reason tests ---

#[test]
fn stop_reasons_match_pi() {
    assert_eq!(StopReason::Stop.as_str(), "stop");
    assert_eq!(StopReason::Length.as_str(), "length");
    assert_eq!(StopReason::ToolUse.as_str(), "tool_use");
    assert_eq!(StopReason::Error.as_str(), "error");
    assert_eq!(StopReason::Aborted.as_str(), "aborted");
}

// --- ToolDef and ToolCall tests ---

#[test]
fn tool_def_round_trips_through_json() {
    let def = ToolDef {
        name: "read_file".into(),
        description: "Read a file".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        }),
    };
    let json = serde_json::to_string(&def).expect("serialize");
    let back: ToolDef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(def, back);
}

#[test]
fn tool_call_in_stream_event() {
    let msg = sample_assistant_message();
    let tc = ToolCall {
        id: "call_abc".into(),
        name: "read_file".into(),
        arguments: "{\"path\":\"/tmp/x\"}".into(),
    };
    let event = AssistantStreamEvent::ToolCallEnd {
        content_index: 0,
        tool_call: tc,
        partial: msg,
    };
    assert!(!event.is_terminal());
    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains("call_abc"));
}

// --- Usage tests ---

#[test]
fn usage_default_is_zero() {
    let u = Usage::default();
    assert_eq!(u.input_tokens, 0);
    assert_eq!(u.output_tokens, 0);
}
