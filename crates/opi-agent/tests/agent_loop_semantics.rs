//! Regression tests for agent loop semantics:
//!   H3 — batch parallel vs serial execution
//!   H4 — terminate flag (all vs partial)
//!   H5 — prepare_next_turn message injection
//!   M1 — should_stop_after_turn receives current-turn tool_results only

use std::collections::VecDeque;
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
        trace: None,
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

// ---------------------------------------------------------------------------
// Phase 8: runtime event order and queue-polling contract (task 8.1)
//
// Pins the documented agent_start -> agent_end order through
// opi_agent::agent_loop: AgentStart/AgentEnd bracket the run; per turn the
// order is TurnStart -> assistant message events -> tool execution -> TurnEnd
// -> should_stop_after_turn -> prepare_next_turn -> steering/follow-up polling.
// ---------------------------------------------------------------------------

/// Reduce an `AgentEvent` to a stable kind label for sequence assertions.
fn event_kind(e: &AgentEvent) -> &'static str {
    match e {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_execution_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
        AgentEvent::QueueUpdate { .. } => "queue_update",
        AgentEvent::AutoRetryStart { .. } => "auto_retry_start",
        AgentEvent::AutoRetryEnd { .. } => "auto_retry_end",
        AgentEvent::CompactionStart { .. } => "compaction_start",
        AgentEvent::CompactionEnd { .. } => "compaction_end",
        AgentEvent::SessionPersistError { .. } => "session_persist_error",
        // AgentEvent is #[non_exhaustive]; future variants collapse to "other".
        _ => "other",
    }
}

/// Build an event sink that records the kind of every emitted `AgentEvent`.
fn recording_sink(log: Arc<Mutex<Vec<String>>>) -> Box<dyn Fn(AgentEvent) + Send + Sync> {
    Box::new(move |e| {
        log.lock().unwrap().push(event_kind(&e).to_string());
    })
}

/// Position of the first occurrence of `kind` in the recorded sequence.
fn event_pos(seq: &[String], kind: &str) -> usize {
    seq.iter()
        .position(|s| s == kind)
        .unwrap_or_else(|| panic!("event `{kind}` not present in sequence: {seq:?}"))
}

/// True if `m` is a user text message equal to `target`.
fn matches_user_text(m: &AgentMessage, target: &str) -> bool {
    matches!(
        m,
        AgentMessage::Llm(Message::User(u))
            if u.content.iter().any(|c| matches!(
                c,
                InputContent::Text { text } if text == target
            ))
    )
}

/// Hooks that terminate the run on the first `should_stop_after_turn` and
/// record every `prepare_next_turn` invocation. Models a compaction coordinator
/// that requests a stop before the next turn.
struct TerminalStopHooks {
    prepare_calls: Arc<Mutex<Vec<u32>>>,
}

impl AgentHooks for TerminalStopHooks {
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
        Box::pin(async { true })
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

    fn prepare_next_turn(
        &self,
        ctx: PrepareNextTurnContext,
    ) -> Pin<Box<dyn Future<Output = Option<AgentLoopTurnUpdate>> + Send>> {
        let prepare_calls = self.prepare_calls.clone();
        Box::pin(async move {
            prepare_calls.lock().unwrap().push(ctx.turn);
            None
        })
    }
}

// DoD: no-tool run emits the documented order with no tool events.
#[tokio::test]
async fn phase8_event_order_no_tool_run() {
    let provider = RecordingProvider::new(vec![text_response("done")]);
    let call_count = provider.call_count.clone();

    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = recording_sink(events.clone());

    let context = make_context(Box::new(provider), vec![]);
    let hooks = MinimalHooks;

    opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        sink,
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let seq = events.lock().unwrap();
    assert_eq!(
        seq.first().map(String::as_str),
        Some("agent_start"),
        "AgentStart must be first: {seq:?}"
    );
    assert_eq!(
        seq.last().map(String::as_str),
        Some("agent_end"),
        "AgentEnd must be last: {seq:?}"
    );
    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "no-tool run makes exactly one provider call"
    );
    assert!(
        !seq.contains(&"tool_execution_start".to_string()),
        "no tool events in a no-tool run: {seq:?}"
    );
    assert!(!seq.contains(&"tool_execution_end".to_string()));
    assert_eq!(
        seq.iter().filter(|s| s.as_str() == "turn_start").count(),
        1,
        "no-tool run is a single turn"
    );
    assert_eq!(seq.iter().filter(|s| s.as_str() == "turn_end").count(), 1);
    // turn_start < message_end < turn_end within the only turn
    assert!(event_pos(&seq, "turn_start") < event_pos(&seq, "message_end"));
    assert!(event_pos(&seq, "message_end") < event_pos(&seq, "turn_end"));
}

// DoD: one-tool run brackets tool execution inside the first turn and runs a
// second turn for the final assistant response.
#[tokio::test]
async fn phase8_event_order_one_tool_run() {
    let provider = RecordingProvider::new(vec![
        multi_tool_call_response(vec![("c1", "echo", r#"{"arg":"x"}"#)]),
        text_response("done"),
    ]);
    let call_count = provider.call_count.clone();

    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = recording_sink(events.clone());

    let context = make_context(Box::new(provider), vec![Box::new(EchoTool)]);
    let hooks = MinimalHooks;

    opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        sink,
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let seq = events.lock().unwrap();
    assert_eq!(seq.first().map(String::as_str), Some("agent_start"));
    assert_eq!(seq.last().map(String::as_str), Some("agent_end"));
    assert_eq!(
        *call_count.lock().unwrap(),
        2,
        "one-tool run makes two provider calls"
    );
    assert_eq!(
        seq.iter().filter(|s| s.as_str() == "turn_start").count(),
        2,
        "one-tool run spans two turns"
    );
    assert_eq!(seq.iter().filter(|s| s.as_str() == "turn_end").count(), 2);
    assert_eq!(
        seq.iter()
            .filter(|s| s.as_str() == "tool_execution_start")
            .count(),
        1
    );
    assert_eq!(
        seq.iter()
            .filter(|s| s.as_str() == "tool_execution_end")
            .count(),
        1
    );
    // Tool execution sits inside the first turn: after its assistant
    // message_end and before the first turn_end.
    let first_turn_end = event_pos(&seq, "turn_end");
    assert!(event_pos(&seq, "message_end") < event_pos(&seq, "tool_execution_start"));
    assert!(event_pos(&seq, "tool_execution_start") < first_turn_end);
    assert!(event_pos(&seq, "tool_execution_end") < first_turn_end);
}

// DoD: prepare_next_turn injection is applied before follow-up polling. With a
// turn-1 injection and a queued follow-up, the injected message reaches the
// provider strictly before the follow-up message.
#[tokio::test]
async fn phase8_event_order_prepare_next_turn_injection() {
    let provider = RecordingProvider::new(vec![
        text_response("first"),
        text_response("second"),
        text_response("third"),
    ]);
    let call_count = provider.call_count.clone();

    let follow_up_queue: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::from(vec![
        "follow-up-msg".to_string(),
    ])));

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![],
        messages: vec![AgentMessage::Llm(Message::User(
            opi_ai::message::UserMessage {
                content: vec![InputContent::Text {
                    text: "seed".into(),
                }],
                timestamp_ms: 0,
            },
        ))],
        model: "mock".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: Some(follow_up_queue),
        diagnostic_sink: None,
        trace: None,
    };
    let hooks = InjectingHooks::new(1, "injected-context".into());

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
        3,
        "injection then follow-up yield three provider calls"
    );
    let injected_at = result
        .iter()
        .position(|m| matches_user_text(m, "injected-context"))
        .expect("injected message must be present");
    let follow_up_at = result
        .iter()
        .position(|m| matches_user_text(m, "follow-up-msg"))
        .expect("follow-up message must be present");
    assert!(
        injected_at < follow_up_at,
        "prepare_next_turn injection must precede follow-up delivery"
    );
}

// DoD: a terminal should_stop_after_turn stops the run before prepare_next_turn
// runs and before any queue is polled.
#[tokio::test]
async fn phase8_event_order_terminal_should_stop_skips_prepare_next_turn() {
    let provider = RecordingProvider::new(vec![text_response("only")]);
    let call_count = provider.call_count.clone();

    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = recording_sink(events.clone());

    let hooks = TerminalStopHooks {
        prepare_calls: Arc::new(Mutex::new(Vec::new())),
    };
    let prepare_calls = hooks.prepare_calls.clone();

    let context = make_context(Box::new(provider), vec![]);

    opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        sink,
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let seq = events.lock().unwrap();
    assert_eq!(seq.first().map(String::as_str), Some("agent_start"));
    assert_eq!(seq.last().map(String::as_str), Some("agent_end"));
    assert_eq!(
        *call_count.lock().unwrap(),
        1,
        "terminal stop makes exactly one provider call"
    );
    assert!(
        prepare_calls.lock().unwrap().is_empty(),
        "prepare_next_turn must NOT run after a terminal should_stop_after_turn"
    );
    assert!(
        !seq.contains(&"queue_update".to_string()),
        "no queue polling after a terminal should_stop_after_turn: {seq:?}"
    );
}
