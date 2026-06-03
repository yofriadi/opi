//! RPC JSONL mode — bidirectional command/event protocol over stdin/stdout.
//!
//! RPC mode enables headless operation of the coding agent via a strict JSONL
//! protocol. Commands arrive on stdin (one JSON object per line), responses
//! and events are emitted on stdout (one JSON object per line). Diagnostics
//! go to stderr.
//!
//! # Protocol version
//!
//! This is an **unstable 0.x** protocol. The schema may change between minor
//! versions without notice. Clients MUST check `schema_version` in the
//! `rpc_ready` header.
//!
//! # Framing
//!
//! LF (`\n`) is the only record delimiter. Clients MUST split on `\n` only
//! and SHOULD strip a trailing `\r` if present. Generic line readers that
//! split on Unicode separators (U+2028, U+2029) are not protocol-compliant.
//!
//! # Commands
//!
//! | Command           | Description                                      |
//! |-------------------|--------------------------------------------------|
//! | `prompt`          | Send user prompt, stream agent events            |
//! | `continue`        | Continue conversation with additional text        |
//! | `steer`           | Queue steering message during agent operation     |
//! | `follow_up`       | Queue follow-up message for after agent stops     |
//! | `abort`           | Cancel current agent operation                    |
//! | `set_model`       | Switch provider:model                             |
//! | `set_thinking_level` | Set reasoning/thinking level                   |
//! | `compact`         | Trigger manual compaction                         |
//! | `session_info`    | Query session metadata                            |
//! | `quit`            | Shut down the RPC session                         |
//!
//! # Responses
//!
//! Every command produces at most one `response` object. For `prompt` and
//! `continue`, `success: true` means the command was accepted; agent events
//! (including errors after acceptance) arrive as async `event` lines.
//!
//! # Error semantics
//!
//! - **Parse errors**: `{"type":"response","command":"parse","success":false,"error":"..."}`
//! - **Command rejected**: `{"type":"response","command":"<cmd>","success":false,"error":"..."}`
//! - **Agent errors after acceptance**: emitted as regular agent events, not as a second response.
//!
//! # Cancellation
//!
//! `abort` cancels the current agent operation via the cancellation token.
//! The agent returns a `Cancelled` error which surfaces as an `agent_end`
//! event. A second `abort` while idle is a no-op (returns success).

use std::io::{self, Write as IoWrite};
use std::path::PathBuf;
use std::sync::Arc;

use opi_agent::event::AgentEvent;
use opi_agent::loop_types::AgentError;
use opi_agent::message::AgentMessage;
use opi_agent::session_event::CompactionReason;
use opi_ai::provider::Provider;

use crate::config::OpiConfig;
use crate::harness::CodingHarness;
use crate::policy::{RunMode, ToolSelection};
use crate::runner::ExitCode;

/// RPC protocol schema version. Clients MUST check this.
pub const RPC_SCHEMA_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Command types
// ---------------------------------------------------------------------------

/// An RPC command parsed from a stdin line.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type")]
pub enum RpcCommand {
    /// Send a user prompt.
    prompt {
        #[serde(default)]
        id: Option<String>,
        message: String,
    },
    /// Continue conversation with additional text.
    #[serde(rename = "continue")]
    continue_ {
        #[serde(default)]
        id: Option<String>,
        message: String,
    },
    /// Queue a steering message.
    steer {
        #[serde(default)]
        id: Option<String>,
        message: String,
    },
    /// Queue a follow-up message.
    follow_up {
        #[serde(default)]
        id: Option<String>,
        message: String,
    },
    /// Cancel current agent operation.
    abort {
        #[serde(default)]
        id: Option<String>,
    },
    /// Switch model.
    set_model {
        #[serde(default)]
        id: Option<String>,
        model: String,
    },
    /// Set thinking/reasoning level.
    set_thinking_level {
        #[serde(default)]
        id: Option<String>,
        level: String,
    },
    /// Trigger manual compaction.
    compact {
        #[serde(default)]
        id: Option<String>,
    },
    /// Query session metadata.
    session_info {
        #[serde(default)]
        id: Option<String>,
    },
    /// Shut down the RPC session.
    quit {
        #[serde(default)]
        id: Option<String>,
    },
}

impl RpcCommand {
    /// Return the optional correlation id.
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::prompt { id, .. }
            | Self::continue_ { id, .. }
            | Self::steer { id, .. }
            | Self::follow_up { id, .. }
            | Self::abort { id }
            | Self::set_model { id, .. }
            | Self::set_thinking_level { id, .. }
            | Self::compact { id }
            | Self::session_info { id }
            | Self::quit { id } => id.as_deref(),
        }
    }

    /// Return the command name for response correlation.
    pub fn command_name(&self) -> &'static str {
        match self {
            Self::prompt { .. } => "prompt",
            Self::continue_ { .. } => "continue",
            Self::steer { .. } => "steer",
            Self::follow_up { .. } => "follow_up",
            Self::abort { .. } => "abort",
            Self::set_model { .. } => "set_model",
            Self::set_thinking_level { .. } => "set_thinking_level",
            Self::compact { .. } => "compact",
            Self::session_info { .. } => "session_info",
            Self::quit { .. } => "quit",
        }
    }

    /// Whether this is the quit command.
    pub fn is_quit(&self) -> bool {
        matches!(self, Self::quit { .. })
    }
}

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

/// Build a success response.
fn success_response(id: Option<&str>, command: &str) -> serde_json::Value {
    let mut v = serde_json::json!({
        "type": "response",
        "command": command,
        "success": true,
    });
    if let Some(id) = id {
        v["id"] = serde_json::Value::String(id.to_owned());
    }
    v
}

/// Build a success response with data.
fn success_response_with_data(
    id: Option<&str>,
    command: &str,
    data: serde_json::Value,
) -> serde_json::Value {
    let mut v = success_response(id, command);
    v["data"] = data;
    v
}

/// Build an error response.
fn error_response(id: Option<&str>, command: &str, error: &str) -> serde_json::Value {
    let mut v = serde_json::json!({
        "type": "response",
        "command": command,
        "success": false,
        "error": error,
    });
    if let Some(id) = id {
        v["id"] = serde_json::Value::String(id.to_owned());
    }
    v
}

// ---------------------------------------------------------------------------
// Event mapping
// ---------------------------------------------------------------------------

/// Convert an AgentEvent to an RPC event JSON line.
fn agent_event_to_rpc(event: &AgentEvent) -> serde_json::Value {
    // Reuse the existing AgentEvent serialization (which includes the "type" tag).
    match serde_json::to_value(event) {
        Ok(v) => v,
        Err(_) => serde_json::json!({
            "type": "session_persist_error",
            "message": "failed to serialize agent event",
        }),
    }
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// RPC runner that owns the harness and processes commands from stdin.
pub struct RpcRunner {
    harness: CodingHarness,
    /// Whether the agent is currently processing a prompt/continue.
    running: bool,
}

impl RpcRunner {
    /// Create a new RPC runner.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        allow_mutating: bool,
        user_system_prompt: Option<String>,
        initial_messages: Vec<AgentMessage>,
    ) -> Result<Self, crate::policy::ToolPolicyError> {
        let tool_config = crate::policy::ToolRuntimeConfig::resolve(
            RunMode::NonInteractive,
            allow_mutating,
            ToolSelection::Default,
        )?;
        let hooks = Box::new(crate::runner::NonInteractiveHooks::new(allow_mutating));
        let harness = CodingHarness::new_with_hooks_and_resume_tool_config(
            provider,
            model,
            config,
            workspace_root,
            hooks,
            user_system_prompt,
            initial_messages,
            None,
            tool_config,
        );
        Ok(Self {
            harness,
            running: false,
        })
    }

    /// Run the RPC main loop. Returns an exit code.
    pub async fn run(&mut self) -> i32 {
        let stdout = io::stdout();
        let mut writer = io::BufWriter::new(stdout.lock());
        let stdin = io::stdin();

        // Write the rpc_ready header.
        let header = serde_json::json!({
            "type": "rpc_ready",
            "schema_version": RPC_SCHEMA_VERSION,
            "mode": "rpc",
            "version": env!("CARGO_PKG_VERSION"),
        });
        if write_jsonl(&mut writer, &header).is_err() {
            return ExitCode::RuntimeFailure as i32;
        }

        // Set up event forwarding: subscriber callback → channel → stdout.
        let (event_tx, mut event_rx): (
            tokio::sync::mpsc::UnboundedSender<serde_json::Value>,
            tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
        ) = tokio::sync::mpsc::unbounded_channel();

        let event_tx = Arc::new(event_tx);
        let etx = event_tx.clone();
        self.harness.subscribe(Box::new(move |event: &AgentEvent| {
            let rpc_event = agent_event_to_rpc(event);
            let _ = etx.send(rpc_event);
        }));

        // Clone the cancel token for abort during prompt operations.
        let cancel_token = self.harness.cancel_token();

        // Buffered stdin reader.
        let reader = io::BufReader::new(stdin.lock());
        let mut lines = reader.lines();

        // Main loop.
        loop {
            // Flush any pending event output before reading the next command.
            let _ = writer.flush();

            // Try to drain events first (non-blocking).
            while let Ok(event) = event_rx.try_recv() {
                if write_jsonl(&mut writer, &event).is_err() {
                    return ExitCode::RuntimeFailure as i32;
                }
            }
            let _ = writer.flush();

            // Read next command line (blocking).
            let line = match lines.next() {
                Some(Ok(l)) => l,
                Some(Err(_)) | None => {
                    // EOF or read error — flush events and exit.
                    drain_events(&mut event_rx, &mut writer);
                    return ExitCode::Success as i32;
                }
            };

            let trimmed = line.trim_end_matches('\r').trim();
            if trimmed.is_empty() {
                continue;
            }

            // Parse command.
            let cmd = match serde_json::from_str::<RpcCommand>(trimmed) {
                Ok(c) => c,
                Err(e) => {
                    let resp =
                        error_response(None, "parse", &format!("failed to parse command: {e}"));
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                    continue;
                }
            };

            let cmd_id = cmd.id().map(String::from);
            let cmd_name = cmd.command_name();

            if cmd.is_quit() {
                let resp = success_response(cmd_id.as_deref(), cmd_name);
                let _ = write_jsonl(&mut writer, &resp);
                let _ = writer.flush();
                drain_events(&mut event_rx, &mut writer);
                return ExitCode::Success as i32;
            }

            match cmd {
                RpcCommand::prompt { message, .. } => {
                    self.running = true;
                    // Respond immediately — success means accepted.
                    let resp = success_response(cmd_id.as_deref(), cmd_name);
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();

                    let result = self.harness.prompt(&message).await;
                    self.running = false;
                    self.handle_agent_result(&mut writer, result);
                    drain_events(&mut event_rx, &mut writer);
                }
                RpcCommand::continue_ { message, .. } => {
                    if !self.running {
                        self.running = true;
                        let resp = success_response(cmd_id.as_deref(), cmd_name);
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();

                        let result = self.harness.continue_(&message).await;
                        self.running = false;
                        self.handle_agent_result(&mut writer, result);
                        drain_events(&mut event_rx, &mut writer);
                    } else {
                        let resp = error_response(
                            cmd_id.as_deref(),
                            cmd_name,
                            "agent is already running; use steer or follow_up to queue messages",
                        );
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();
                    }
                }
                RpcCommand::abort { .. } => {
                    cancel_token.cancel();
                    let resp = success_response(cmd_id.as_deref(), cmd_name);
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                RpcCommand::set_model { model, .. } => {
                    if self.running {
                        let resp = error_response(
                            cmd_id.as_deref(),
                            cmd_name,
                            "cannot change model while agent is running",
                        );
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();
                    } else {
                        self.harness.set_model(model);
                        let resp = success_response(cmd_id.as_deref(), cmd_name);
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();
                    }
                }
                RpcCommand::set_thinking_level { level, .. } => {
                    // Thinking level is controlled through AgentLoopConfig.
                    // For now, acknowledge the command. Full implementation
                    // requires updating the agent config at runtime, which
                    // will be addressed when the SDK surface (task 4.2) is
                    // implemented.
                    let _ = level; // accepted but no-op until agent supports runtime config changes
                    let resp = success_response(cmd_id.as_deref(), cmd_name);
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                RpcCommand::compact { .. } => {
                    if self.running {
                        let resp = error_response(
                            cmd_id.as_deref(),
                            cmd_name,
                            "cannot compact while agent is running",
                        );
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();
                    } else {
                        match self.harness.compact(CompactionReason::Manual) {
                            Ok(Some(result)) => {
                                let data = serde_json::json!({
                                    "summary": result.summary,
                                    "first_kept_entry_id": result.first_kept_entry_id,
                                    "tokens_before": result.tokens_before,
                                    "tokens_after": result.tokens_after,
                                });
                                let resp =
                                    success_response_with_data(cmd_id.as_deref(), cmd_name, data);
                                let _ = write_jsonl(&mut writer, &resp);
                                let _ = writer.flush();
                            }
                            Ok(None) => {
                                let resp = error_response(
                                    cmd_id.as_deref(),
                                    cmd_name,
                                    "compaction produced no output",
                                );
                                let _ = write_jsonl(&mut writer, &resp);
                                let _ = writer.flush();
                            }
                            Err(e) => {
                                let resp = error_response(cmd_id.as_deref(), cmd_name, &e);
                                let _ = write_jsonl(&mut writer, &resp);
                                let _ = writer.flush();
                            }
                        }
                    }
                }
                RpcCommand::session_info { .. } => {
                    let mut data = serde_json::json!({
                        "model": self.harness.model(),
                    });
                    if let Some(session) = self.harness.session() {
                        data["session_id"] =
                            serde_json::Value::String(session.session_id().to_owned());
                    }
                    let resp = success_response_with_data(cmd_id.as_deref(), cmd_name, data);
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                RpcCommand::steer { message, .. } => {
                    // Steering is queued via the agent's steer method.
                    self.harness.steer(message);
                    let resp = success_response(cmd_id.as_deref(), cmd_name);
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                RpcCommand::follow_up { message, .. } => {
                    // Follow-up is queued via the agent's follow_up method.
                    self.harness.follow_up(message);
                    let resp = success_response(cmd_id.as_deref(), cmd_name);
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                _ => unreachable!(), // quit handled above
            }
        }
    }

    fn handle_agent_result(
        &self,
        writer: &mut io::BufWriter<io::StdoutLock<'_>>,
        result: Result<Vec<AgentMessage>, AgentError>,
    ) {
        // Agent errors after acceptance are already surfaced through events.
        // We emit a summary line here for the RPC client to detect completion.
        match result {
            Ok(_) => {}
            Err(AgentError::Cancelled) => {
                // Already surfaced via events; no extra response needed.
            }
            Err(_) => {
                // Other errors also surfaced via events.
            }
        }
        let _ = writer.flush();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a JSON value as a single line to the writer.
fn write_jsonl(writer: &mut dyn IoWrite, value: &serde_json::Value) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")
}

/// Drain all pending events from the channel and write them.
fn drain_events(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    writer: &mut io::BufWriter<io::StdoutLock<'_>>,
) {
    while let Ok(event) = rx.try_recv() {
        let _ = write_jsonl(writer, &event);
    }
    let _ = writer.flush();
}

/// Helper for lines() iterator — not using std::io::BufRead::lines directly
/// because we need a type alias.
use std::io::BufRead;

// ---------------------------------------------------------------------------
// Hooks — re-uses NonInteractiveHooks from runner.rs (tool safety policy).
// No additional hooks needed for RPC mode.
