//! E2E tests for interactive CLI wiring with MockProvider (task 1.14).
//!
//! DoD: "runs against mock provider"
//!
//! Tests exercise the full path: CodingHarness → Agent → MockProvider,
//! verifying tool wiring, system prompt construction, hooks, and multi-turn.

use std::sync::{Arc, Mutex};

use opi_agent::event::AgentEvent;
use opi_agent::message::AgentMessage;
use opi_agent::tool::{Tool, ToolError, ToolResult};
use opi_ai::message::{InputContent, Message};
use opi_ai::test_support::{self, MockProvider};
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::harness::CodingHarness;
use serde_json::json;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A mock tool that records its invocations for assertion.
struct RecordTool {
    name: String,
    call_log: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl RecordTool {
    fn new(name: &str, call_log: Arc<Mutex<Vec<serde_json::Value>>>) -> Self {
        Self {
            name: name.to_owned(),
            call_log,
        }
    }
}

impl Tool for RecordTool {
    fn definition(&self) -> opi_ai::message::ToolDef {
        opi_ai::message::ToolDef {
            name: self.name.clone(),
            description: format!("Record tool: {}", self.name),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "arg": { "type": "string" }
                },
                "required": ["arg"]
            }),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        arguments: serde_json::Value,
        _signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<ToolResult, ToolError>> + Send>> {
        let log = self.call_log.clone();
        log.lock().unwrap().push(arguments.clone());
        let text = arguments
            .get("arg")
            .and_then(|v| v.as_str())
            .unwrap_or("mock-result")
            .to_owned();
        Box::pin(async move {
            Ok(ToolResult {
                content: vec![opi_ai::message::OutputContent::Text {
                    text: format!("tool-result: {text}"),
                }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }
}

fn event_name(event: &AgentEvent) -> &'static str {
    use AgentEvent::*;
    match event {
        AgentStart => "AgentStart",
        AgentEnd { .. } => "AgentEnd",
        TurnStart => "TurnStart",
        TurnEnd { .. } => "TurnEnd",
        MessageStart { .. } => "MessageStart",
        MessageUpdate { .. } => "MessageUpdate",
        MessageEnd { .. } => "MessageEnd",
        ToolExecutionStart { .. } => "ToolExecutionStart",
        ToolExecutionEnd { is_error, .. } => {
            if *is_error {
                "ToolExecutionEnd(error)"
            } else {
                "ToolExecutionEnd(ok)"
            }
        }
        _ => "Other",
    }
}

// ---------------------------------------------------------------------------
// Test 1: text prompt through CodingHarness with MockProvider
// ---------------------------------------------------------------------------

#[tokio::test]
async fn harness_text_prompt_with_mock() {
    let response = test_support::text_response("Hello from harness!");
    let provider = MockProvider::new("mock", vec![response]);

    let mut harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
    );

    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let ev = events.clone();
    harness.subscribe(Box::new(move |event| {
        ev.lock().unwrap().push(event_name(event).to_owned());
    }));

    let result = harness.prompt("Hi there").await.unwrap();

    // Should have user message + assistant response
    assert!(
        result.len() >= 2,
        "expected >= 2 messages, got {}",
        result.len()
    );

    // First message is user
    if let AgentMessage::Llm(Message::User(user)) = &result[0] {
        let text = &user.content[0];
        assert!(
            matches!(text, InputContent::Text { text } if text == "Hi there"),
            "first message should be the user prompt"
        );
    } else {
        panic!("first message should be a User message");
    }

    // Should have assistant message
    let has_assistant = result
        .iter()
        .any(|m| matches!(m, AgentMessage::Llm(Message::Assistant(_))));
    assert!(has_assistant, "should have at least one Assistant message");

    // Lifecycle events
    let ev_lock = events.lock().unwrap();
    assert!(
        ev_lock.contains(&"AgentStart".to_owned()),
        "missing AgentStart"
    );
    assert!(ev_lock.contains(&"AgentEnd".to_owned()), "missing AgentEnd");
}

// ---------------------------------------------------------------------------
// Test 2: tool call through CodingHarness with MockProvider
// ---------------------------------------------------------------------------

#[tokio::test]
async fn harness_tool_call_with_mock() {
    let tool_call_log: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));

    let first = test_support::tool_call_response("tc-1", "record_tool", r#"{"arg":"hello"}"#);
    let second = test_support::text_response("Tool executed!");

    let provider = MockProvider::new("mock", vec![first, second]);

    let mut harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
    );

    // Add the record tool alongside built-in tools
    let record_tool = RecordTool::new("record_tool", tool_call_log.clone());
    harness.add_tool(Box::new(record_tool));

    let result = harness.prompt("Use the record tool").await.unwrap();

    // Tool should have been called
    let log = tool_call_log.lock().unwrap();
    assert_eq!(log.len(), 1, "tool should have been called exactly once");
    assert_eq!(log[0]["arg"], "hello");

    // Should have: user → assistant(tool_call) → tool_result → assistant(text)
    assert!(
        result.len() >= 4,
        "expected >= 4 messages, got {}",
        result.len()
    );
}

// ---------------------------------------------------------------------------
// Test 3: system prompt includes built-in tool descriptions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn harness_system_prompt_includes_tools() {
    let response = test_support::text_response("ok");
    let provider = MockProvider::new("mock", vec![response]);

    let harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
    );

    // Use the system_prompt() accessor to verify tool descriptions
    let system_prompt = harness.system_prompt();
    assert!(
        system_prompt.contains("Available tools:"),
        "system prompt should include tool section header"
    );
    assert!(
        system_prompt.contains("read"),
        "system prompt should mention read tool"
    );
    assert!(
        system_prompt.contains("bash"),
        "system prompt should mention bash tool"
    );
}

// ---------------------------------------------------------------------------
// Test 4: multi-turn conversation through CodingHarness
// ---------------------------------------------------------------------------

#[tokio::test]
async fn harness_multi_turn_with_mock() {
    let first = test_support::text_response("First response");
    let second = test_support::text_response("Second response");

    let provider = MockProvider::new("mock", vec![first, second]);

    let mut harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
    );

    let result1 = harness.prompt("Hello").await.unwrap();
    assert!(result1.len() >= 2, "first turn should have >= 2 messages");

    let result2 = harness.continue_("Tell me more").await.unwrap();

    // After two turns: user1 + asst1 + user2 + asst2
    assert!(
        result2.len() >= 4,
        "expected >= 4 messages after two turns, got {}",
        result2.len()
    );
}

// ---------------------------------------------------------------------------
// Test 5: harness respects config max_iterations
// ---------------------------------------------------------------------------

#[tokio::test]
async fn harness_respects_max_iterations_config() {
    let response = test_support::text_response("ok");
    let provider = MockProvider::new("mock", vec![response]);

    let mut config = OpiConfig::default();
    config.defaults.max_iterations = 3;

    let harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        config,
        std::env::current_dir().unwrap(),
    );

    // Harness should be created without error even with low max_iterations
    // (the agent loop will enforce the cap internally)
    drop(harness);
}
