//! Behavioral tests for the Agent wrapper (task 1.7).
//!
//! DoD: "prompt, continue, abort, subscribe tested"

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use futures_util::stream;
use opi_agent::agent::Agent;
use opi_agent::event::AgentEvent;
use opi_agent::hooks::{
    AgentHooks, BeforeToolCallContext, BeforeToolCallResult, ShouldStopAfterTurnContext,
};
use opi_agent::loop_types::{AgentError, AgentLoopConfig};
use opi_agent::message::AgentMessage;
use opi_ai::message::{AssistantContent, AssistantMessage, InputContent, Message};
use opi_ai::provider::{EventStream, Provider, ProviderError, Request};
use opi_ai::stream::{AssistantStreamEvent, StopReason, Usage};

// ---------------------------------------------------------------------------
// Mock provider (reused from agent_loop_mock)
// ---------------------------------------------------------------------------

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
// Helpers
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

// ---------------------------------------------------------------------------
// Test 1: prompt sends user message and returns result
// ---------------------------------------------------------------------------

#[tokio::test]
async fn prompt_sends_user_message_and_returns_result() {
    let provider = MockProvider::new("mock", vec![text_response("Hello!")]);

    let mut agent = Agent::new(
        Box::new(provider),
        vec![],
        "mock-model".into(),
        None,
        AgentLoopConfig::default(),
        Box::new(TestHooks),
    );

    let result = agent.prompt("Hi there").await.unwrap();

    // Should contain: user message + assistant message
    assert!(
        result.len() >= 2,
        "expected at least 2 messages, got {}",
        result.len()
    );

    // First message should be the user message
    if let AgentMessage::Llm(Message::User(msg)) = &result[0] {
        match &msg.content[0] {
            InputContent::Text { text } => assert_eq!(text, "Hi there"),
            _ => panic!("expected text content"),
        }
    } else {
        panic!("first message should be user message");
    }
}

// ---------------------------------------------------------------------------
// Test 2: prompt accumulates state across calls
// ---------------------------------------------------------------------------

#[tokio::test]
async fn prompt_accumulates_state_across_calls() {
    let provider = MockProvider::new(
        "mock",
        vec![text_response("First"), text_response("Second")],
    );

    let mut agent = Agent::new(
        Box::new(provider),
        vec![],
        "mock-model".into(),
        None,
        AgentLoopConfig::default(),
        Box::new(TestHooks),
    );

    let r1 = agent.prompt("Hello").await.unwrap();
    assert!(r1.len() >= 2);

    let r2 = agent.prompt("World").await.unwrap();
    // Second call should include messages from first call
    // r2 includes: [user1, assistant1, user2, assistant2]
    assert!(
        r2.len() >= 4,
        "expected at least 4 messages after two prompts, got {}",
        r2.len()
    );
}

// ---------------------------------------------------------------------------
// Test 3: continue appends and runs loop
// ---------------------------------------------------------------------------

#[tokio::test]
async fn continue_appends_message_and_runs_loop() {
    let provider = MockProvider::new(
        "mock",
        vec![text_response("First"), text_response("Continued")],
    );

    let mut agent = Agent::new(
        Box::new(provider),
        vec![],
        "mock-model".into(),
        None,
        AgentLoopConfig::default(),
        Box::new(TestHooks),
    );

    let r1 = agent.prompt("Hello").await.unwrap();
    assert!(r1.len() >= 2);

    let r2 = agent.continue_("Tell me more").await.unwrap();
    assert!(
        r2.len() >= 4,
        "expected at least 4 messages after prompt+continue, got {}",
        r2.len()
    );
}

// ---------------------------------------------------------------------------
// Test 4: abort cancels running loop
// ---------------------------------------------------------------------------

#[tokio::test]
async fn abort_cancels_running_loop() {
    // Provider that yields Start then blocks until cancelled
    struct BlockingProvider;

    impl Provider for BlockingProvider {
        fn id(&self) -> &str {
            "blocking"
        }

        fn models(&self) -> &[opi_ai::provider::ModelInfo] {
            &[]
        }

        fn stream(&self, request: Request) -> EventStream {
            let cancel = request.cancel;
            // Yield Start event, then wait for cancellation to end the stream
            Box::pin(
                futures_util::stream::once(async move {
                    Ok(AssistantStreamEvent::Start {
                        partial: base_assistant(),
                    })
                })
                .chain(futures_util::stream::unfold((), move |()| {
                    let cancel = cancel.clone();
                    async move {
                        cancel.cancelled().await;
                        None // end the stream when cancelled
                    }
                })),
            )
        }
    }

    let mut agent = Agent::new(
        Box::new(BlockingProvider),
        vec![],
        "mock-model".into(),
        None,
        AgentLoopConfig::default(),
        Box::new(TestHooks),
    );

    // Get cancel token before moving agent into spawned task
    let token = agent.cancel_token();

    let handle = tokio::spawn(async move { agent.prompt("Hello").await });

    // Let the prompt start and enter the stream loop
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Cancel via the external token
    token.cancel();

    let result = handle.await.unwrap();

    assert!(
        matches!(result, Err(AgentError::Cancelled)),
        "expected Cancelled error, got {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Test 5: subscribe receives events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_receives_events() {
    let provider = MockProvider::new("mock", vec![text_response("Response")]);

    let collected: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let collected_clone = collected.clone();

    let mut agent = Agent::new(
        Box::new(provider),
        vec![],
        "mock-model".into(),
        None,
        AgentLoopConfig::default(),
        Box::new(TestHooks),
    );

    agent.subscribe(Box::new(move |event| {
        let name = match event {
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
        };
        collected_clone.lock().unwrap().push(name.to_owned());
    }));

    let result = agent.prompt("Hello").await.unwrap();
    assert!(result.len() >= 2);

    let events = collected.lock().unwrap();
    assert!(
        events.contains(&"AgentStart".to_owned()),
        "subscriber should receive AgentStart, got {:?}",
        *events
    );
    assert!(
        events.contains(&"AgentEnd".to_owned()),
        "subscriber should receive AgentEnd, got {:?}",
        *events
    );
}
