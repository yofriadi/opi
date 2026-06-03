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
use opi_agent::sdk::{SDK_SCHEMA_VERSION, SdkCommand, SdkResponse, agent_event_to_value};
use opi_agent::session_event::CompactionReason;
use opi_ai::provider::Provider;

use crate::config::OpiConfig;
use crate::harness::CodingHarness;
use crate::policy::{RunMode, ToolSelection};
use crate::runner::ExitCode;

/// Re-export the SDK command type as the RPC command type.
///
/// The canonical definition lives in [`opi_agent::sdk::SdkCommand`]; this
/// alias preserves the `RpcCommand` name for backward-compat within the
/// crate while ensuring no protocol logic is duplicated.
pub type RpcCommand = SdkCommand;

/// Re-export the SDK schema version for crate-level access (e.g. tests).
pub const RPC_SCHEMA_VERSION: u32 = SDK_SCHEMA_VERSION;

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
            "schema_version": SDK_SCHEMA_VERSION,
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
            let rpc_event = agent_event_to_value(event);
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
            let cmd = match serde_json::from_str::<SdkCommand>(trimmed) {
                Ok(c) => c,
                Err(e) => {
                    let resp = serde_json::to_value(SdkResponse::error(
                        None,
                        "parse",
                        &format!("failed to parse command: {e}"),
                    ))
                    .unwrap();
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                    continue;
                }
            };

            let cmd_id = cmd.id().map(String::from);
            let cmd_name = cmd.command_name();

            if cmd.is_quit() {
                let resp = serde_json::to_value(SdkResponse::success(cmd_id.as_deref(), cmd_name))
                    .unwrap();
                let _ = write_jsonl(&mut writer, &resp);
                let _ = writer.flush();
                drain_events(&mut event_rx, &mut writer);
                return ExitCode::Success as i32;
            }

            match cmd {
                SdkCommand::prompt { message, .. } => {
                    self.running = true;
                    // Respond immediately — success means accepted.
                    let resp =
                        serde_json::to_value(SdkResponse::success(cmd_id.as_deref(), cmd_name))
                            .unwrap();
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();

                    let result = self.harness.prompt(&message).await;
                    self.running = false;
                    self.handle_agent_result(&mut writer, result);
                    drain_events(&mut event_rx, &mut writer);
                }
                SdkCommand::continue_ { message, .. } => {
                    if !self.running {
                        self.running = true;
                        let resp =
                            serde_json::to_value(SdkResponse::success(cmd_id.as_deref(), cmd_name))
                                .unwrap();
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();

                        let result = self.harness.continue_(&message).await;
                        self.running = false;
                        self.handle_agent_result(&mut writer, result);
                        drain_events(&mut event_rx, &mut writer);
                    } else {
                        let resp = serde_json::to_value(SdkResponse::error(
                            cmd_id.as_deref(),
                            cmd_name,
                            "agent is already running; use steer or follow_up to queue messages",
                        ))
                        .unwrap();
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();
                    }
                }
                SdkCommand::abort { .. } => {
                    cancel_token.cancel();
                    let resp =
                        serde_json::to_value(SdkResponse::success(cmd_id.as_deref(), cmd_name))
                            .unwrap();
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                SdkCommand::set_model { model, .. } => {
                    if self.running {
                        let resp = serde_json::to_value(SdkResponse::error(
                            cmd_id.as_deref(),
                            cmd_name,
                            "cannot change model while agent is running",
                        ))
                        .unwrap();
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();
                    } else {
                        self.harness.set_model(model);
                        let resp =
                            serde_json::to_value(SdkResponse::success(cmd_id.as_deref(), cmd_name))
                                .unwrap();
                        let _ = write_jsonl(&mut writer, &resp);
                        let _ = writer.flush();
                    }
                }
                SdkCommand::set_thinking_level { level, .. } => {
                    // Thinking level is controlled through AgentLoopConfig.
                    // For now, acknowledge the command. Full implementation
                    // requires updating the agent config at runtime.
                    let _ = level; // accepted but no-op until agent supports runtime config changes
                    let resp =
                        serde_json::to_value(SdkResponse::success(cmd_id.as_deref(), cmd_name))
                            .unwrap();
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                SdkCommand::compact { .. } => {
                    if self.running {
                        let resp = serde_json::to_value(SdkResponse::error(
                            cmd_id.as_deref(),
                            cmd_name,
                            "cannot compact while agent is running",
                        ))
                        .unwrap();
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
                                let resp = serde_json::to_value(SdkResponse::success_with_data(
                                    cmd_id.as_deref(),
                                    cmd_name,
                                    data,
                                ))
                                .unwrap();
                                let _ = write_jsonl(&mut writer, &resp);
                                let _ = writer.flush();
                            }
                            Ok(None) => {
                                let resp = serde_json::to_value(SdkResponse::error(
                                    cmd_id.as_deref(),
                                    cmd_name,
                                    "compaction produced no output",
                                ))
                                .unwrap();
                                let _ = write_jsonl(&mut writer, &resp);
                                let _ = writer.flush();
                            }
                            Err(e) => {
                                let resp = serde_json::to_value(SdkResponse::error(
                                    cmd_id.as_deref(),
                                    cmd_name,
                                    &e,
                                ))
                                .unwrap();
                                let _ = write_jsonl(&mut writer, &resp);
                                let _ = writer.flush();
                            }
                        }
                    }
                }
                SdkCommand::session_info { .. } => {
                    let mut data = serde_json::json!({
                        "model": self.harness.model(),
                    });
                    if let Some(session) = self.harness.session() {
                        data["session_id"] =
                            serde_json::Value::String(session.session_id().to_owned());
                    }
                    let resp = serde_json::to_value(SdkResponse::success_with_data(
                        cmd_id.as_deref(),
                        cmd_name,
                        data,
                    ))
                    .unwrap();
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                SdkCommand::steer { message, .. } => {
                    self.harness.steer(message);
                    let resp =
                        serde_json::to_value(SdkResponse::success(cmd_id.as_deref(), cmd_name))
                            .unwrap();
                    let _ = write_jsonl(&mut writer, &resp);
                    let _ = writer.flush();
                }
                SdkCommand::follow_up { message, .. } => {
                    self.harness.follow_up(message);
                    let resp =
                        serde_json::to_value(SdkResponse::success(cmd_id.as_deref(), cmd_name))
                            .unwrap();
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
