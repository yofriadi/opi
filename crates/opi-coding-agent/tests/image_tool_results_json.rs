//! JSON mode image tool result tests for task 3.5.
//!
//! Validates that image content in tool results serializes correctly through
//! the AgentEvent / AgentSessionEvent NDJSON protocol without lossy text coercion.

use opi_agent::event::AgentEvent;
use opi_agent::session_event::AgentSessionEvent;
use opi_ai::message::{ImageSource, MediaType, OutputContent};

fn image_tool_result_content() -> Vec<OutputContent> {
    vec![
        OutputContent::Text {
            text: "Screenshot captured".into(),
        },
        OutputContent::Image {
            source: ImageSource::Base64 {
                data: "iVBORw0KGgo=".into(),
            },
            media_type: MediaType::Png,
        },
    ]
}

// --- ToolExecutionEnd event ---

#[test]
fn tool_execution_end_serializes_image_content() {
    let content = image_tool_result_content();
    let event = AgentEvent::ToolExecutionEnd {
        tool_call_id: "call_1".into(),
        tool_name: "screenshot".into(),
        result: serde_json::json!(&content),
        details: None,
        is_error: false,
    };
    let json = serde_json::to_string(&event).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(val["type"], "ToolExecutionEnd");
    assert_eq!(val["tool_name"], "screenshot");
    let result = &val["result"];
    assert!(result.is_array());
    assert_eq!(result[0]["type"], "text");
    assert_eq!(result[1]["type"], "image");
    assert_eq!(result[1]["source"]["type"], "base64");
    assert_eq!(result[1]["media_type"], "image/png");
}

#[test]
fn tool_execution_end_image_bytes_not_coerced() {
    let content = vec![OutputContent::Image {
        source: ImageSource::Bytes {
            data: vec![0x89, 0x50, 0x4E, 0x47],
        },
        media_type: MediaType::Png,
    }];
    let event = AgentEvent::ToolExecutionEnd {
        tool_call_id: "call_2".into(),
        tool_name: "capture".into(),
        result: serde_json::json!(&content),
        details: None,
        is_error: false,
    };
    let val: serde_json::Value = serde_json::to_value(&event).unwrap();

    let result = &val["result"];
    assert_eq!(result[0]["type"], "image");
    assert_eq!(result[0]["source"]["type"], "bytes");
}

#[test]
fn tool_execution_end_image_url_in_result() {
    let content = vec![OutputContent::Image {
        source: ImageSource::Url {
            url: "https://example.com/screenshot.png".into(),
        },
        media_type: MediaType::Png,
    }];
    let event = AgentEvent::ToolExecutionEnd {
        tool_call_id: "call_3".into(),
        tool_name: "web_capture".into(),
        result: serde_json::json!(&content),
        details: Some(serde_json::json!({"source": "url"})),
        is_error: false,
    };
    let val: serde_json::Value = serde_json::to_value(&event).unwrap();

    let result = &val["result"];
    assert_eq!(result[0]["type"], "image");
    assert_eq!(
        result[0]["source"]["url"],
        "https://example.com/screenshot.png"
    );
}

// --- SessionEvent wrapping ---

#[test]
fn session_event_wraps_tool_execution_end_with_image() {
    let content = image_tool_result_content();
    let event = AgentEvent::ToolExecutionEnd {
        tool_call_id: "call_1".into(),
        tool_name: "screenshot".into(),
        result: serde_json::json!(&content),
        details: None,
        is_error: false,
    };
    let session_event = AgentSessionEvent::Agent { event };
    let json = serde_json::to_string(&session_event).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(val["type"], "Agent");
    assert_eq!(val["event"]["type"], "ToolExecutionEnd");
    let result = &val["event"]["result"];
    assert_eq!(result[1]["type"], "image");
    assert_eq!(result[1]["media_type"], "image/png");
}

// --- Round-trip ---

#[test]
fn session_event_image_roundtrip_preserves_metadata() {
    let content = vec![OutputContent::Image {
        source: ImageSource::Base64 {
            data: "abc123".into(),
        },
        media_type: MediaType::Gif,
    }];
    let event = AgentEvent::ToolExecutionEnd {
        tool_call_id: "call_rt".into(),
        tool_name: "snapshot".into(),
        result: serde_json::json!(&content),
        details: None,
        is_error: false,
    };
    let session_event = AgentSessionEvent::Agent { event };
    let json = serde_json::to_string(&session_event).unwrap();
    let back: AgentSessionEvent = serde_json::from_str(&json).unwrap();

    if let AgentSessionEvent::Agent {
        event: AgentEvent::ToolExecutionEnd { result, .. },
    } = back
    {
        assert_eq!(result[0]["type"], "image");
        assert_eq!(result[0]["source"]["data"], "abc123");
        assert_eq!(result[0]["media_type"], "image/gif");
    } else {
        panic!("expected Agent ToolExecutionEnd event");
    }
}
