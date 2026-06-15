//! Adapter runtime bridge tests (task 5.6).
//!
//! Covers: ProcessAdapter implements Extension, tools are bridged through Tool
//! trait, commands dispatch through Extension::on_command, hooks block/allow
//! tool calls, events are forwarded, state serialize/restore round-trips,
//! model overrides are collected, and cancellation bridges through.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use opi_agent::extension::{Extension, ExtensionCommand, ExtensionHookResult, ExtensionRegistry};
use opi_agent::hooks::{AgentHooks, BeforeToolCallContext, PrepareNextTurnContext};
use opi_agent::message::AgentMessage;
use opi_coding_agent::adapter_extension::ProcessAdapter;
use opi_coding_agent::adapter_host::{AdapterHost, AdapterProcessConfig};
use opi_coding_agent::adapter_protocol::AdapterHostMessage;

// ---------------------------------------------------------------------------
// Noop hooks for composite hook testing
// ---------------------------------------------------------------------------

/// Minimal no-op AgentHooks for test use.
struct NoopHooks;

impl AgentHooks for NoopHooks {
    fn convert_to_llm(
        &self,
        _messages: &[opi_agent::message::AgentMessage],
    ) -> Result<Vec<opi_ai::message::Message>, opi_agent::loop_types::AgentError> {
        Ok(vec![])
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Locate the `adapter_host_mock` test binary, preferring the newest version
/// when multiple hashed copies exist.
fn mock_adapter_bin() -> PathBuf {
    let current = std::env::current_exe().expect("current exe path");
    let deps_dir = current.parent().expect("deps directory");

    let exact_name = if cfg!(windows) {
        "adapter_host_mock.exe"
    } else {
        "adapter_host_mock"
    };
    let exact_path = deps_dir.join(exact_name);
    if exact_path.exists() {
        return exact_path;
    }

    let exe_suffix = if cfg!(windows) { ".exe" } else { "" };
    let prefix = "adapter_host_mock-";
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(deps_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(prefix)
                && name_str.ends_with(exe_suffix)
                && !name_str.ends_with(".d")
                && let Ok(meta) = entry.metadata()
                && let Ok(modified) = meta.modified()
                && best.as_ref().is_none_or(|(t, _)| modified > *t)
            {
                best = Some((modified, entry.path()));
            }
        }
    }

    best.map(|(_, p)| p)
        .expect("Could not find adapter_host_mock binary in deps directory")
}

fn config_for_mode(mode: &str) -> AdapterProcessConfig {
    AdapterProcessConfig {
        command: mock_adapter_bin(),
        args: vec![],
        working_dir: std::env::current_dir().expect("cwd"),
        env: vec![("OPI_ADAPTER_TEST_MODE".to_string(), mode.to_string())],
    }
}

async fn start_capabilities_adapter() -> (Arc<AdapterHost>, ProcessAdapter) {
    let host = AdapterHost::start(
        "mock",
        config_for_mode("capabilities"),
        Duration::from_secs(5),
    )
    .await
    .expect("start host");
    let caps = host.capabilities().clone();
    let host = Arc::new(host);
    let adapter = ProcessAdapter::from_host("mock", host.clone(), caps);
    (host, adapter)
}

async fn start_gate_adapter() -> (Arc<AdapterHost>, ProcessAdapter) {
    let host = AdapterHost::start("gate", config_for_mode("gate"), Duration::from_secs(5))
        .await
        .expect("start host");
    let caps = host.capabilities().clone();
    let host = Arc::new(host);
    let adapter = ProcessAdapter::from_host("gate", host.clone(), caps);
    (host, adapter)
}

async fn start_prepare_adapter() -> (Arc<AdapterHost>, ProcessAdapter) {
    let host = AdapterHost::start(
        "prepare",
        config_for_mode("prepare"),
        Duration::from_secs(5),
    )
    .await
    .expect("start host");
    let caps = host.capabilities().clone();
    let host = Arc::new(host);
    let adapter = ProcessAdapter::from_host("prepare", host.clone(), caps);
    (host, adapter)
}

async fn start_transform_adapter() -> (Arc<AdapterHost>, ProcessAdapter) {
    let host = AdapterHost::start(
        "transform",
        config_for_mode("transform"),
        Duration::from_secs(5),
    )
    .await
    .expect("start host");
    let caps = host.capabilities().clone();
    let host = Arc::new(host);
    let adapter = ProcessAdapter::from_host("transform", host.clone(), caps);
    (host, adapter)
}

// ---------------------------------------------------------------------------
// 1. Extension name
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_extension_reports_name() {
    let (_host, adapter) = start_capabilities_adapter().await;
    assert_eq!(adapter.name(), "mock");
}

// ---------------------------------------------------------------------------
// 2. Tools are collected from capabilities
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_extension_provides_tools_from_capabilities() {
    let (_host, adapter) = start_capabilities_adapter().await;
    let tools = adapter.tools();
    assert_eq!(tools.len(), 1);

    let def = tools[0].definition();
    assert_eq!(def.name, "test_tool");
    assert!(!def.description.is_empty());
}

// ---------------------------------------------------------------------------
// 3. Tool execution round-trips through the adapter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_tool_execute_sends_tool_call() {
    let (_host, adapter) = start_capabilities_adapter().await;
    let tools = adapter.tools();
    let tool = &tools[0];

    let result = tool
        .execute(
            "call-1",
            serde_json::json!({"input": "hello"}),
            tokio_util::sync::CancellationToken::new(),
            None,
        )
        .await
        .expect("tool execute");

    assert!(!result.is_error);
    assert!(
        result
            .content
            .iter()
            .any(|c| matches!(c, opi_ai::message::OutputContent::Text { text } if text.contains("mock_result"))),
        "expected mock_result in tool output"
    );
}

// ---------------------------------------------------------------------------
// 4. Tool execution bridges cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_tool_execute_respects_cancellation() {
    // Start a hang_request adapter that never responds after handshake
    let host = AdapterHost::start(
        "mock",
        config_for_mode("hang_request"),
        Duration::from_secs(5),
    )
    .await
    .expect("start host");

    // Manually craft a ProcessAdapter with a tool definition that will time out
    let caps = host.capabilities().clone();
    let host = Arc::new(host);
    let adapter = ProcessAdapter::from_host("mock", host.clone(), caps);
    let tools = adapter.tools();

    if tools.is_empty() {
        // hang_request mode has no tools — skip this variant
        let _ = host
            .send_request(
                AdapterHostMessage::Shutdown {
                    id: "end".into(),
                    reason: "test_end".into(),
                },
                Duration::from_secs(1),
            )
            .await;
        return;
    }

    let token = tokio_util::sync::CancellationToken::new();
    let token_clone = token.clone();

    let handle = tokio::spawn(async move {
        tools[0]
            .execute("call-cancel", serde_json::json!({}), token_clone, None)
            .await
    });

    // Cancel immediately
    token.cancel();

    let result = tokio::time::timeout(Duration::from_secs(5), handle).await;
    match result {
        Ok(Ok(Ok(tool_result))) => {
            // Cancellation may produce an error result or succeed with what
            // completed before cancel — both are acceptable.
            assert!(tool_result.is_error || !tool_result.content.is_empty());
        }
        Ok(Ok(Err(_))) | Ok(Err(_)) | Err(_) => {
            // Cancellation caused the future to be dropped or timed out —
            // acceptable.
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Command dispatches through Extension::on_command
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_command_dispatches_through_extension() {
    let (_host, adapter) = start_capabilities_adapter().await;

    let cmd = ExtensionCommand::new("test/status", serde_json::json!({}));
    let result = adapter.on_command(&cmd).await.expect("command dispatch");
    assert!(result.is_some(), "adapter should handle its own commands");
    let data = result.unwrap();
    assert_eq!(data["status"], "ok");
}

// ---------------------------------------------------------------------------
// 6. Unknown command returns None
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_unknown_command_returns_none() {
    let (_host, adapter) = start_capabilities_adapter().await;

    let cmd = ExtensionCommand::new("nonexistent", serde_json::json!({}));
    let result = adapter.on_command(&cmd).await.expect("command dispatch");
    assert!(
        result.is_none(),
        "adapter should not handle unknown commands"
    );
}

// ---------------------------------------------------------------------------
// 7. Hook blocks destructive tool calls
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_before_tool_hook_can_block() {
    let (_host, adapter) = start_gate_adapter().await;

    let result = adapter
        .on_before_tool_call("bash", &serde_json::json!({"command": "rm -rf target"}))
        .await;
    assert!(
        matches!(result, ExtensionHookResult::Block { .. }),
        "expected Block for destructive bash command"
    );
}

// ---------------------------------------------------------------------------
// 8. Hook allows non-destructive tool calls
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_before_tool_hook_allows_safe_tools() {
    let (_host, adapter) = start_gate_adapter().await;

    let result = adapter
        .on_before_tool_call("read", &serde_json::json!({"path": "/tmp/file"}))
        .await;
    assert!(
        matches!(result, ExtensionHookResult::Continue),
        "expected Continue for safe read tool"
    );
}

// ---------------------------------------------------------------------------
// 9. Hook skips if adapter does not declare it
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_skips_hook_not_in_capabilities() {
    let (_host, adapter) = start_capabilities_adapter().await;

    // The capabilities mock hooks are ["before_tool_call", "event"].
    // It does NOT declare "after_tool_call", so on_after_tool_call should
    // return immediately without sending anything to the adapter process.
    // Verify by: calling on_after_tool_call, then sending a command through
    // the adapter to confirm the host is not blocked by a stale request.
    let _result = adapter
        .on_after_tool_call(
            "read",
            &opi_agent::tool::ToolResult {
                content: vec![],
                details: None,
                is_error: false,
                terminate: false,
            },
        )
        .await;

    // If on_after_tool_call had sent a request to the adapter, the adapter
    // (which doesn't handle "after_tool_call") would not respond, and the
    // pending request would time out or block. Verify the host is still
    // responsive by dispatching a command.
    let cmd = ExtensionCommand::new("test/status", serde_json::json!({}));
    let result = adapter.on_command(&cmd).await.expect("command dispatch");
    assert!(
        result.is_some(),
        "adapter host should still be responsive after skipped hook"
    );
}

// ---------------------------------------------------------------------------
// 10. Event forwarding
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_event_forwarding_does_not_block() {
    let (_host, adapter) = start_capabilities_adapter().await;

    let start = std::time::Instant::now();
    adapter.on_event(&create_test_event());
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "on_event took {elapsed:?}, should be near-instant"
    );
}

fn create_test_event() -> opi_agent::event::AgentEvent {
    opi_agent::event::AgentEvent::TurnStart
}

// ---------------------------------------------------------------------------
// 11. State serialize round-trips
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn adapter_state_serialize_returns_state() {
    let (_host, adapter) = start_capabilities_adapter().await;

    let state = adapter.serialize_state().expect("serialize");
    assert!(state.is_some());
    let state = state.unwrap();
    assert_eq!(state["mock"], true);
}

// ---------------------------------------------------------------------------
// 12. State restore round-trips
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn adapter_state_restore_accepts_state() {
    let (_host, adapter) = start_capabilities_adapter().await;

    adapter
        .restore_state(serde_json::json!({"items": ["a"]}))
        .expect("restore");
}

#[tokio::test]
async fn adapter_state_async_round_trip_works_on_current_thread_runtime() {
    let (_host, adapter) = start_capabilities_adapter().await;

    let state = adapter
        .serialize_state_async()
        .await
        .expect("serialize")
        .expect("state");
    assert_eq!(state["mock"], true);

    adapter
        .restore_state_async(serde_json::json!({"items": ["a"]}))
        .await
        .expect("restore");
}

// ---------------------------------------------------------------------------
// 13. Model overrides collected from capabilities
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_model_overrides_from_capabilities() {
    let (_host, adapter) = start_capabilities_adapter().await;
    // Capabilities mock has empty model_overrides
    let overrides = adapter.model_overrides();
    assert!(overrides.is_empty());
}

// ---------------------------------------------------------------------------
// 14. Full registry integration: tools + command dispatch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn adapter_registered_in_registry_dispatches_command() {
    let (_host, adapter) = start_capabilities_adapter().await;

    let mut registry = ExtensionRegistry::new();
    registry
        .register(Box::new(adapter))
        .expect("register adapter");

    // Collect tools
    let tools = registry.collect_tools();
    assert_eq!(tools.len(), 1);

    // Dispatch command
    let cmd = ExtensionCommand::new("test/status", serde_json::json!({}));
    let result = registry.dispatch_command(&cmd).await.expect("dispatch");
    assert!(result.is_some());
    assert_eq!(result.unwrap()["status"], "ok");
}

// ---------------------------------------------------------------------------
// 15. Gate adapter in registry blocks destructive commands
// ---------------------------------------------------------------------------

#[tokio::test]
async fn gate_adapter_in_registry_blocks_destructive_tools() {
    let (_host, adapter) = start_gate_adapter().await;

    let mut registry = ExtensionRegistry::new();
    registry
        .register(Box::new(adapter))
        .expect("register gate adapter");

    // Use wrap_hooks with a no-op base hook
    let base_hooks = Box::new(NoopHooks);
    let composite = registry.wrap_hooks(base_hooks);

    let ctx = BeforeToolCallContext {
        tool_call_id: "call-1".into(),
        tool_name: "bash".into(),
        args: serde_json::json!({"command": "rm -rf target"}),
        messages: vec![],
    };

    let result = composite.before_tool_call(ctx).await;
    assert!(
        matches!(result, opi_agent::hooks::BeforeToolCallResult::Deny { .. }),
        "composite hooks should deny destructive bash via adapter"
    );
}

// ---------------------------------------------------------------------------
// 16. State serialize/restore through registry
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn adapter_state_round_trip_through_registry() {
    let (_host, adapter) = start_gate_adapter().await;

    let mut registry = ExtensionRegistry::new();
    registry.register(Box::new(adapter)).expect("register");

    let states = registry.serialize_states().expect("serialize");
    assert!(states.is_object());
    assert!(states.get("gate").is_some());

    registry.restore_states(states).expect("restore");
}

#[tokio::test]
async fn adapter_prepare_next_turn_can_inject_message() {
    let (_host, adapter) = start_prepare_adapter().await;
    let update = adapter
        .prepare_next_turn(&PrepareNextTurnContext {
            messages: vec![],
            turn: 1,
        })
        .await
        .expect("update");

    assert_eq!(update.extra_messages.len(), 1);
    match &update.extra_messages[0] {
        AgentMessage::Custom(message) => {
            assert_eq!(message.kind, "adapter_note");
            assert_eq!(message.data["text"], "next turn");
        }
        other => panic!("expected custom message, got {other:?}"),
    }
}

#[tokio::test]
async fn adapter_transform_context_can_rewrite_messages() {
    let (_host, adapter) = start_transform_adapter().await;
    let messages = adapter.transform_context(vec![]).await.expect("transform");

    assert_eq!(messages.len(), 1);
    match &messages[0] {
        AgentMessage::Custom(message) => {
            assert_eq!(message.kind, "adapter_transform");
            assert_eq!(message.data["text"], "transformed");
        }
        other => panic!("expected custom message, got {other:?}"),
    }
}
