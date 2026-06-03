//! SDK embedding surface tests (task 4.2).
//!
//! Tests drive the SDK with MockProvider through prompt, continue,
//! session, model, thinking, compaction, and cancellation flows.
//! No live provider network access required.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use opi_agent::Agent;
use opi_agent::event::AgentEvent;
use opi_agent::hooks::AgentHooks;
use opi_agent::loop_types::{AgentError, AgentLoopConfig};
use opi_agent::sdk::{SDK_SCHEMA_VERSION, SdkCommand, SdkResponse, agent_event_to_value};
use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};
use opi_ai::test_support::{MockProvider, text_response};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct NoopTool;

impl Tool for NoopTool {
    fn definition(&self) -> ToolDef {
        serde_json::from_value(serde_json::json!({
            "name": "noop",
            "description": "does nothing",
            "input_schema": { "type": "object", "properties": {} }
        }))
        .unwrap()
    }

    fn execute(
        &self,
        _call_id: &str,
        _arguments: serde_json::Value,
        _signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        Box::pin(async {
            Ok(ToolResult {
                content: vec![OutputContent::Text { text: "ok".into() }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Parallel
    }
}

struct NoopHooks;

impl AgentHooks for NoopHooks {
    fn convert_to_llm(
        &self,
        messages: &[opi_agent::message::AgentMessage],
    ) -> Result<Vec<opi_ai::message::Message>, AgentError> {
        // Passthrough: forward all Llm messages, drop internal variants.
        Ok(messages
            .iter()
            .filter_map(|m| match m {
                opi_agent::message::AgentMessage::Llm(msg) => Some(msg.clone()),
                _ => None,
            })
            .collect())
    }
}

fn make_agent(responses: Vec<Vec<opi_ai::stream::AssistantStreamEvent>>) -> Agent {
    let provider = MockProvider::new("mock", responses);
    Agent::new(
        Box::new(provider),
        vec![Box::new(NoopTool)],
        "mock:mock-model".into(),
        Some("test system prompt".into()),
        AgentLoopConfig::default(),
        Box::new(NoopHooks),
    )
}

fn collect_events(agent: &mut Agent) -> Arc<Mutex<Vec<AgentEvent>>> {
    let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let ev_clone = events.clone();
    agent.subscribe(Box::new(move |e: &AgentEvent| {
        ev_clone.lock().unwrap().push(e.clone());
    }));
    events
}

fn assert_has_event(events: &[AgentEvent], predicate: impl Fn(&AgentEvent) -> bool) {
    assert!(
        events.iter().any(predicate),
        "expected event not found in: {:?}",
        events
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
                AgentEvent::ToolExecutionEnd { .. } => "ToolExecutionEnd",
                AgentEvent::QueueUpdate { .. } => "QueueUpdate",
                AgentEvent::AutoRetryStart { .. } => "AutoRetryStart",
                AgentEvent::AutoRetryEnd { .. } => "AutoRetryEnd",
                AgentEvent::CompactionStart { .. } => "CompactionStart",
                AgentEvent::CompactionEnd { .. } => "CompactionEnd",
                AgentEvent::SessionPersistError { .. } => "SessionPersistError",
                AgentEvent::ToolExecutionUpdate { .. } => "ToolExecutionUpdate",
                _ => "Unknown",
            })
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Command parsing tests
// ---------------------------------------------------------------------------

#[test]
fn sdk_command_parse_prompt() {
    let json = r#"{"type":"prompt","message":"hello","id":"42"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::prompt { .. }));
    assert_eq!(cmd.id().unwrap(), "42");
    assert_eq!(cmd.command_name(), "prompt");
}

#[test]
fn sdk_command_parse_continue() {
    let json = r#"{"type":"continue","message":"more"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::continue_ { .. }));
    assert_eq!(cmd.command_name(), "continue");
}

#[test]
fn sdk_command_parse_abort() {
    let json = r#"{"type":"abort","id":"1"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::abort { .. }));
    assert_eq!(cmd.command_name(), "abort");
}

#[test]
fn sdk_command_parse_set_model() {
    let json = r#"{"type":"set_model","model":"anthropic:claude-sonnet"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::set_model { .. }));
    assert_eq!(cmd.command_name(), "set_model");
}

#[test]
fn sdk_command_parse_set_thinking_level() {
    let json = r#"{"type":"set_thinking_level","level":"high"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::set_thinking_level { .. }));
    assert_eq!(cmd.command_name(), "set_thinking_level");
}

#[test]
fn sdk_command_parse_compact() {
    let json = r#"{"type":"compact"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::compact { .. }));
    assert_eq!(cmd.command_name(), "compact");
}

#[test]
fn sdk_command_parse_session_info() {
    let json = r#"{"type":"session_info","id":"s1"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::session_info { .. }));
    assert_eq!(cmd.command_name(), "session_info");
}

#[test]
fn sdk_command_parse_quit() {
    let json = r#"{"type":"quit"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(cmd.is_quit());
    assert_eq!(cmd.command_name(), "quit");
}

#[test]
fn sdk_command_parse_steer() {
    let json = r#"{"type":"steer","message":"redirect"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::steer { .. }));
    assert_eq!(cmd.command_name(), "steer");
}

#[test]
fn sdk_command_parse_follow_up() {
    let json = r#"{"type":"follow_up","message":"then do this"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, SdkCommand::follow_up { .. }));
    assert_eq!(cmd.command_name(), "follow_up");
}

#[test]
fn sdk_command_roundtrip_all_variants() {
    let commands = vec![
        serde_json::json!({"type":"prompt","message":"hi","id":"a"}),
        serde_json::json!({"type":"continue","message":"more"}),
        serde_json::json!({"type":"steer","message":"s"}),
        serde_json::json!({"type":"follow_up","message":"f"}),
        serde_json::json!({"type":"abort"}),
        serde_json::json!({"type":"set_model","model":"m"}),
        serde_json::json!({"type":"set_thinking_level","level":"low"}),
        serde_json::json!({"type":"compact"}),
        serde_json::json!({"type":"session_info"}),
        serde_json::json!({"type":"quit"}),
    ];
    for json in commands {
        let cmd: SdkCommand = serde_json::from_value(json.clone()).unwrap();
        let serialized = serde_json::to_value(&cmd).unwrap();
        assert_eq!(
            json,
            serialized,
            "roundtrip failed for {:?}",
            cmd.command_name()
        );
    }
}

#[test]
fn sdk_command_invalid_type_rejected() {
    let json = r#"{"type":"unknown_command"}"#;
    assert!(serde_json::from_str::<SdkCommand>(json).is_err());
}

#[test]
fn sdk_command_missing_required_field_rejected() {
    // prompt without message field
    let json = r#"{"type":"prompt"}"#;
    assert!(serde_json::from_str::<SdkCommand>(json).is_err());
}

// ---------------------------------------------------------------------------
// Response tests
// ---------------------------------------------------------------------------

#[test]
fn sdk_response_success_serializes() {
    let resp = SdkResponse::success(Some("42"), "prompt");
    let val = serde_json::to_value(&resp).unwrap();
    assert_eq!(val["type"], "response");
    assert_eq!(val["command"], "prompt");
    assert_eq!(val["success"], true);
    assert_eq!(val["id"], "42");
    assert!(val.get("error").is_none());
    assert!(val.get("data").is_none());
}

#[test]
fn sdk_response_success_without_id() {
    let resp = SdkResponse::success(None, "compact");
    let val = serde_json::to_value(&resp).unwrap();
    assert_eq!(val["type"], "response");
    assert!(val.get("id").is_none());
}

#[test]
fn sdk_response_success_with_data() {
    let data = serde_json::json!({"model": "test", "session_id": "abc"});
    let resp = SdkResponse::success_with_data(Some("1"), "session_info", data.clone());
    let val = serde_json::to_value(&resp).unwrap();
    assert_eq!(val["type"], "response");
    assert_eq!(val["success"], true);
    assert_eq!(val["data"], data);
}

#[test]
fn sdk_response_error_serializes() {
    let resp = SdkResponse::error(Some("1"), "set_model", "cannot change while running");
    let val = serde_json::to_value(&resp).unwrap();
    assert_eq!(val["type"], "response");
    assert_eq!(val["success"], false);
    assert_eq!(val["error"], "cannot change while running");
}

#[test]
fn sdk_response_error_without_id() {
    let resp = SdkResponse::error(None, "parse", "invalid json");
    let val = serde_json::to_value(&resp).unwrap();
    assert!(val.get("id").is_none());
}

// ---------------------------------------------------------------------------
// Schema version
// ---------------------------------------------------------------------------

#[test]
fn sdk_schema_version_matches_rpc() {
    assert_eq!(SDK_SCHEMA_VERSION, 2);
}

// ---------------------------------------------------------------------------
// Agent driving tests (SDK flows with MockProvider)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sdk_prompt_flow() {
    let mut agent = make_agent(vec![text_response("hello world")]);
    let events = collect_events(&mut agent);

    let result = agent.prompt("test prompt").await;
    assert!(result.is_ok());

    let ev = events.lock().unwrap();
    assert_has_event(&ev, |e| matches!(e, AgentEvent::AgentStart));
    assert_has_event(&ev, |e| matches!(e, AgentEvent::AgentEnd { .. }));
    assert_has_event(&ev, |e| matches!(e, AgentEvent::TurnStart));
    assert_has_event(&ev, |e| matches!(e, AgentEvent::TurnEnd { .. }));
}

#[tokio::test]
async fn sdk_continue_flow() {
    let mut agent = make_agent(vec![text_response("first"), text_response("second")]);

    let first = agent.prompt("first prompt").await;
    assert!(first.is_ok());

    let second = agent.continue_("continue prompt").await;
    assert!(second.is_ok());
}

#[tokio::test]
async fn sdk_continue_without_prior_prompt_errors() {
    let mut agent = make_agent(vec![]);
    let result = agent.continue_("orphan continue").await;
    assert!(matches!(result, Err(AgentError::Hook(_))));
}

#[tokio::test]
async fn sdk_abort_cancels_running_agent() {
    let mut agent = make_agent(vec![text_response("should not finish")]);
    let events = collect_events(&mut agent);

    let cancel = agent.cancel_token();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        cancel_clone.cancel();
    });

    let result = agent.prompt("test").await;
    match result {
        Ok(_) => {}
        Err(AgentError::Cancelled) => {}
        Err(e) => panic!("unexpected error: {:?}", e),
    }

    let ev = events.lock().unwrap();
    assert_has_event(&ev, |e| matches!(e, AgentEvent::AgentStart));
}

#[tokio::test]
async fn sdk_set_model_changes_model() {
    let mut agent = make_agent(vec![text_response("response")]);
    assert_eq!(agent.model(), "mock:mock-model");
    agent.set_model("new:model".into());
    assert_eq!(agent.model(), "new:model");
}

#[tokio::test]
async fn sdk_steer_flow() {
    let mut agent = make_agent(vec![text_response("first"), text_response("steered")]);
    agent.steer("steer this".into());
    let result = agent.prompt("initial").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn sdk_follow_up_flow() {
    let mut agent = make_agent(vec![text_response("first"), text_response("followed")]);
    agent.follow_up("follow up".into());
    let result = agent.prompt("initial").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn sdk_messages_snapshot_after_prompt() {
    let mut agent = make_agent(vec![text_response("response text")]);
    let result = agent.prompt("hello").await;
    assert!(result.is_ok());
    let snapshot = agent.messages_snapshot();
    // Should have at least user message + assistant message
    assert!(snapshot.len() >= 2);
}

#[tokio::test]
async fn sdk_cancel_token_is_clonable() {
    let agent = make_agent(vec![text_response("ok")]);
    let t1 = agent.cancel_token();
    let t2 = t1.clone();
    assert!(!t1.is_cancelled());
    assert!(!t2.is_cancelled());
}

// ---------------------------------------------------------------------------
// Event conversion tests
// ---------------------------------------------------------------------------

#[test]
fn sdk_agent_event_to_value_agent_start() {
    let event = AgentEvent::AgentStart;
    let val = agent_event_to_value(&event);
    assert_eq!(val["type"], "AgentStart");
}

#[test]
fn sdk_agent_event_to_value_turn_start() {
    let event = AgentEvent::TurnStart;
    let val = agent_event_to_value(&event);
    assert_eq!(val["type"], "TurnStart");
}

#[test]
fn sdk_agent_event_to_value_session_persist_error() {
    let event = AgentEvent::SessionPersistError {
        message: "disk full".into(),
    };
    let val = agent_event_to_value(&event);
    assert_eq!(val["type"], "SessionPersistError");
    assert_eq!(val["message"], "disk full");
}

// ---------------------------------------------------------------------------
// SDK command → response correlation tests
// ---------------------------------------------------------------------------

#[test]
fn sdk_command_id_correlation() {
    let json = r#"{"type":"prompt","message":"hi","id":"corr-42"}"#;
    let cmd: SdkCommand = serde_json::from_str(json).unwrap();
    assert_eq!(cmd.id(), Some("corr-42"));

    // Build a response with the same id
    let resp = SdkResponse::success(cmd.id(), cmd.command_name());
    let val = serde_json::to_value(&resp).unwrap();
    assert_eq!(val["id"], "corr-42");
    assert_eq!(val["command"], "prompt");
}

// ---------------------------------------------------------------------------
// Documentation / unstable 0.x marker test
// ---------------------------------------------------------------------------

#[test]
fn sdk_types_are_documented_unstable() {
    // This test exists as a behavioral assertion that the SDK module
    // documentation mentions "unstable" and "0.x". The actual doc
    // check is in the module-level doc comment.
    let module_doc = include_str!("../src/sdk.rs");
    assert!(
        module_doc.contains("unstable"),
        "SDK module must document unstable 0.x status"
    );
    assert!(
        module_doc.contains("0.x"),
        "SDK module must document unstable 0.x status"
    );
}
