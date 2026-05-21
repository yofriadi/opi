//! Behavioral tests for hooks and queues (task 1.8).
//!
//! DoD: "before/after, should-stop, steering, follow-up tested"

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures_util::stream;
use opi_agent::agent::Agent;
use opi_agent::event::AgentEvent;
use opi_agent::hooks::{
    AfterToolCallContext, AfterToolCallResult, AgentHooks, BeforeToolCallContext,
    BeforeToolCallResult, ShouldStopAfterTurnContext,
};
use opi_agent::loop_types::{AgentError, AgentLoopConfig};
use opi_agent::message::AgentMessage;
use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{
    AssistantContent, AssistantMessage, InputContent, Message, OutputContent, ToolCall, ToolDef,
};
use opi_ai::provider::{EventStream, Provider, ProviderError, Request};
use opi_ai::stream::{AssistantStreamEvent, StopReason, Usage};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Recording mock provider
// ---------------------------------------------------------------------------

struct RecordingProvider {
    responses: Arc<Mutex<Vec<Vec<AssistantStreamEvent>>>>,
    received_messages: Arc<Mutex<Vec<Vec<Message>>>>,
}

impl RecordingProvider {
    fn new(responses: Vec<Vec<AssistantStreamEvent>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            received_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Provider for RecordingProvider {
    fn id(&self) -> &str {
        "recording"
    }

    fn models(&self) -> &[opi_ai::provider::ModelInfo] {
        &[]
    }

    fn stream(&self, request: Request) -> EventStream {
        self.received_messages
            .lock()
            .unwrap()
            .push(request.messages);
        let events = self.responses.lock().unwrap().remove(0);
        Box::pin(stream::iter(events.into_iter().map(Ok::<_, ProviderError>)))
    }
}

// ---------------------------------------------------------------------------
// Echo tool
// ---------------------------------------------------------------------------

struct EchoTool;

impl Tool for EchoTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "echo".into(),
            description: "echoes input".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            }),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        args: serde_json::Value,
        _signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        let text = args["text"].as_str().unwrap_or_default().to_owned();
        Box::pin(async move {
            Ok(ToolResult {
                content: vec![OutputContent::Text { text }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sequential
    }
}

// ---------------------------------------------------------------------------
// Recording hooks (records after_tool_call and should_stop contexts)
// ---------------------------------------------------------------------------

struct RecordingHooks {
    after_calls: Arc<Mutex<Vec<AfterToolCallContext>>>,
    stop_calls: Arc<Mutex<Vec<ShouldStopAfterTurnContext>>>,
    stop_result: bool,
}

impl RecordingHooks {
    fn new(stop_result: bool) -> Self {
        Self {
            after_calls: Arc::new(Mutex::new(Vec::new())),
            stop_calls: Arc::new(Mutex::new(Vec::new())),
            stop_result,
        }
    }
}

impl AgentHooks for RecordingHooks {
    fn convert_to_llm(
        &self,
        messages: &[AgentMessage],
    ) -> Result<Vec<opi_ai::message::Message>, AgentError> {
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
        ctx: ShouldStopAfterTurnContext,
    ) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        let calls = self.stop_calls.clone();
        let stop = self.stop_result;
        Box::pin(async move {
            calls.lock().unwrap().push(ctx);
            stop
        })
    }

    fn before_tool_call(
        &self,
        _ctx: BeforeToolCallContext,
    ) -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>> {
        Box::pin(async { BeforeToolCallResult::Allow })
    }

    fn after_tool_call(
        &self,
        ctx: AfterToolCallContext,
    ) -> Pin<Box<dyn Future<Output = AfterToolCallResult> + Send>> {
        let calls = self.after_calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(ctx);
            AfterToolCallResult::Keep
        })
    }
}

// ---------------------------------------------------------------------------
// Replacing hooks (returns AfterToolCallResult::Replace)
// ---------------------------------------------------------------------------

struct ReplacingHooks;

impl AgentHooks for ReplacingHooks {
    fn convert_to_llm(
        &self,
        messages: &[AgentMessage],
    ) -> Result<Vec<opi_ai::message::Message>, AgentError> {
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
    ) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        Box::pin(async { false })
    }

    fn before_tool_call(
        &self,
        _ctx: BeforeToolCallContext,
    ) -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>> {
        Box::pin(async { BeforeToolCallResult::Allow })
    }

    fn after_tool_call(
        &self,
        ctx: AfterToolCallContext,
    ) -> Pin<Box<dyn Future<Output = AfterToolCallResult> + Send>> {
        let content_len = ctx.result.content.len();
        Box::pin(async move {
            AfterToolCallResult::Replace(ToolResult {
                content: vec![OutputContent::Text {
                    text: format!("replaced: {content_len}"),
                }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn base_assistant() -> AssistantMessage {
    AssistantMessage {
        content: vec![],
        api: opi_ai::ApiKind::Anthropic,
        provider: "recording".into(),
        model: "mock-model".into(),
        response_model: None,
        response_id: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp_ms: 0,
    }
}

fn text_response(text: &str) -> Vec<AssistantStreamEvent> {
    let mut partial = base_assistant();
    partial
        .content
        .push(AssistantContent::Text { text: text.into() });
    vec![
        AssistantStreamEvent::Start {
            partial: base_assistant(),
        },
        AssistantStreamEvent::TextDelta {
            content_index: 0,
            delta: text.into(),
            partial: partial.clone(),
        },
        AssistantStreamEvent::Done {
            reason: StopReason::Stop,
            message: partial,
        },
    ]
}

fn tool_call_response(call_id: &str, tool_name: &str, args: &str) -> Vec<AssistantStreamEvent> {
    let tool_call = ToolCall {
        id: call_id.into(),
        name: tool_name.into(),
        arguments: args.into(),
    };
    let mut partial = base_assistant();
    partial.content.push(AssistantContent::ToolCall {
        tool_call: tool_call.clone(),
    });
    partial.stop_reason = StopReason::ToolUse;
    vec![
        AssistantStreamEvent::Start {
            partial: base_assistant(),
        },
        AssistantStreamEvent::ToolCallEnd {
            content_index: 0,
            tool_call,
            partial: partial.clone(),
        },
        AssistantStreamEvent::Done {
            reason: StopReason::ToolUse,
            message: partial,
        },
    ]
}

fn make_agent(
    provider: RecordingProvider,
    tools: Vec<Box<dyn Tool>>,
    hooks: Box<dyn AgentHooks>,
) -> Agent {
    Agent::new(
        Box::new(provider),
        tools,
        "mock-model".into(),
        None,
        AgentLoopConfig::default(),
        hooks,
    )
}

fn user_text_in_messages(messages: &[Message], text: &str) -> bool {
    messages.iter().any(|m| match m {
        Message::User(u) => u
            .content
            .iter()
            .any(|c| matches!(c, InputContent::Text { text: t } if t == text)),
        _ => false,
    })
}

// ---------------------------------------------------------------------------
// Test 1: after_tool_call receives AfterToolCallContext
// ---------------------------------------------------------------------------

#[tokio::test]
async fn after_tool_call_receives_context() {
    let provider = RecordingProvider::new(vec![
        tool_call_response("c1", "echo", r#"{"text":"hello"}"#),
        text_response("done"),
    ]);

    let hooks = RecordingHooks::new(false);
    let after_calls = hooks.after_calls.clone();

    let mut agent = make_agent(provider, vec![Box::new(EchoTool)], Box::new(hooks));
    agent.prompt("test").await.unwrap();

    let calls = after_calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "after_tool_call should be called once");
    assert_eq!(calls[0].tool_call_id, "c1");
    assert_eq!(calls[0].tool_name, "echo");
    assert!(!calls[0].result.is_error);
}

// ---------------------------------------------------------------------------
// Test 2: after_tool_call Replace modifies tool result
// ---------------------------------------------------------------------------

#[tokio::test]
async fn after_tool_call_replace_result() {
    let provider = RecordingProvider::new(vec![
        tool_call_response("c1", "echo", r#"{"text":"hello"}"#),
        text_response("done"),
    ]);

    let mut agent = make_agent(provider, vec![Box::new(EchoTool)], Box::new(ReplacingHooks));
    let result = agent.prompt("test").await.unwrap();

    let tool_result = result
        .iter()
        .find_map(|m| match m {
            AgentMessage::Llm(Message::ToolResult(tr)) => Some(tr.clone()),
            _ => None,
        })
        .expect("should have a tool result");

    match &tool_result.content[0] {
        OutputContent::Text { text } => assert_eq!(text, "replaced: 1"),
        _ => panic!("expected text content"),
    }
}

// ---------------------------------------------------------------------------
// Test 3: should_stop_after_turn receives ShouldStopAfterTurnContext
// ---------------------------------------------------------------------------

#[tokio::test]
async fn should_stop_receives_context() {
    let provider = RecordingProvider::new(vec![text_response("hello")]);

    let hooks = RecordingHooks::new(false);
    let stop_calls = hooks.stop_calls.clone();

    let mut agent = make_agent(provider, vec![], Box::new(hooks));
    agent.prompt("test").await.unwrap();

    let calls = stop_calls.lock().unwrap();
    assert!(!calls.is_empty(), "should_stop_after_turn should be called");
    assert!(
        !calls[0].messages.is_empty(),
        "context should have messages"
    );
}

// ---------------------------------------------------------------------------
// Test 4: steering queue delivers before next request
// ---------------------------------------------------------------------------

#[tokio::test]
async fn steering_queue_delivered_before_next_request() {
    let provider = RecordingProvider::new(vec![
        tool_call_response("c1", "echo", r#"{"text":"hello"}"#),
        text_response("done"),
    ]);
    let received = provider.received_messages.clone();

    let hooks = RecordingHooks::new(false);

    let mut agent = make_agent(provider, vec![Box::new(EchoTool)], Box::new(hooks));
    agent.steer("focus on quality".into());
    agent.prompt("test").await.unwrap();

    let msgs = received.lock().unwrap();
    assert_eq!(msgs.len(), 2, "provider should be called twice");
    assert!(
        user_text_in_messages(&msgs[1], "focus on quality"),
        "second provider call should include steering message"
    );
}

// ---------------------------------------------------------------------------
// Test 5: follow-up queue delivers when agent would stop
// ---------------------------------------------------------------------------

#[tokio::test]
async fn follow_up_queue_delivered_when_would_stop() {
    let provider = RecordingProvider::new(vec![text_response("hello"), text_response("more")]);
    let received = provider.received_messages.clone();

    let hooks = RecordingHooks::new(false);

    let mut agent = make_agent(provider, vec![], Box::new(hooks));
    agent.follow_up("tell me more".into());
    agent.prompt("test").await.unwrap();

    let msgs = received.lock().unwrap();
    assert_eq!(msgs.len(), 2, "provider should be called twice");
    assert!(
        user_text_in_messages(&msgs[1], "tell me more"),
        "second provider call should include follow-up message"
    );
}

// ---------------------------------------------------------------------------
// Test 6: should_stop true prevents queue polling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn should_stop_prevents_queue_polling() {
    let provider = RecordingProvider::new(vec![tool_call_response(
        "c1",
        "echo",
        r#"{"text":"hello"}"#,
    )]);
    let received = provider.received_messages.clone();

    let hooks = RecordingHooks::new(true);

    let mut agent = make_agent(provider, vec![Box::new(EchoTool)], Box::new(hooks));
    agent.steer("should not be delivered".into());
    agent.prompt("test").await.unwrap();

    let msgs = received.lock().unwrap();
    assert_eq!(msgs.len(), 1, "provider should only be called once");
}

// ---------------------------------------------------------------------------
// Test 7: QueueUpdate event emitted
// ---------------------------------------------------------------------------

#[tokio::test]
async fn queue_update_event_emitted() {
    let provider = RecordingProvider::new(vec![
        tool_call_response("c1", "echo", r#"{"text":"hello"}"#),
        text_response("done"),
    ]);

    let hooks = RecordingHooks::new(false);
    type QueueData = (Vec<String>, Vec<String>);
    let queue_events: Arc<Mutex<Vec<QueueData>>> = Arc::new(Mutex::new(Vec::new()));
    let queue_events_clone = queue_events.clone();

    let mut agent = make_agent(provider, vec![Box::new(EchoTool)], Box::new(hooks));
    agent.steer("redirect".into());
    agent.subscribe(Box::new(move |e| {
        if let AgentEvent::QueueUpdate {
            steering,
            follow_up,
        } = e
        {
            queue_events_clone
                .lock()
                .unwrap()
                .push((steering.clone(), follow_up.clone()));
        }
    }));
    agent.prompt("test").await.unwrap();

    let updates = queue_events.lock().unwrap();
    assert!(!updates.is_empty(), "should emit QueueUpdate event");
    assert!(
        updates[0].0.contains(&"redirect".to_owned()),
        "event should contain steering message"
    );
}
