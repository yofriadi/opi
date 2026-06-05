//! Behavioral tests for opi-web-ui event parsing, conversation state,
//! component model, and HTML rendering.
//!
//! All tests use mock data — no live provider calls.

use opi_agent::event::AgentEvent;
use opi_agent::sdk::{SDK_SCHEMA_VERSION, agent_event_to_value};
use opi_web_ui::components::{
    ChatMessage, ConversationView, StatusBar, ThinkingBlock, ToolCallStatus, ToolCallView,
};
use opi_web_ui::event::WebUiEvent;
use opi_web_ui::render::Render;
use opi_web_ui::state::ConversationState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_tool_call_end_event(
    tool_call_id: &str,
    tool_name: &str,
    result: &str,
    is_error: bool,
) -> AgentEvent {
    AgentEvent::ToolExecutionEnd {
        tool_call_id: tool_call_id.to_owned(),
        tool_name: tool_name.to_owned(),
        result: serde_json::json!(result),
        details: None,
        is_error,
    }
}

// ---------------------------------------------------------------------------
// 1. Event parsing — WebUiEvent::parse from raw JSON values
// ---------------------------------------------------------------------------

#[test]
fn parse_agent_start_event() {
    let raw = serde_json::json!({"type": "AgentStart"});
    let event = WebUiEvent::parse(&raw).expect("should parse AgentStart");
    assert!(matches!(event, WebUiEvent::AgentStart));
}

#[test]
fn parse_agent_end_event() {
    let raw = serde_json::json!({"type": "AgentEnd", "messages": []});
    let event = WebUiEvent::parse(&raw).expect("should parse AgentEnd");
    assert!(matches!(event, WebUiEvent::AgentEnd { .. }));
}

#[test]
fn parse_message_start_event() {
    let raw = serde_json::json!({
        "type": "MessageStart",
        "message": {
            "type": "Llm",
            "role": "assistant",
            "content": [],
            "api": "anthropic",
            "provider": "test",
            "model": "test-model",
            "usage": {"input_tokens": 0, "output_tokens": 0},
            "stop_reason": "stop",
            "timestamp_ms": 1000
        }
    });
    let event = WebUiEvent::parse(&raw).expect("should parse MessageStart");
    assert!(matches!(event, WebUiEvent::MessageStart { .. }));
}

#[test]
fn parse_message_update_text_delta() {
    let raw = serde_json::json!({
        "type": "MessageUpdate",
        "message": {
            "type": "Llm",
            "role": "assistant",
            "content": [],
            "api": "anthropic",
            "provider": "test",
            "model": "test-model",
            "usage": {"input_tokens": 0, "output_tokens": 0},
            "stop_reason": "stop",
            "timestamp_ms": 1000
        },
        "assistant_event": {
            "type": "text_delta",
            "content_index": 0,
            "delta": "Hello"
        }
    });
    let event = WebUiEvent::parse(&raw).expect("should parse MessageUpdate");
    match event {
        WebUiEvent::TextDelta { delta, .. } => assert_eq!(delta, "Hello"),
        other => panic!("expected TextDelta, got {:?}", other),
    }
}

#[test]
fn parse_message_update_thinking_delta() {
    let raw = serde_json::json!({
        "type": "MessageUpdate",
        "message": {
            "type": "Llm",
            "role": "assistant",
            "content": [],
            "api": "anthropic",
            "provider": "test",
            "model": "test-model",
            "usage": {"input_tokens": 0, "output_tokens": 0},
            "stop_reason": "stop",
            "timestamp_ms": 1000
        },
        "assistant_event": {
            "type": "thinking_delta",
            "content_index": 0,
            "delta": "Hmm"
        }
    });
    let event = WebUiEvent::parse(&raw).expect("should parse MessageUpdate");
    match event {
        WebUiEvent::ThinkingDelta { delta, .. } => assert_eq!(delta, "Hmm"),
        other => panic!("expected ThinkingDelta, got {:?}", other),
    }
}

#[test]
fn parse_tool_execution_start_event() {
    let raw = serde_json::json!({
        "type": "ToolExecutionStart",
        "tool_call_id": "tc-1",
        "tool_name": "read",
        "args": {"path": "/tmp/test.txt"}
    });
    let event = WebUiEvent::parse(&raw).expect("should parse ToolExecutionStart");
    match event {
        WebUiEvent::ToolStart {
            tool_call_id,
            tool_name,
            ..
        } => {
            assert_eq!(tool_call_id, "tc-1");
            assert_eq!(tool_name, "read");
        }
        other => panic!("expected ToolStart, got {:?}", other),
    }
}

#[test]
fn parse_tool_execution_end_event() {
    let raw = serde_json::json!({
        "type": "ToolExecutionEnd",
        "tool_call_id": "tc-1",
        "tool_name": "read",
        "result": "file contents",
        "details": null,
        "is_error": false
    });
    let event = WebUiEvent::parse(&raw).expect("should parse ToolExecutionEnd");
    match event {
        WebUiEvent::ToolEnd {
            tool_call_id,
            tool_name,
            is_error,
            ..
        } => {
            assert_eq!(tool_call_id, "tc-1");
            assert_eq!(tool_name, "read");
            assert!(!is_error);
        }
        other => panic!("expected ToolEnd, got {:?}", other),
    }
}

#[test]
fn parse_tool_execution_end_error_event() {
    let raw = serde_json::json!({
        "type": "ToolExecutionEnd",
        "tool_call_id": "tc-2",
        "tool_name": "bash",
        "result": "command failed",
        "details": null,
        "is_error": true
    });
    let event = WebUiEvent::parse(&raw).expect("should parse ToolExecutionEnd error");
    match event {
        WebUiEvent::ToolEnd { is_error, .. } => assert!(is_error),
        other => panic!("expected ToolEnd, got {:?}", other),
    }
}

#[test]
fn parse_compaction_start_event() {
    let raw = serde_json::json!({
        "type": "CompactionStart",
        "reason": "manual"
    });
    let event = WebUiEvent::parse(&raw).expect("should parse CompactionStart");
    assert!(matches!(event, WebUiEvent::CompactionStart { .. }));
}

#[test]
fn parse_compaction_end_event() {
    let raw = serde_json::json!({
        "type": "CompactionEnd",
        "reason": "manual",
        "result": null,
        "aborted": false,
        "error_message": null
    });
    let event = WebUiEvent::parse(&raw).expect("should parse CompactionEnd");
    assert!(matches!(event, WebUiEvent::CompactionEnd { .. }));
}

#[test]
fn parse_queue_update_event() {
    let raw = serde_json::json!({
        "type": "QueueUpdate",
        "steering": ["fix that bug"],
        "follow_up": ["now run tests"]
    });
    let event = WebUiEvent::parse(&raw).expect("should parse QueueUpdate");
    assert!(matches!(event, WebUiEvent::QueueUpdate { .. }));
}

#[test]
fn parse_turn_start_event() {
    let raw = serde_json::json!({"type": "TurnStart"});
    let event = WebUiEvent::parse(&raw).expect("should parse TurnStart");
    assert!(matches!(event, WebUiEvent::TurnStart));
}

#[test]
fn parse_turn_end_event() {
    let raw = serde_json::json!({
        "type": "TurnEnd",
        "message": {
            "type": "Llm",
            "role": "assistant",
            "content": [],
            "api": "anthropic",
            "provider": "test",
            "model": "test-model",
            "usage": {"input_tokens": 0, "output_tokens": 0},
            "stop_reason": "stop",
            "timestamp_ms": 1000
        },
        "tool_results": []
    });
    let event = WebUiEvent::parse(&raw).expect("should parse TurnEnd");
    assert!(matches!(event, WebUiEvent::TurnEnd));
}

#[test]
fn parse_rpc_response_event() {
    let raw = serde_json::json!({
        "type": "response",
        "command": "prompt",
        "success": true,
        "id": "42"
    });
    let event = WebUiEvent::parse(&raw).expect("should parse RPC response");
    match event {
        WebUiEvent::RpcResponse {
            command,
            success,
            id,
            ..
        } => {
            assert_eq!(command, "prompt");
            assert!(success);
            assert_eq!(id.as_deref(), Some("42"));
        }
        other => panic!("expected RpcResponse, got {:?}", other),
    }
}

#[test]
fn parse_rpc_response_error() {
    let raw = serde_json::json!({
        "type": "response",
        "command": "set_model",
        "success": false,
        "error": "model not found"
    });
    let event = WebUiEvent::parse(&raw).expect("should parse RPC error response");
    match event {
        WebUiEvent::RpcResponse { success, error, .. } => {
            assert!(!success);
            assert_eq!(error.as_deref(), Some("model not found"));
        }
        other => panic!("expected RpcResponse, got {:?}", other),
    }
}

#[test]
fn parse_rpc_ready_event() {
    let raw = serde_json::json!({
        "type": "rpc_ready",
        "schema_version": 2,
        "mode": "rpc",
        "version": "0.4.0"
    });
    let event = WebUiEvent::parse(&raw).expect("should parse rpc_ready");
    assert!(matches!(event, WebUiEvent::RpcReady { .. }));
}

#[test]
fn parse_unknown_event_type_returns_unknown() {
    let raw = serde_json::json!({"type": "FutureEventType", "data": 42});
    let event = WebUiEvent::parse(&raw).expect("should parse unknown");
    assert!(matches!(event, WebUiEvent::Unknown { .. }));
}

#[test]
fn parse_missing_type_returns_unknown() {
    let raw = serde_json::json!({"data": "no type field"});
    let event = WebUiEvent::parse(&raw).expect("should parse missing type");
    assert!(matches!(event, WebUiEvent::Unknown { .. }));
}

#[test]
fn parse_auto_retry_start_event() {
    let raw = serde_json::json!({
        "type": "AutoRetryStart",
        "attempt": 1,
        "max_attempts": 3,
        "delay_ms": 1000,
        "error_message": "rate limited"
    });
    let event = WebUiEvent::parse(&raw).expect("should parse AutoRetryStart");
    assert!(matches!(event, WebUiEvent::AutoRetryStart { .. }));
}

#[test]
fn parse_session_persist_error_event() {
    let raw = serde_json::json!({
        "type": "SessionPersistError",
        "message": "disk full"
    });
    let event = WebUiEvent::parse(&raw).expect("should parse SessionPersistError");
    assert!(matches!(event, WebUiEvent::SessionPersistError { .. }));
}

// ---------------------------------------------------------------------------
// 2. Conversation state — processing events maintains correct state
// ---------------------------------------------------------------------------

#[test]
fn state_starts_empty() {
    let state = ConversationState::new();
    assert!(state.messages().is_empty());
    assert!(state.tool_calls().is_empty());
    assert!(state.thinking_blocks().is_empty());
    assert_eq!(state.model(), None);
    assert_eq!(state.session_id(), None);
}

#[test]
fn state_processes_text_streaming() {
    let mut state = ConversationState::new();

    // Start a message
    state.process(WebUiEvent::MessageStart {
        model: "claude-sonnet-4-5".to_owned(),
        provider: "anthropic".to_owned(),
    });

    // Stream text deltas
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "Hello ".to_owned(),
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "world".to_owned(),
    });

    // End message
    state.process(WebUiEvent::MessageEnd);

    let messages = state.messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text(), "Hello world");
}

#[test]
fn state_accumulates_thinking() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::MessageStart {
        model: "test".to_owned(),
        provider: "test".to_owned(),
    });

    state.process(WebUiEvent::ThinkingStart { index: 0 });
    state.process(WebUiEvent::ThinkingDelta {
        index: 0,
        delta: "Let me think".to_owned(),
    });
    state.process(WebUiEvent::ThinkingDelta {
        index: 0,
        delta: " about this".to_owned(),
    });
    state.process(WebUiEvent::ThinkingEnd {
        index: 0,
        content: "Let me think about this".to_owned(),
    });

    state.process(WebUiEvent::TextDelta {
        index: 1,
        delta: "Answer".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);

    let thinking = state.thinking_blocks();
    assert_eq!(thinking.len(), 1);
    assert_eq!(thinking[0].content(), "Let me think about this");
}

#[test]
fn state_tracks_tool_execution() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::ToolStart {
        tool_call_id: "tc-1".to_owned(),
        tool_name: "read".to_owned(),
        args: serde_json::json!({"path": "/tmp/test.txt"}),
    });

    assert_eq!(state.tool_calls().len(), 1);
    assert_eq!(state.tool_calls()[0].tool_name(), "read");
    assert_eq!(state.tool_calls()[0].status(), ToolCallStatus::Running);

    state.process(WebUiEvent::ToolEnd {
        tool_call_id: "tc-1".to_owned(),
        tool_name: "read".to_owned(),
        result: serde_json::json!("file contents"),
        is_error: false,
    });

    assert_eq!(state.tool_calls()[0].status(), ToolCallStatus::Completed);
}

#[test]
fn state_tracks_tool_error() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::ToolStart {
        tool_call_id: "tc-err".to_owned(),
        tool_name: "bash".to_owned(),
        args: serde_json::json!({"command": "false"}),
    });
    state.process(WebUiEvent::ToolEnd {
        tool_call_id: "tc-err".to_owned(),
        tool_name: "bash".to_owned(),
        result: serde_json::json!("exit code 1"),
        is_error: true,
    });

    assert_eq!(state.tool_calls()[0].status(), ToolCallStatus::Failed);
}

#[test]
fn state_handles_model_change() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::ModelChanged {
        model: "gpt-4o".to_owned(),
    });
    assert_eq!(state.model(), Some("gpt-4o"));

    state.process(WebUiEvent::ModelChanged {
        model: "claude-sonnet-4-5".to_owned(),
    });
    assert_eq!(state.model(), Some("claude-sonnet-4-5"));
}

#[test]
fn state_handles_session_info() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::SessionInfo {
        session_id: "sess-abc".to_owned(),
        turn_count: 5,
        message_count: 12,
    });
    assert_eq!(state.session_id(), Some("sess-abc"));
    assert_eq!(state.turn_count(), 5);
}

#[test]
fn state_handles_compaction_events() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::CompactionStart {
        reason: "overflow".to_owned(),
    });
    assert!(state.is_compacting());

    state.process(WebUiEvent::CompactionEnd {
        reason: "overflow".to_owned(),
        aborted: false,
    });
    assert!(!state.is_compacting());
}

#[test]
fn state_tracks_multi_turn_conversation() {
    let mut state = ConversationState::new();

    // Turn 1
    state.process(WebUiEvent::TurnStart);
    state.process(WebUiEvent::MessageStart {
        model: "test".to_owned(),
        provider: "test".to_owned(),
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "First".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);
    state.process(WebUiEvent::TurnEnd);

    // Turn 2
    state.process(WebUiEvent::TurnStart);
    state.process(WebUiEvent::MessageStart {
        model: "test".to_owned(),
        provider: "test".to_owned(),
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "Second".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);
    state.process(WebUiEvent::TurnEnd);

    assert_eq!(state.messages().len(), 2);
    assert_eq!(state.messages()[0].text(), "First");
    assert_eq!(state.messages()[1].text(), "Second");
}

#[test]
fn state_handles_rpc_response_success() {
    let mut state = ConversationState::new();
    state.process(WebUiEvent::RpcResponse {
        command: "prompt".to_owned(),
        success: true,
        id: Some("42".to_owned()),
        error: None,
    });
    assert!(state.last_response().is_some());
    let resp = state.last_response().unwrap();
    assert_eq!(resp.command, "prompt");
    assert!(resp.success);
}

#[test]
fn state_handles_rpc_response_error() {
    let mut state = ConversationState::new();
    state.process(WebUiEvent::RpcResponse {
        command: "set_model".to_owned(),
        success: false,
        id: None,
        error: Some("model not found".to_owned()),
    });
    let resp = state.last_response().unwrap();
    assert!(!resp.success);
    assert_eq!(resp.error.as_deref(), Some("model not found"));
}

#[test]
fn state_tracks_agent_lifecycle() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::AgentStart);
    assert!(state.agent_running());

    state.process(WebUiEvent::AgentEnd { message_count: 1 });
    assert!(!state.agent_running());
}

// ---------------------------------------------------------------------------
// 3. Component model — UI component types
// ---------------------------------------------------------------------------

#[test]
fn chat_message_stores_text() {
    let msg = ChatMessage::new(
        "Hello world".to_owned(),
        "claude-sonnet-4-5".to_owned(),
        "anthropic".to_owned(),
    );
    assert_eq!(msg.text(), "Hello world");
    assert_eq!(msg.model(), "claude-sonnet-4-5");
    assert_eq!(msg.provider(), "anthropic");
}

#[test]
fn chat_message_with_thinking() {
    let msg = ChatMessage::new("Answer".to_owned(), "test".to_owned(), "test".to_owned())
        .with_thinking("Let me reason".to_owned());
    assert_eq!(msg.thinking(), Some("Let me reason"));
}

#[test]
fn chat_message_with_tool_calls() {
    let tc = ToolCallView::new(
        "tc-1".to_owned(),
        "read".to_owned(),
        serde_json::json!({"path": "/tmp/f.txt"}),
    );
    let msg = ChatMessage::new(
        "Here is the file".to_owned(),
        "test".to_owned(),
        "test".to_owned(),
    )
    .with_tool_call(tc);
    assert_eq!(msg.tool_calls().len(), 1);
    assert_eq!(msg.tool_calls()[0].tool_name(), "read");
}

#[test]
fn tool_call_view_statuses() {
    let mut tc = ToolCallView::new(
        "tc-1".to_owned(),
        "bash".to_owned(),
        serde_json::json!({"command": "ls"}),
    );
    assert_eq!(tc.status(), ToolCallStatus::Running);

    tc.complete(serde_json::json!("output"));
    assert_eq!(tc.status(), ToolCallStatus::Completed);
    assert!(!tc.is_error());

    let mut tc2 = ToolCallView::new(
        "tc-2".to_owned(),
        "bash".to_owned(),
        serde_json::json!({"command": "false"}),
    );
    tc2.fail(serde_json::json!("exit 1"));
    assert_eq!(tc2.status(), ToolCallStatus::Failed);
    assert!(tc2.is_error());
}

#[test]
fn thinking_block_content() {
    let tb = ThinkingBlock::new("Deep thought".to_owned());
    assert_eq!(tb.content(), "Deep thought");
}

#[test]
fn status_bar_model_and_session() {
    let mut sb = StatusBar::new();
    assert_eq!(sb.model(), None);

    sb.set_model("gpt-4o".to_owned());
    assert_eq!(sb.model(), Some("gpt-4o"));

    sb.set_session_id("sess-1".to_owned());
    assert_eq!(sb.session_id(), Some("sess-1"));
}

#[test]
fn conversation_view_aggregates_components() {
    let mut view = ConversationView::new();
    assert!(view.messages().is_empty());

    view.add_message(ChatMessage::new(
        "Hello".to_owned(),
        "test".to_owned(),
        "test".to_owned(),
    ));
    view.add_message(ChatMessage::new(
        "World".to_owned(),
        "test".to_owned(),
        "test".to_owned(),
    ));
    assert_eq!(view.messages().len(), 2);
}

// ---------------------------------------------------------------------------
// 4. HTML rendering — components render to valid HTML strings
// ---------------------------------------------------------------------------

#[test]
fn render_text_message_to_html() {
    let msg = ChatMessage::new(
        "Hello world".to_owned(),
        "test".to_owned(),
        "test".to_owned(),
    );
    let html = msg.render_html();
    assert!(html.contains("Hello world"));
    assert!(html.contains("<div"));
    assert!(html.contains("</div>"));
}

#[test]
fn render_thinking_to_html() {
    let msg = ChatMessage::new("Answer".to_owned(), "test".to_owned(), "test".to_owned())
        .with_thinking("My reasoning".to_owned());
    let html = msg.render_html();
    assert!(html.contains("My reasoning"));
    assert!(html.contains("thinking"));
}

#[test]
fn render_tool_call_to_html() {
    let tc = ToolCallView::new(
        "tc-1".to_owned(),
        "read".to_owned(),
        serde_json::json!({"path": "/tmp/f.txt"}),
    );
    let html = tc.render_html();
    assert!(html.contains("read"));
    assert!(html.contains("tool-call"));
}

#[test]
fn render_tool_call_with_result_to_html() {
    let mut tc = ToolCallView::new(
        "tc-1".to_owned(),
        "bash".to_owned(),
        serde_json::json!({"command": "ls"}),
    );
    tc.complete(serde_json::json!("file1.txt\nfile2.txt"));
    let html = tc.render_html();
    assert!(html.contains("file1.txt"));
    assert!(html.contains("completed"));
}

#[test]
fn render_tool_call_error_to_html() {
    let mut tc = ToolCallView::new(
        "tc-2".to_owned(),
        "bash".to_owned(),
        serde_json::json!({"command": "false"}),
    );
    tc.fail(serde_json::json!("exit code 1"));
    let html = tc.render_html();
    assert!(html.contains("error"));
}

#[test]
fn render_status_bar_to_html() {
    let mut sb = StatusBar::new();
    sb.set_model("claude-sonnet-4-5".to_owned());
    sb.set_session_id("sess-abc".to_owned());
    let html = sb.render_html();
    assert!(html.contains("claude-sonnet-4-5"));
    assert!(html.contains("status-bar"));
}

#[test]
fn render_conversation_view_to_html() {
    let mut view = ConversationView::new();
    view.add_message(ChatMessage::new(
        "Hello".to_owned(),
        "test".to_owned(),
        "test".to_owned(),
    ));
    view.add_message(ChatMessage::new(
        "World".to_owned(),
        "test".to_owned(),
        "test".to_owned(),
    ));
    let html = view.render_html();
    assert!(html.contains("Hello"));
    assert!(html.contains("World"));
    assert!(html.contains("conversation"));
}

#[test]
fn render_escaping_prevents_xss() {
    let msg = ChatMessage::new(
        "<script>alert('xss')</script>".to_owned(),
        "test".to_owned(),
        "test".to_owned(),
    );
    let html = msg.render_html();
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
}

// ---------------------------------------------------------------------------
// 5. End-to-end flow — simulate full RPC event stream through state to HTML
// ---------------------------------------------------------------------------

#[test]
fn full_prompt_flow_produces_renderable_html() {
    let mut state = ConversationState::new();

    // Simulate RPC ready
    state.process(WebUiEvent::RpcReady {
        schema_version: SDK_SCHEMA_VERSION,
        version: "0.4.0".to_owned(),
    });

    // RPC response
    state.process(WebUiEvent::RpcResponse {
        command: "prompt".to_owned(),
        success: true,
        id: Some("1".to_owned()),
        error: None,
    });

    // Agent starts
    state.process(WebUiEvent::AgentStart);

    // Turn with text
    state.process(WebUiEvent::TurnStart);
    state.process(WebUiEvent::MessageStart {
        model: "claude-sonnet-4-5".to_owned(),
        provider: "anthropic".to_owned(),
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "Here is the answer.".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);
    state.process(WebUiEvent::TurnEnd);

    // Agent ends
    state.process(WebUiEvent::AgentEnd { message_count: 1 });

    // Build view from state
    let view = state.to_conversation_view();
    let html = view.render_html();

    assert!(html.contains("Here is the answer."));
    assert!(!state.agent_running());
}

#[test]
fn full_tool_call_flow_produces_renderable_html() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::AgentStart);
    state.process(WebUiEvent::TurnStart);
    state.process(WebUiEvent::MessageStart {
        model: "test".to_owned(),
        provider: "test".to_owned(),
    });
    state.process(WebUiEvent::ToolStart {
        tool_call_id: "tc-1".to_owned(),
        tool_name: "read".to_owned(),
        args: serde_json::json!({"path": "Cargo.toml"}),
    });
    state.process(WebUiEvent::ToolEnd {
        tool_call_id: "tc-1".to_owned(),
        tool_name: "read".to_owned(),
        result: serde_json::json!("[package]\nname = \"test\""),
        is_error: false,
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "The package name is test.".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);
    state.process(WebUiEvent::TurnEnd);
    state.process(WebUiEvent::AgentEnd { message_count: 1 });

    let view = state.to_conversation_view();
    let html = view.render_html();

    assert!(html.contains("read"));
    assert!(html.contains("The package name is test."));
    assert_eq!(view.messages()[0].tool_calls().len(), 1);
}

#[test]
fn session_lifecycle_across_prompts() {
    let mut state = ConversationState::new();

    // First prompt
    state.process(WebUiEvent::RpcReady {
        schema_version: 2,
        version: "0.4.0".to_owned(),
    });
    state.process(WebUiEvent::SessionInfo {
        session_id: "sess-1".to_owned(),
        turn_count: 0,
        message_count: 0,
    });
    assert_eq!(state.session_id(), Some("sess-1"));

    state.process(WebUiEvent::AgentStart);
    state.process(WebUiEvent::MessageStart {
        model: "gpt-4o".to_owned(),
        provider: "openai".to_owned(),
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "First response".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);
    state.process(WebUiEvent::AgentEnd { message_count: 1 });

    // Second prompt — model change between prompts
    state.process(WebUiEvent::ModelChanged {
        model: "claude-sonnet-4-5".to_owned(),
    });
    state.process(WebUiEvent::AgentStart);
    state.process(WebUiEvent::MessageStart {
        model: "claude-sonnet-4-5".to_owned(),
        provider: "anthropic".to_owned(),
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "Second response".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);
    state.process(WebUiEvent::AgentEnd { message_count: 2 });

    let messages = state.messages();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text(), "First response");
    assert_eq!(messages[1].text(), "Second response");
    assert_eq!(state.model(), Some("claude-sonnet-4-5"));
}

#[test]
fn compaction_flow_updates_state() {
    let mut state = ConversationState::new();

    // Build up some messages
    state.process(WebUiEvent::MessageStart {
        model: "test".to_owned(),
        provider: "test".to_owned(),
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "Old content".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);

    state.process(WebUiEvent::CompactionStart {
        reason: "overflow".to_owned(),
    });
    assert!(state.is_compacting());

    state.process(WebUiEvent::CompactionEnd {
        reason: "overflow".to_owned(),
        aborted: false,
    });
    assert!(!state.is_compacting());
}

#[test]
fn sdk_event_round_trip_through_web_ui() {
    // Verify that AgentEvent -> json value -> WebUiEvent parses correctly
    let agent_event = AgentEvent::AgentStart;
    let value = agent_event_to_value(&agent_event);
    let web_event = WebUiEvent::parse(&value).expect("should round-trip AgentStart");
    assert!(matches!(web_event, WebUiEvent::AgentStart));
}

#[test]
fn sdk_tool_event_round_trip() {
    let agent_event = make_tool_call_end_event("tc-1", "read", "contents", false);
    let value = agent_event_to_value(&agent_event);
    let web_event = WebUiEvent::parse(&value).expect("should round-trip ToolExecutionEnd");
    match web_event {
        WebUiEvent::ToolEnd { tool_name, .. } => assert_eq!(tool_name, "read"),
        other => panic!("expected ToolEnd, got {:?}", other),
    }
}
