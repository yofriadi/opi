//! Integration tests verifying that opi-web-ui can consume RPC protocol events.
//!
//! Tests use the compiled `opi` binary in RPC mode and feed its output through
//! the web UI event parser and state machine. No live provider calls are made.

use std::io::{BufRead, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use opi_web_ui::event::WebUiEvent;
use opi_web_ui::render::Render;
use opi_web_ui::state::ConversationState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn opi_binary_path() -> std::path::PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_opi") {
        return std::path::PathBuf::from(path);
    }
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let mut path = std::path::PathBuf::from(manifest_dir);
    path.push("../../target/debug/opi");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}

struct RpcProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: std::io::BufReader<ChildStdout>,
}

impl RpcProcess {
    fn spawn() -> Self {
        let binary = opi_binary_path();
        let mut child = Command::new(&binary)
            .arg("--rpc")
            .arg("--model")
            .arg("anthropic:claude-sonnet-4")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("ANTHROPIC_API_KEY", "test-key-for-web-ui-rpc-tests")
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn {:?}: {e}", binary));

        let stdin = child.stdin.take().unwrap();
        let stdout = std::io::BufReader::new(child.stdout.take().unwrap());

        Self {
            child,
            stdin: Some(stdin),
            stdout,
        }
    }

    fn send(&mut self, cmd: &serde_json::Value) {
        let mut line = serde_json::to_string(cmd).unwrap();
        line.push('\n');
        self.stdin
            .as_mut()
            .unwrap()
            .write_all(line.as_bytes())
            .unwrap();
        self.stdin.as_mut().unwrap().flush().unwrap();
    }

    fn read_line(&mut self) -> serde_json::Value {
        loop {
            let mut line = String::new();
            let n = self
                .stdout
                .read_line(&mut line)
                .expect("failed to read line");
            if n == 0 {
                panic!("EOF from child process (no more stdout)");
            }
            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
            if trimmed.is_empty() {
                continue;
            }
            return serde_json::from_str(trimmed)
                .unwrap_or_else(|_| panic!("invalid JSON from stdout: {trimmed}"));
        }
    }

    /// Read lines until we see a response with the given command type.
    fn read_until_response(&mut self, command: &str) -> serde_json::Value {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if std::time::Instant::now() > deadline {
                panic!("timed out waiting for response with command={command}");
            }
            let line = self.read_line();
            if line["type"] == "response" && line["command"] == command {
                return line;
            }
        }
    }

    fn wait(mut self) -> std::process::ExitStatus {
        self.stdin.take();
        self.child.wait().unwrap()
    }
}

impl Drop for RpcProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn skip_if_no_binary() -> Option<()> {
    let binary = opi_binary_path();
    if !binary.exists() {
        eprintln!(
            "skipping web_ui_rpc subprocess test: binary not found at {:?}",
            binary
        );
        None
    } else {
        Some(())
    }
}

// ---------------------------------------------------------------------------
// Tests: web UI consumes RPC subprocess output
// ---------------------------------------------------------------------------

#[test]
fn web_ui_consumes_rpc_ready_event() {
    let Some(_) = skip_if_no_binary() else { return };
    let mut proc = RpcProcess::spawn();

    let raw = proc.read_line();
    let event = WebUiEvent::parse(&raw).expect("should parse rpc_ready");

    match event {
        WebUiEvent::RpcReady {
            schema_version,
            version,
        } => {
            assert_eq!(schema_version, 2);
            assert!(!version.is_empty());
        }
        other => panic!("expected RpcReady, got {:?}", other),
    }

    proc.send(&serde_json::json!({"type": "quit"}));
    let _resp = proc.read_line();
    let status = proc.wait();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn web_ui_state_tracks_rpc_session_info() {
    let Some(_) = skip_if_no_binary() else { return };
    let mut proc = RpcProcess::spawn();
    let mut state = ConversationState::new();

    // Parse ready header
    let ready_raw = proc.read_line();
    state.process(WebUiEvent::parse(&ready_raw).unwrap());

    // Request session info
    proc.send(&serde_json::json!({"type": "session_info", "id": "si-1"}));
    let resp_raw = proc.read_until_response("session_info");
    let resp_event = WebUiEvent::parse(&resp_raw).unwrap();
    state.process(resp_event);

    // Verify response tracked
    assert!(state.last_response().is_some());
    let resp = state.last_response().unwrap();
    assert_eq!(resp.command, "session_info");
    assert!(resp.success);

    proc.send(&serde_json::json!({"type": "quit"}));
    let _resp = proc.read_line();
    let status = proc.wait();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn web_ui_consumes_full_prompt_flow() {
    let Some(_) = skip_if_no_binary() else { return };
    let mut proc = RpcProcess::spawn();
    let mut state = ConversationState::new();

    // Parse ready header into state
    let ready_raw = proc.read_line();
    state.process(WebUiEvent::parse(&ready_raw).unwrap());

    // Send quit (end of flow)
    proc.send(&serde_json::json!({"type": "quit"}));
    let resp_raw = proc.read_line();

    // Parse the quit response
    let resp_event = WebUiEvent::parse(&resp_raw).unwrap();
    state.process(resp_event);

    // Verify the response was tracked
    let resp = state.last_response().unwrap();
    assert_eq!(resp.command, "quit");
    assert!(resp.success);

    let status = proc.wait();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn web_ui_conversation_renders_from_rpc_output() {
    let Some(_) = skip_if_no_binary() else { return };
    let mut proc = RpcProcess::spawn();
    let mut state = ConversationState::new();

    // Parse ready header
    let ready_raw = proc.read_line();
    state.process(WebUiEvent::parse(&ready_raw).unwrap());

    // Session info
    proc.send(&serde_json::json!({"type": "session_info", "id": "1"}));
    let si_raw = proc.read_until_response("session_info");
    state.process(WebUiEvent::parse(&si_raw).unwrap());

    // Set model
    proc.send(
        &serde_json::json!({"type": "set_model", "model": "anthropic:claude-sonnet-4", "id": "2"}),
    );
    let sm_raw = proc.read_until_response("set_model");
    state.process(WebUiEvent::parse(&sm_raw).unwrap());

    // Quit
    proc.send(&serde_json::json!({"type": "quit"}));
    let quit_raw = proc.read_line();
    state.process(WebUiEvent::parse(&quit_raw).unwrap());

    // Render the status bar — should contain model and session info
    let sb = state.to_status_bar();
    let html = sb.render_html();
    assert!(html.contains("status-bar"));
    assert!(html.contains("claude-sonnet-4"));

    let status = proc.wait();
    assert_eq!(status.code(), Some(0));
}

#[test]
fn web_ui_handles_multiple_rpc_commands() {
    let Some(_) = skip_if_no_binary() else { return };
    let mut proc = RpcProcess::spawn();
    let mut state = ConversationState::new();

    // Parse ready header
    let ready_raw = proc.read_line();
    state.process(WebUiEvent::parse(&ready_raw).unwrap());

    // Multiple session_info requests
    for i in 0..3 {
        proc.send(&serde_json::json!({"type": "session_info", "id": format!("si-{i}")}));
        let si_raw = proc.read_until_response("session_info");
        state.process(WebUiEvent::parse(&si_raw).unwrap());
    }

    // All responses should have been tracked (last one wins)
    let resp = state.last_response().unwrap();
    assert_eq!(resp.id.as_deref(), Some("si-2"));

    proc.send(&serde_json::json!({"type": "quit"}));
    let _resp = proc.read_line();
    let status = proc.wait();
    assert_eq!(status.code(), Some(0));
}

// ---------------------------------------------------------------------------
// Unit tests: web UI event parser with mock RPC data
// ---------------------------------------------------------------------------

#[test]
fn web_ui_parses_mock_agent_start() {
    let raw = serde_json::json!({"type": "AgentStart"});
    let event = WebUiEvent::parse(&raw).unwrap();
    let mut state = ConversationState::new();
    state.process(event);
    assert!(state.agent_running());
}

#[test]
fn web_ui_parses_mock_agent_end() {
    let raw = serde_json::json!({"type": "AgentEnd", "messages": []});
    let event = WebUiEvent::parse(&raw).unwrap();
    let mut state = ConversationState::new();
    state.process(WebUiEvent::AgentStart);
    assert!(state.agent_running());
    state.process(event);
    assert!(!state.agent_running());
}

#[test]
fn web_ui_parses_mock_tool_execution_lifecycle() {
    let mut state = ConversationState::new();

    // Simulate a full tool lifecycle through parsed events
    state.process(
        WebUiEvent::parse(&serde_json::json!({
            "type": "ToolExecutionStart",
            "tool_call_id": "tc-mock",
            "tool_name": "read",
            "args": {"path": "test.txt"}
        }))
        .unwrap(),
    );
    assert_eq!(state.tool_calls().len(), 1);
    assert_eq!(state.tool_calls()[0].tool_name(), "read");

    state.process(
        WebUiEvent::parse(&serde_json::json!({
            "type": "ToolExecutionEnd",
            "tool_call_id": "tc-mock",
            "tool_name": "read",
            "result": "file content",
            "details": null,
            "is_error": false
        }))
        .unwrap(),
    );
    assert_eq!(
        state.tool_calls()[0].status(),
        opi_web_ui::components::ToolCallStatus::Completed
    );
}

#[test]
fn web_ui_renders_conversation_from_mock_events() {
    let mut state = ConversationState::new();

    state.process(WebUiEvent::MessageStart {
        model: "test-model".to_owned(),
        provider: "test-provider".to_owned(),
    });
    state.process(WebUiEvent::TextDelta {
        index: 0,
        delta: "Hello from mock agent".to_owned(),
    });
    state.process(WebUiEvent::MessageEnd);

    let view = state.to_conversation_view();
    let html = view.render_html();
    assert!(html.contains("Hello from mock agent"));
    assert!(html.contains("conversation"));
}
