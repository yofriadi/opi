//! Adapter JSONL protocol type tests (task 5.4).
//!
//! Covers: all host-to-adapter and adapter-to-host message types, serde
//! round-trip fidelity, unknown type rejection, and protocol version
//! negotiation constants.

use opi_coding_agent::adapter_protocol::{
    AdapterCommandCapability, AdapterHostMessage, AdapterModelOverride, AdapterProcessMessage,
    AdapterToolCapability, PROTOCOL_VERSION,
};

// ---------------------------------------------------------------------------
// Protocol version constant
// ---------------------------------------------------------------------------

#[test]
fn protocol_version_matches_manifest_constant() {
    assert_eq!(
        PROTOCOL_VERSION, "opi-extension-jsonl-v1",
        "protocol version must match the value validated in package manifests"
    );
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: initialize
// ---------------------------------------------------------------------------

#[test]
fn host_initialize_serializes_and_round_trips() {
    let msg = AdapterHostMessage::Initialize {
        id: "1".into(),
        protocol: PROTOCOL_VERSION.into(),
        package: "todo".into(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(
        json.contains("\"type\":\"initialize\""),
        "must have type tag"
    );
    assert!(json.contains("\"protocol\":\"opi-extension-jsonl-v1\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: tool_call
// ---------------------------------------------------------------------------

#[test]
fn host_tool_call_round_trips() {
    let msg = AdapterHostMessage::ToolCall {
        id: "2".into(),
        tool: "todo_add".into(),
        args: serde_json::json!({"text": "write spec"}),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"tool_call\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: command
// ---------------------------------------------------------------------------

#[test]
fn host_command_round_trips() {
    let msg = AdapterHostMessage::Command {
        id: "3".into(),
        name: "todo/list".into(),
        args: serde_json::json!({}),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"command\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: hook
// ---------------------------------------------------------------------------

#[test]
fn host_hook_round_trips() {
    let msg = AdapterHostMessage::Hook {
        id: "4".into(),
        hook: "before_tool_call".into(),
        payload: serde_json::json!({"tool": "bash", "args": {}}),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"hook\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: event (fire-and-forget)
// ---------------------------------------------------------------------------

#[test]
fn host_event_round_trips() {
    let msg = AdapterHostMessage::Event {
        event: serde_json::json!({"type": "turn_start", "turn": 1}),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"event\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: state_serialize
// ---------------------------------------------------------------------------

#[test]
fn host_state_serialize_round_trips() {
    let msg = AdapterHostMessage::StateSerialize { id: "5".into() };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"state_serialize\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: state_restore
// ---------------------------------------------------------------------------

#[test]
fn host_state_restore_round_trips() {
    let msg = AdapterHostMessage::StateRestore {
        id: "5".into(),
        state: serde_json::json!({"items": ["a", "b"]}),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"state_restore\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: cancel
// ---------------------------------------------------------------------------

#[test]
fn host_cancel_round_trips() {
    let msg = AdapterHostMessage::Cancel {
        id: "2".into(),
        reason: "user_abort".into(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"cancel\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Host -> Adapter messages: shutdown
// ---------------------------------------------------------------------------

#[test]
fn host_shutdown_round_trips() {
    let msg = AdapterHostMessage::Shutdown {
        id: "6".into(),
        reason: "session_shutdown".into(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"shutdown\""));

    let back: AdapterHostMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Adapter -> Host messages: capabilities
// ---------------------------------------------------------------------------

#[test]
fn adapter_capabilities_round_trips() {
    let msg = AdapterProcessMessage::Capabilities {
        id: "1".into(),
        tools: vec![AdapterToolCapability {
            name: "todo_add".into(),
            description: "Add a todo item".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"}
                },
                "required": ["text"]
            }),
        }],
        commands: vec![AdapterCommandCapability {
            name: "todo/list".into(),
            description: "List todo items".into(),
        }],
        hooks: vec!["before_tool_call".into(), "event".into()],
        model_overrides: vec![AdapterModelOverride {
            model: "todo-adapter-model".into(),
            tools: vec!["todo_add".into()],
        }],
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"capabilities\""));

    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn adapter_capabilities_minimal_round_trips() {
    // Minimal capabilities with empty vectors
    let msg = AdapterProcessMessage::Capabilities {
        id: "1".into(),
        tools: vec![],
        commands: vec![],
        hooks: vec![],
        model_overrides: vec![],
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Adapter -> Host messages: tool_result
// ---------------------------------------------------------------------------

#[test]
fn adapter_tool_result_round_trips() {
    let msg = AdapterProcessMessage::ToolResult {
        id: "2".into(),
        content: vec![serde_json::json!({"type": "text", "text": "ok"})],
        is_error: false,
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"tool_result\""));

    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn adapter_tool_result_error_round_trips() {
    let msg = AdapterProcessMessage::ToolResult {
        id: "2".into(),
        content: vec![serde_json::json!({"type": "text", "text": "not found"})],
        is_error: true,
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert!(back.is_error());
}

// ---------------------------------------------------------------------------
// Adapter -> Host messages: command_result
// ---------------------------------------------------------------------------

#[test]
fn adapter_command_result_round_trips() {
    let msg = AdapterProcessMessage::CommandResult {
        id: "3".into(),
        data: serde_json::json!({"items": []}),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"command_result\""));

    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Adapter -> Host messages: hook_result
// ---------------------------------------------------------------------------

#[test]
fn adapter_hook_result_continue_round_trips() {
    let msg = AdapterProcessMessage::HookResult {
        id: "4".into(),
        action: "continue".into(),
        data: None,
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"hook_result\""));

    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn adapter_hook_result_block_round_trips() {
    let msg = AdapterProcessMessage::HookResult {
        id: "4".into(),
        action: "block".into(),
        data: Some(serde_json::json!({"reason": "forbidden"})),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Adapter -> Host messages: state_result
// ---------------------------------------------------------------------------

#[test]
fn adapter_state_result_round_trips() {
    let msg = AdapterProcessMessage::StateResult {
        id: "5".into(),
        state: serde_json::json!({"todos": ["a", "b"]}),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"state_result\""));

    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Adapter -> Host messages: error
// ---------------------------------------------------------------------------

#[test]
fn adapter_error_round_trips() {
    let msg = AdapterProcessMessage::Error {
        id: Some("2".into()),
        message: "tool execution failed".into(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"error\""));

    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

#[test]
fn adapter_error_without_id_round_trips() {
    let msg = AdapterProcessMessage::Error {
        id: None,
        message: "adapter crashed".into(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: AdapterProcessMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back);
}

// ---------------------------------------------------------------------------
// Unknown type rejection
// ---------------------------------------------------------------------------

#[test]
fn unknown_host_message_type_is_rejected() {
    let json = r#"{"type":"fly_to_moon","id":"99"}"#;
    let result = serde_json::from_str::<AdapterHostMessage>(json);
    assert!(result.is_err(), "unknown type must be rejected");
}

#[test]
fn unknown_process_message_type_is_rejected() {
    let json = r#"{"type":"teleport","id":"99"}"#;
    let result = serde_json::from_str::<AdapterProcessMessage>(json);
    assert!(result.is_err(), "unknown type must be rejected");
}

// ---------------------------------------------------------------------------
// JSONL round-trip: messages survive line-oriented serialization
// ---------------------------------------------------------------------------

#[test]
fn jsonl_round_trip_all_message_types() {
    let host_messages: Vec<AdapterHostMessage> = vec![
        AdapterHostMessage::Initialize {
            id: "1".into(),
            protocol: PROTOCOL_VERSION.into(),
            package: "todo".into(),
        },
        AdapterHostMessage::ToolCall {
            id: "2".into(),
            tool: "todo_add".into(),
            args: serde_json::json!({"text": "test"}),
        },
        AdapterHostMessage::Command {
            id: "3".into(),
            name: "todo/list".into(),
            args: serde_json::json!({}),
        },
        AdapterHostMessage::Hook {
            id: "4".into(),
            hook: "before_tool_call".into(),
            payload: serde_json::json!({"tool": "bash"}),
        },
        AdapterHostMessage::Event {
            event: serde_json::json!({"turn": 1}),
        },
        AdapterHostMessage::StateSerialize { id: "5".into() },
        AdapterHostMessage::StateRestore {
            id: "5".into(),
            state: serde_json::json!({}),
        },
        AdapterHostMessage::Cancel {
            id: "2".into(),
            reason: "user_abort".into(),
        },
        AdapterHostMessage::Shutdown {
            id: "6".into(),
            reason: "done".into(),
        },
    ];

    let process_messages: Vec<AdapterProcessMessage> = vec![
        AdapterProcessMessage::Capabilities {
            id: "1".into(),
            tools: vec![],
            commands: vec![],
            hooks: vec![],
            model_overrides: vec![],
        },
        AdapterProcessMessage::ToolResult {
            id: "2".into(),
            content: vec![serde_json::json!("ok")],
            is_error: false,
        },
        AdapterProcessMessage::CommandResult {
            id: "3".into(),
            data: serde_json::json!({}),
        },
        AdapterProcessMessage::HookResult {
            id: "4".into(),
            action: "continue".into(),
            data: None,
        },
        AdapterProcessMessage::StateResult {
            id: "5".into(),
            state: serde_json::json!({}),
        },
        AdapterProcessMessage::Error {
            id: None,
            message: "crash".into(),
        },
    ];

    // Write all messages as JSONL lines, then read them back
    let mut jsonl = String::new();
    for msg in &host_messages {
        jsonl.push_str(&serde_json::to_string(msg).expect("serialize host"));
        jsonl.push('\n');
    }
    for msg in &process_messages {
        jsonl.push_str(&serde_json::to_string(msg).expect("serialize process"));
        jsonl.push('\n');
    }

    let mut host_roundtrip = Vec::new();
    let mut process_roundtrip = Vec::new();
    for line in jsonl.lines() {
        if line.is_empty() {
            continue;
        }
        // Try host first, then process
        if let Ok(h) = serde_json::from_str::<AdapterHostMessage>(line) {
            host_roundtrip.push(h);
        } else if let Ok(p) = serde_json::from_str::<AdapterProcessMessage>(line) {
            process_roundtrip.push(p);
        } else {
            panic!("line did not parse as either message type: {line}");
        }
    }

    assert_eq!(
        host_roundtrip, host_messages,
        "host messages must survive JSONL round-trip"
    );
    assert_eq!(
        process_roundtrip, process_messages,
        "process messages must survive JSONL round-trip"
    );
}

// ---------------------------------------------------------------------------
// Helper: AdapterProcessMessage::is_error
// ---------------------------------------------------------------------------

#[test]
fn is_error_true_only_for_tool_result_with_error_flag() {
    let err = AdapterProcessMessage::ToolResult {
        id: "1".into(),
        content: vec![],
        is_error: true,
    };
    assert!(err.is_error());

    let ok = AdapterProcessMessage::ToolResult {
        id: "1".into(),
        content: vec![],
        is_error: false,
    };
    assert!(!ok.is_error());

    // Non-tool_result variants should return false
    let cmd = AdapterProcessMessage::CommandResult {
        id: "1".into(),
        data: serde_json::json!({}),
    };
    assert!(!cmd.is_error());
}
