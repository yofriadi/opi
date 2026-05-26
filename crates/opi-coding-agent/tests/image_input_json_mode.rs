//! JSON mode image event tests for task 3.4.
//!
//! Validates that image content in user messages serializes correctly through
//! the AgentSessionEvent / AgentEvent NDJSON protocol, preserving media type
//! and source metadata without lossy text coercion.

use opi_agent::event::AgentEvent;
use opi_agent::message::AgentMessage;
use opi_agent::session_event::AgentSessionEvent;
use opi_ai::message::{ImageSource, InputContent, MediaType, Message, UserMessage};

fn image_user_msg() -> Message {
    Message::User(UserMessage {
        content: vec![
            InputContent::Text {
                text: "Describe this image".into(),
            },
            InputContent::Image {
                source: ImageSource::Url {
                    url: "https://example.com/photo.png".into(),
                },
                media_type: MediaType::Png,
            },
        ],
        timestamp_ms: 1000,
    })
}

#[test]
fn agent_event_turn_end_serializes_image_content() {
    let event = AgentEvent::TurnEnd {
        message: AgentMessage::Llm(image_user_msg()),
        tool_results: vec![],
    };
    let json = serde_json::to_string(&event).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(val["type"], "TurnEnd");
    let content = &val["message"]["content"];
    assert!(content.is_array());
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["type"], "url");
    assert_eq!(content[1]["source"]["url"], "https://example.com/photo.png");
    assert_eq!(content[1]["media_type"], "image/png");
}

#[test]
fn agent_event_message_start_serializes_image_content() {
    let event = AgentEvent::MessageStart {
        message: AgentMessage::Llm(image_user_msg()),
    };
    let json = serde_json::to_string(&event).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(val["type"], "MessageStart");
    let content = &val["message"]["content"];
    assert!(content.is_array());
    assert_eq!(content[1]["type"], "image");
}

#[test]
fn session_event_wraps_image_content_in_ndjson() {
    let session_event = AgentSessionEvent::Agent {
        event: AgentEvent::MessageStart {
            message: AgentMessage::Llm(image_user_msg()),
        },
    };
    let json = serde_json::to_string(&session_event).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(val["type"], "Agent");
    assert_eq!(val["event"]["type"], "MessageStart");
    let content = &val["event"]["message"]["content"];
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["media_type"], "image/png");
}

#[test]
fn session_event_roundtrip_preserves_image_bytes() {
    let msg = Message::User(UserMessage {
        content: vec![InputContent::Image {
            source: ImageSource::Bytes {
                data: (0u8..=255).collect(),
            },
            media_type: MediaType::Png,
        }],
        timestamp_ms: 1000,
    });
    let session_event = AgentSessionEvent::Agent {
        event: AgentEvent::TurnEnd {
            message: AgentMessage::Llm(msg),
            tool_results: vec![],
        },
    };

    let json = serde_json::to_string(&session_event).unwrap();
    let back: AgentSessionEvent = serde_json::from_str(&json).unwrap();

    if let AgentSessionEvent::Agent {
        event: AgentEvent::TurnEnd { message, .. },
    } = back
    {
        if let AgentMessage::Llm(Message::User(u)) = message {
            if let InputContent::Image {
                source: ImageSource::Bytes { data },
                ..
            } = &u.content[0]
            {
                assert_eq!(data.len(), 256);
            } else {
                panic!("expected Image with Bytes source");
            }
        } else {
            panic!("expected Llm User message");
        }
    } else {
        panic!("expected Agent TurnEnd event");
    }
}
