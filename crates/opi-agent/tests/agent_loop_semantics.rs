//! Regression tests for agent loop semantics:
//!   H3 — batch parallel vs serial execution
//!   H4 — terminate flag (all vs partial)
//!   H5 — prepare_next_turn message injection
//!   M1 — should_stop_after_turn receives current-turn tool_results only

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::stream;
use opi_agent::event::AgentEvent;
use opi_agent::hooks::{
    AfterToolCallContext, AfterToolCallResult, AgentHooks, BeforeToolCallContext,
    BeforeToolCallResult, PrepareNextTurnContext, ShouldStopAfterTurnContext,
};
use opi_agent::loop_types::{AgentError, AgentLoopConfig, AgentLoopContext, AgentLoopTurnUpdate};
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
    call_count: Arc<Mutex<usize>>,
}

impl RecordingProvider {
    fn new(responses: Vec<Vec<AssistantStreamEvent>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            call_count: Arc::new(Mutex::new(0)),
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
    fn stream(&self, _request: Request) -> EventStream {
        *self.call_count.lock().unwrap() += 1;
        let events = self.responses.lock().unwrap().remove(0);
        Box::pin(stream::iter(events.into_iter().map(Ok::<_, ProviderError>)))
    }
}

// ---------------------------------------------------------------------------
// Order-logging tool (configurable execution mode and terminate flag)
// ---------------------------------------------------------------------------

struct OrderTool {
    name: String,
    log: Arc<Mutex<Vec<String>>>,
    mode: ExecutionMode,
    terminate: bool,
}

impl OrderTool {
    fn new(name: &str, log: Arc<Mutex<Vec<String>>>, mode: ExecutionMode, terminate: bool) -> Self {
        Self {
            name: name.into(),
            log,
            mode,
            terminate,
        }
    }
}

impl Tool for OrderTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: self.name.clone(),
            description: format!("order tool: {}", self.name),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "arg": { "type": "string" } },
                "required": ["arg"]
            }),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        _args: serde_json::Value,
        _signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        let log = self.log.clone();
        let name = self.name.clone();
        let terminate = self.terminate;
        Box::pin(async move {
            log.lock().unwrap().push(format!("{name}:start"));
            tokio::time::sleep(Duration::from_millis(5)).await;
            log.lock().unwrap().push(format!("{name}:end"));
            Ok(ToolResult {
                content: vec![OutputContent::Text {
                    text: format!("{name}:ok"),
                }],
                details: None,
                is_error: false,
                terminate,
            })
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        self.mode
    }
}

// ---------------------------------------------------------------------------
// Simple echo tool
// ---------------------------------------------------------------------------

struct EchoTool;

impl Tool for EchoTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "echo".into(),
            description: "echo".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "arg": { "type": "string" } },
                "required": ["arg"]
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
        let text = args["arg"].as_str().unwrap_or_default().to_owned();
        Box::pin(async move {
            Ok(ToolResult {
                content: vec![OutputContent::Text { text }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Hook implementations
// ---------------------------------------------------------------------------

struct MinimalHooks;

impl AgentHooks for MinimalHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(messages
            .iter()
            .filter_map(|m| match m {
                AgentMessage::Llm(msg) => Some(msg.clone()),
                _ => None,
            })
            .collect())
    }

    fn should_stop_after_turn(
        &self,
        _: ShouldStopAfterTurnContext,
    ) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        Box::pin(async { false })
    }

    fn before_tool_call(
        &self,
        _: BeforeToolCallContext,
    ) -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>> {
        Box::pin(async { BeforeToolCallResult::Allow })
    }

    fn after_tool_call(
        &self,
        _: AfterToolCallContext,
    ) -> Pin<Box<dyn Future<Output = AfterToolCallResult> + Send>> {
        Box::pin(async { AfterToolCallResult::Keep })
    }
}

struct RecordingStopHooks {
    stop_contexts: Arc<Mutex<Vec<ShouldStopAfterTurnContext>>>,
}

impl RecordingStopHooks {
    fn new() -> Self {
        Self {
            stop_contexts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl AgentHooks for RecordingStopHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(messages
            .iter()
            .filter_map(|m| match m {
                AgentMessage::Llm(msg) => Some(msg.clone()),
                _ => None,
            })
            .collect())
    }

    fn should_stop_after_turn(
        &self,
        ctx: ShouldStopAfterTurnContext,
    ) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        let stop_contexts = self.stop_contexts.clone();
        Box::pin(async move {
            stop_contexts.lock().unwrap().push(ctx);
            false
        })
    }

    fn before_tool_call(
        &self,
        _: BeforeToolCallContext,
    ) -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>> {
        Box::pin(async { BeforeToolCallResult::Allow })
    }

    fn after_tool_call(
        &self,
        _: AfterToolCallContext,
    ) -> Pin<Box<dyn Future<Output = AfterToolCallResult> + Send>> {
        Box::pin(async { AfterToolCallResult::Keep })
    }
}

struct InjectingHooks {
    inject_on_turn: u32,
    inject_text: String,
}

impl InjectingHooks {
    fn new(inject_on_turn: u32, inject_text: String) -> Self {
        Self {
            inject_on_turn,
            inject_text,
        }
    }
}

impl AgentHooks for InjectingHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(messages
            .iter()
            .filter_map(|m| match m {
                AgentMessage::Llm(msg) => Some(msg.clone()),
                _ => None,
            })
            .collect())
    }

    fn should_stop_after_turn(
        &self,
        _: ShouldStopAfterTurnContext,
    ) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        Box::pin(async { false })
    }

    fn before_tool_call(
        &self,
        _: BeforeToolCallContext,
    ) -> Pin<Box<dyn Future<Output = BeforeToolCallResult> + Send>> {
        Box::pin(async { BeforeToolCallResult::Allow })
    }

    fn prepare_next_turn(
        &self,
        ctx: PrepareNextTurnContext,
    ) -> Pin<Box<dyn Future<Output = Option<AgentLoopTurnUpdate>> + Send>> {
        let text = self.inject_text.clone();
        let inject_on_turn = self.inject_on_turn;
        Box::pin(async move {
            if ctx.turn == inject_on_turn {
                Some(AgentLoopTurnUpdate {
                    extra_messages: vec![AgentMessage::Llm(Message::User(
                        opi_ai::message::UserMessage {
                            content: vec![InputContent::Text { text }],
                            timestamp_ms: 0,
                        },
                    ))],
                })
            } else {
                None
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn base_msg() -> AssistantMessage {
    AssistantMessage {
        content: vec![],
        api: opi_ai::ApiKind::Anthropic,
        provider: "recording".into(),
        model: "mock".into(),
        response_model: None,
        response_id: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp_ms: 0,
    }
}

fn text_response(text: &str) -> Vec<AssistantStreamEvent> {
    let mut partial = base_msg();
    partial
        .content
        .push(AssistantContent::Text { text: text.into() });
    vec![
        AssistantStreamEvent::Start {
            partial: base_msg(),
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

fn multi_tool_call_response(calls: Vec<(&str, &str, &str)>) -> Vec<AssistantStreamEvent> {
    let mut partial = base_msg();
    for (id, name, args) in &calls {
        let tc = ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args.to_string(),
        };
        partial
            .content
            .push(AssistantContent::ToolCall { tool_call: tc });
    }
    partial.stop_reason = StopReason::ToolUse;

    let mut events = vec![AssistantStreamEvent::Start {
        partial: base_msg(),
    }];
    for (id, name, args) in &calls {
        let tc = ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args.to_string(),
        };
        events.push(AssistantStreamEvent::ToolCallEnd {
            content_index: 0,
            tool_call: tc,
            partial: partial.clone(),
        });
    }
    events.push(AssistantStreamEvent::Done {
        reason: StopReason::ToolUse,
        message: partial,
    });
    events
}

fn make_context(provider: Box<dyn Provider>, tools: Vec<Box<dyn Tool>>) -> AgentLoopContext {
    AgentLoopContext {
        provider,
        tools,
        messages: vec![AgentMessage::Llm(Message::User(
            opi_ai::message::UserMessage {
                content: vec![InputContent::Text {
                    text: "test".into(),
                }],
                timestamp_ms: 0,
            },
        ))],
        model: "mock".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: None,
        diagnostic_sink: None,
    }
}

fn noop_sink() -> Box<dyn Fn(AgentEvent) + Send + Sync> {
    Box::new(|_| {})
}

// ---------------------------------------------------------------------------
// H3: Batch parallel execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn h3_parallel_tools_both_execute() {
    let log = Arc::new(Mutex::new(Vec::new()));

    let provider = RecordingProvider::new(vec![
        multi_tool_call_response(vec![
            ("c1", "tool_a", r#"{"arg":"a"}"#),
            ("c2", "tool_b", r#"{"arg":"b"}"#),
        ]),
        text_response("done"),
    ]);

    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(OrderTool::new(
            "tool_a",
            log.clone(),
            ExecutionMode::Parallel,
            false,
        )),
        Box::new(OrderTool::new(
            "tool_b",
            log.clone(),
            ExecutionMode::Parallel,
            false,
        )),
    ];

    let context = make_context(Box::new(provider), tools);
    let hooks = MinimalHooks;

    opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        noop_sink(),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let entries = log.lock().unwrap();
    assert!(
        entries.contains(&"tool_a:start".to_string()),
        "tool_a should execute"
    );
    assert!(
        entries.contains(&"tool_b:start".to_string()),
        "tool_b should execute"
    );
}

#[tokio::test]
async fn h3_sequential_tool_forces_serial_execution() {
    let log = Arc::new(Mutex::new(Vec::new()));

    let provider = RecordingProvider::new(vec![
        multi_tool_call_response(vec![
            ("c1", "tool_a", r#"{"arg":"a"}"#),
            ("c2", "tool_b", r#"{"arg":"b"}"#),
        ]),
        text_response("done"),
    ]);

    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(OrderTool::new(
            "tool_a",
            log.clone(),
            ExecutionMode::Sequential,
            false,
        )),
        Box::new(OrderTool::new(
            "tool_b",
            log.clone(),
            ExecutionMode::Parallel,
            false,
        )),
    ];

    let context = make_context(Box::new(provider), tools);
    let hooks = MinimalHooks;

    opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        noop_sink(),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let entries = log.lock().unwrap();
    let a_end = entries
        .iter()
        .position(|e| e == "tool_a:end")
        .expect("tool_a should end");
    let b_start = entries
        .iter()
        .position(|e| e == "tool_b:start")
        .expect("tool_b should start");
    assert!(
        a_end < b_start,
        "sequential batch: tool_a must complete before tool_b starts, got: {entries:?}"
    );
}

// ---------------------------------------------------------------------------
// H4: Terminate flags
// ---------------------------------------------------------------------------

#[tokio::test]
async fn h4_all_terminate_stops_early() {
    let log = Arc::new(Mutex::new(Vec::new()));

    let provider = RecordingProvider::new(vec![multi_tool_call_response(vec![
        ("c1", "tool_a", r#"{"arg":"a"}"#),
        ("c2", "tool_b", r#"{"arg":"b"}"#),
    ])]);
    let call_count = provider.call_count.clone();

    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(OrderTool::new(
            "tool_a",
            log.clone(),
            ExecutionMode::Parallel,
            true,
        )),
        Box::new(OrderTool::new(
            "tool_b",
            log.clone(),
            ExecutionMode::Parallel,
            true,
        )),
    ];

    let context = make_context(Box::new(provider), tools);
    let hooks = MinimalHooks;

    let result = opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        noop_sink(),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "provider should be called once when all tools terminate"
    );
    assert!(
        result.len() >= 3,
        "should have user + assistant + tool results"
    );
}

#[tokio::test]
async fn h4_partial_terminate_continues() {
    let log = Arc::new(Mutex::new(Vec::new()));

    let provider = RecordingProvider::new(vec![
        multi_tool_call_response(vec![
            ("c1", "tool_a", r#"{"arg":"a"}"#),
            ("c2", "tool_b", r#"{"arg":"b"}"#),
        ]),
        text_response("done"),
    ]);
    let call_count = provider.call_count.clone();

    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(OrderTool::new(
            "tool_a",
            log.clone(),
            ExecutionMode::Parallel,
            true,
        )),
        Box::new(OrderTool::new(
            "tool_b",
            log.clone(),
            ExecutionMode::Parallel,
            false,
        )),
    ];

    let context = make_context(Box::new(provider), tools);
    let hooks = MinimalHooks;

    opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        noop_sink(),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    assert_eq!(
        *call_count.lock().unwrap(),
        2,
        "provider should be called twice when only some tools terminate"
    );
}

// ---------------------------------------------------------------------------
// H5: prepare_next_turn injection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn h5_prepare_next_turn_injects_and_continues() {
    let provider = RecordingProvider::new(vec![text_response("first"), text_response("second")]);
    let call_count = provider.call_count.clone();

    let hooks = InjectingHooks::new(1, "injected context".into());

    let context = make_context(Box::new(provider), vec![]);

    let result = opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        noop_sink(),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    assert_eq!(
        *call_count.lock().unwrap(),
        2,
        "provider should be called twice after prepare_next_turn injection"
    );

    let has_injected = result.iter().any(|m| match m {
        AgentMessage::Llm(Message::User(u)) => u
            .content
            .iter()
            .any(|c| matches!(c, InputContent::Text { text } if text == "injected context")),
        _ => false,
    });
    assert!(has_injected, "injected message should appear in result");
}

// ---------------------------------------------------------------------------
// M1: should_stop_after_turn receives current-turn tool_results only
// ---------------------------------------------------------------------------

#[tokio::test]
async fn m1_tool_results_scoped_to_current_turn() {
    let provider = RecordingProvider::new(vec![
        multi_tool_call_response(vec![("c1", "echo", r#"{"arg":"first"}"#)]),
        multi_tool_call_response(vec![("c2", "echo", r#"{"arg":"second"}"#)]),
        text_response("done"),
    ]);

    let hooks = RecordingStopHooks::new();
    let stop_contexts = hooks.stop_contexts.clone();

    let context = make_context(Box::new(provider), vec![Box::new(EchoTool)]);

    opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        noop_sink(),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let contexts = stop_contexts.lock().unwrap();
    let with_tools: Vec<_> = contexts
        .iter()
        .filter(|c| !c.tool_results.is_empty())
        .collect();

    assert!(
        with_tools.len() >= 2,
        "at least 2 turns should have tool_results"
    );

    // Turn 1: only c1
    assert_eq!(
        with_tools[0].tool_results.len(),
        1,
        "turn 1 should have exactly 1 tool_result"
    );
    assert_eq!(with_tools[0].tool_results[0].tool_call_id, "c1");

    // Turn 2: only c2 (not accumulated from turn 1)
    assert_eq!(
        with_tools[1].tool_results.len(),
        1,
        "turn 2 should have exactly 1 tool_result (current turn only)"
    );
    assert_eq!(with_tools[1].tool_results[0].tool_call_id, "c2");
}
