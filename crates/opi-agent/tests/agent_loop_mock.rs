//! Behavioral tests for agent_loop with mock provider and tools (task 1.6).
//!
//! DoD: "mock tests cover no-tool and tool-use turns"

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures_util::stream;
use opi_agent::event::{AgentEvent, AgentEventSink};
use opi_agent::hooks::{
    AgentHooks, BeforeToolCallContext, BeforeToolCallResult, ShouldStopAfterTurnContext,
};
use opi_agent::loop_types::{AgentError, AgentLoopConfig, AgentLoopContext};
use opi_agent::message::AgentMessage;
use opi_agent::tool::{Tool, ToolError, ToolResult};
use opi_ai::message::{
    AssistantContent, AssistantMessage, InputContent, Message, ToolCall, UserMessage,
};
use opi_ai::provider::{EventStream, Provider, ProviderError, Request};
use opi_ai::stream::{AssistantStreamEvent, StopReason, Usage};
use serde_json::json;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Mock provider
// ---------------------------------------------------------------------------

/// Provider that returns a pre-programmed sequence of stream event batches.
/// Each call to `stream()` pops the next batch of events from the queue.
struct MockProvider {
    id: String,
    responses: Arc<Mutex<Vec<Vec<AssistantStreamEvent>>>>,
}

impl MockProvider {
    fn new(id: &str, responses: Vec<Vec<AssistantStreamEvent>>) -> Self {
        Self {
            id: id.to_owned(),
            responses: Arc::new(Mutex::new(responses)),
        }
    }
}

impl Provider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn models(&self) -> &[opi_ai::provider::ModelInfo] {
        &[]
    }

    fn stream(&self, _request: Request) -> EventStream {
        let events = self.responses.lock().unwrap().remove(0);
        Box::pin(stream::iter(events.into_iter().map(Ok::<_, ProviderError>)))
    }
}

// ---------------------------------------------------------------------------
// Mock tool
// -------------------------------------------------------------------

struct MockTool {
    name: String,
    call_log: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl MockTool {
    fn new(name: &str, call_log: Arc<Mutex<Vec<serde_json::Value>>>) -> Self {
        Self {
            name: name.to_owned(),
            call_log,
        }
    }
}

impl Tool for MockTool {
    fn definition(&self) -> opi_ai::message::ToolDef {
        opi_ai::message::ToolDef {
            name: self.name.clone(),
            description: format!("Mock tool: {}", self.name),
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

// ---------------------------------------------------------------------------
// Default hooks for testing
// ---------------------------------------------------------------------------

struct TestHooks;

impl AgentHooks for TestHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        let mut result = Vec::new();
        for msg in messages {
            if let AgentMessage::Llm(m) = msg {
                result.push(m.clone());
            }
        }
        Ok(result)
    }

    fn should_stop_after_turn(
        &self,
        _ctx: ShouldStopAfterTurnContext,
    ) -> Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        Box::pin(async { false })
    }

    fn before_tool_call(
        &self,
        _ctx: BeforeToolCallContext,
    ) -> Pin<Box<dyn std::future::Future<Output = BeforeToolCallResult> + Send>> {
        Box::pin(async { BeforeToolCallResult::Allow })
    }
}

// ---------------------------------------------------------------------------
// Helper: build a base assistant message
// ---------------------------------------------------------------------------

fn base_assistant() -> AssistantMessage {
    AssistantMessage {
        content: vec![],
        api: opi_ai::ApiKind::Anthropic,
        provider: "mock".into(),
        model: "mock-model".into(),
        response_model: None,
        response_id: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp_ms: 0,
    }
}

// ---------------------------------------------------------------------------
// Test: no-tool turn
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_tool_turn_emits_lifecycle_events() {
    // Provider returns a simple text response, no tool calls.
    let mut partial = base_assistant();
    partial.content.push(AssistantContent::Text {
        text: "Hello!".into(),
    });

    let response_events = vec![
        AssistantStreamEvent::Start {
            partial: base_assistant(),
        },
        AssistantStreamEvent::TextDelta {
            content_index: 0,
            delta: "Hello!".into(),
            partial: partial.clone(),
        },
        AssistantStreamEvent::Done {
            reason: StopReason::Stop,
            message: partial,
        },
    ];

    let provider = MockProvider::new("mock", vec![response_events]);
    let collected_events: Arc<Mutex<Vec<AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let event_collector = collected_events.clone();

    let sink: AgentEventSink = Box::new(move |event| {
        event_collector.lock().unwrap().push(event);
    });

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![],
        messages: vec![AgentMessage::Llm(Message::User(UserMessage {
            content: vec![InputContent::Text { text: "Hi".into() }],
            timestamp_ms: 0,
        }))],
        model: "mock-model".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: None,
    };

    let config = AgentLoopConfig {
        max_turns: 10,
        ..Default::default()
    };

    let hooks = TestHooks;
    let result = opi_agent::agent_loop(context, config, &hooks, sink, CancellationToken::new())
        .await
        .unwrap();

    // Should return at least the original user message + assistant response
    assert!(result.len() >= 2);

    // Events should include AgentStart and AgentEnd
    let events = collected_events.lock().unwrap();
    let event_types: Vec<&str> = events
        .iter()
        .map(|e| match e {
            AgentEvent::AgentStart => "AgentStart",
            AgentEvent::AgentEnd { .. } => "AgentEnd",
            AgentEvent::TurnStart => "TurnStart",
            AgentEvent::TurnEnd { .. } => "TurnEnd",
            AgentEvent::MessageStart { .. } => "MessageStart",
            AgentEvent::MessageUpdate { .. } => "MessageUpdate",
            AgentEvent::MessageEnd { .. } => "MessageEnd",
            AgentEvent::ToolExecutionStart { .. } => "ToolExecutionStart",
            AgentEvent::ToolExecutionUpdate { .. } => "ToolExecutionUpdate",
            AgentEvent::ToolExecutionEnd { .. } => "ToolExecutionEnd",
            _ => "Unknown",
        })
        .collect();

    assert!(event_types.contains(&"AgentStart"), "missing AgentStart");
    assert!(event_types.contains(&"AgentEnd"), "missing AgentEnd");
    assert!(event_types.contains(&"TurnStart"), "missing TurnStart");
    assert!(event_types.contains(&"TurnEnd"), "missing TurnEnd");
}

// ---------------------------------------------------------------------------
// Test: tool-use turn
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tool_use_turn_executes_tool_and_loops() {
    // First response: tool call. Second response: text.
    let tool_call_log: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));

    let mut partial_with_tool = base_assistant();
    let tool_call = ToolCall {
        id: "tc-1".into(),
        name: "greet".into(),
        arguments: r#"{"arg":"world"}"#.into(),
    };
    partial_with_tool.content.push(AssistantContent::ToolCall {
        tool_call: tool_call.clone(),
    });

    let first_response = vec![
        AssistantStreamEvent::Start {
            partial: base_assistant(),
        },
        AssistantStreamEvent::ToolCallEnd {
            content_index: 0,
            tool_call: tool_call.clone(),
            partial: partial_with_tool.clone(),
        },
        AssistantStreamEvent::Done {
            reason: StopReason::ToolUse,
            message: partial_with_tool,
        },
    ];

    // Second response: plain text
    let mut partial_final = base_assistant();
    partial_final.content.push(AssistantContent::Text {
        text: "Done!".into(),
    });

    let second_response = vec![
        AssistantStreamEvent::Start {
            partial: base_assistant(),
        },
        AssistantStreamEvent::TextDelta {
            content_index: 0,
            delta: "Done!".into(),
            partial: partial_final.clone(),
        },
        AssistantStreamEvent::Done {
            reason: StopReason::Stop,
            message: partial_final,
        },
    ];

    let provider = MockProvider::new("mock", vec![first_response, second_response]);

    let tool = MockTool::new("greet", tool_call_log.clone());

    let collected_events: Arc<Mutex<Vec<AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let event_collector = collected_events.clone();

    let sink: AgentEventSink = Box::new(move |event| {
        event_collector.lock().unwrap().push(event);
    });

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![Box::new(tool)],
        messages: vec![AgentMessage::Llm(Message::User(UserMessage {
            content: vec![InputContent::Text {
                text: "Use the tool".into(),
            }],
            timestamp_ms: 0,
        }))],
        model: "mock-model".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: None,
    };

    let config = AgentLoopConfig {
        max_turns: 10,
        ..Default::default()
    };

    let hooks = TestHooks;
    let result = opi_agent::agent_loop(context, config, &hooks, sink, CancellationToken::new())
        .await
        .unwrap();

    // Tool should have been called
    let log = tool_call_log.lock().unwrap();
    assert_eq!(log.len(), 1, "tool should have been called exactly once");
    assert_eq!(log[0]["arg"], "world");

    // Result should contain user message, assistant with tool call, tool result, final assistant
    assert!(
        result.len() >= 3,
        "expected at least 3 messages, got {}",
        result.len()
    );

    // Events should include tool execution events
    let events = collected_events.lock().unwrap();
    let has_tool_start = events.iter().any(
        |e| matches!(e, AgentEvent::ToolExecutionStart { tool_name, .. } if tool_name == "greet"),
    );
    let has_tool_end = events.iter().any(|e| {
        matches!(e, AgentEvent::ToolExecutionEnd { tool_name, is_error: false, .. } if tool_name == "greet")
    });
    assert!(has_tool_start, "missing ToolExecutionStart for greet");
    assert!(has_tool_end, "missing ToolExecutionEnd for greet");

    // Should have two turns (tool call + final response)
    let turn_starts = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::TurnStart))
        .count();
    assert!(
        turn_starts >= 2,
        "expected at least 2 TurnStart events, got {}",
        turn_starts
    );
}
