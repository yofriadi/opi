//! RPC JSONL mode: bidirectional command/event protocol over stdin/stdout.
//!
//! RPC mode enables headless operation of the coding agent via a strict JSONL
//! protocol. Commands arrive on stdin (one JSON object per line), responses
//! and events are emitted on stdout (one JSON object per line). Diagnostics
//! go to stderr.
//!
//! # Protocol version
//!
//! This is an unstable 0.x protocol. The schema may change between minor
//! versions without notice. Clients MUST check `schema_version` in the
//! `rpc_ready` header.
//!
//! # Framing
//!
//! LF (`\n`) is the only record delimiter. Clients MUST split on `\n` only
//! and SHOULD strip a trailing `\r` if present.
//!
//! # Commands
//!
//! | Command           | Description                                      |
//! |-------------------|--------------------------------------------------|
//! | `prompt`          | Send user prompt, stream agent events            |
//! | `continue`        | Continue conversation with additional text       |
//! | `steer`           | Queue steering message during agent operation    |
//! | `follow_up`       | Queue follow-up message for after agent stops    |
//! | `abort`           | Cancel current agent operation                   |
//! | `set_model`       | Switch provider:model                            |
//! | `set_thinking_level` | Set reasoning/thinking level                  |
//! | `compact`         | Trigger manual compaction                        |
//! | `session_info`    | Query session metadata                           |
//! | `extension_command` | Dispatch a command to registered extensions    |
//! | `quit`            | Shut down the RPC session                        |
//!
//! # Responses and Errors
//!
//! Every command produces at most one `response` object. For `prompt` and
//! `continue`, `success: true` means the turn was accepted; subsequent agent
//! output arrives as asynchronous event lines. Errors after acceptance are
//! surfaced as events, not as a second response.
//!
//! `abort` cancels the active operation and succeeds immediately when a turn is
//! running. A second `abort` while idle is a successful no-op.

use std::io::{self, BufRead, Write as IoWrite};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use opi_agent::agent::AgentControl;
use opi_agent::event::AgentEvent;
use opi_agent::extension::ExtensionRegistry;
use opi_agent::loop_types::AgentError;
use opi_agent::message::AgentMessage;
use opi_agent::sdk::{SDK_SCHEMA_VERSION, SdkCommand, SdkResponse, agent_event_to_value};
use opi_agent::session_event::CompactionReason;
use opi_ai::provider::Provider;

use crate::config::OpiConfig;
use crate::harness::CodingHarness;
use crate::policy::{RunMode, ToolSelection};
use crate::runner::ExitCode;

const ACTIVE_RUN_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Re-export the SDK command type as the RPC command type.
pub type RpcCommand = SdkCommand;

/// Re-export the SDK schema version for crate-level access (e.g. tests).
pub const RPC_SCHEMA_VERSION: u32 = SDK_SCHEMA_VERSION;

enum RpcInput {
    Command(SdkCommand),
    ParseError(String),
}

enum ActiveRun {
    Prompt(String),
    Continue(String),
}

type RunResult = (CodingHarness, Result<Vec<AgentMessage>, AgentError>);

/// RPC runner that owns the harness and processes commands.
pub struct RpcRunner {
    harness: Option<CodingHarness>,
    control: AgentControl,
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
        tool_selection: ToolSelection,
        user_system_prompt: Option<String>,
        initial_messages: Vec<AgentMessage>,
    ) -> Result<Self, crate::policy::ToolPolicyError> {
        Self::new_with_optional_extension_registry(
            provider,
            model,
            config,
            workspace_root,
            allow_mutating,
            tool_selection,
            user_system_prompt,
            initial_messages,
            None,
        )
    }

    /// Create a new RPC runner with an in-process extension registry.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_extension_registry(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        allow_mutating: bool,
        tool_selection: ToolSelection,
        user_system_prompt: Option<String>,
        initial_messages: Vec<AgentMessage>,
        extension_registry: ExtensionRegistry,
    ) -> Result<Self, crate::policy::ToolPolicyError> {
        Self::new_with_optional_extension_registry(
            provider,
            model,
            config,
            workspace_root,
            allow_mutating,
            tool_selection,
            user_system_prompt,
            initial_messages,
            Some(extension_registry),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_optional_extension_registry(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        allow_mutating: bool,
        tool_selection: ToolSelection,
        user_system_prompt: Option<String>,
        initial_messages: Vec<AgentMessage>,
        extension_registry: Option<ExtensionRegistry>,
    ) -> Result<Self, crate::policy::ToolPolicyError> {
        let tool_config = crate::policy::ToolRuntimeConfig::resolve(
            RunMode::NonInteractive,
            allow_mutating,
            tool_selection.clone(),
        )?;
        let hooks = Box::new(crate::runner::NonInteractiveHooks::new(allow_mutating));
        let mut builder = CodingHarness::builder(provider, model, config, workspace_root)
            .hooks(hooks)
            .initial_messages(initial_messages)
            .tool_selection(tool_selection)
            .tool_config(tool_config);
        if let Some(prompt) = user_system_prompt {
            builder = builder.user_system_prompt(prompt);
        }
        if let Some(registry) = extension_registry {
            builder = builder.extension_registry(registry);
        }
        let harness = builder.build();
        let control = harness.control_handle();
        Ok(Self {
            harness: Some(harness),
            control,
            running: false,
        })
    }

    /// Return the assembled system prompt while the runner is idle.
    pub fn system_prompt(&self) -> Option<&str> {
        self.harness.as_ref().map(CodingHarness::system_prompt)
    }

    /// Run the RPC main loop over stdin/stdout. Returns an exit code.
    pub async fn run(&mut self) -> i32 {
        let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || {
            let stdin = io::stdin();
            let reader = io::BufReader::new(stdin.lock());
            for line in reader.lines() {
                let line = match line {
                    Ok(line) => line,
                    Err(_) => break,
                };
                let trimmed = line.trim_end_matches('\r').trim();
                if trimmed.is_empty() {
                    continue;
                }
                let input = match serde_json::from_str::<SdkCommand>(trimmed) {
                    Ok(command) => RpcInput::Command(command),
                    Err(e) => RpcInput::ParseError(format!("failed to parse command: {e}")),
                };
                if input_tx.send(input).is_err() {
                    break;
                }
            }
        });

        let stdout = io::stdout();
        let mut writer = io::BufWriter::new(stdout.lock());
        self.run_loop(input_rx, |value| {
            write_jsonl(&mut writer, value)
                .and_then(|_| writer.flush())
                .is_ok()
        })
        .await
    }

    /// Run the RPC main loop with in-process command and output channels.
    ///
    /// This is intended for tests and SDK-style embedders that already have
    /// structured commands. Stdin parsing is covered by `run`.
    pub async fn run_with_channels(
        &mut self,
        mut command_rx: tokio::sync::mpsc::UnboundedReceiver<SdkCommand>,
        output_tx: tokio::sync::mpsc::UnboundedSender<serde_json::Value>,
    ) -> i32 {
        let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                if input_tx.send(RpcInput::Command(command)).is_err() {
                    break;
                }
            }
        });

        self.run_loop(input_rx, |value| output_tx.send(value.clone()).is_ok())
            .await
    }

    async fn run_loop(
        &mut self,
        mut input_rx: tokio::sync::mpsc::UnboundedReceiver<RpcInput>,
        mut emit: impl FnMut(&serde_json::Value) -> bool,
    ) -> i32 {
        let header = serde_json::json!({
            "type": "rpc_ready",
            "schema_version": SDK_SCHEMA_VERSION,
            "mode": "rpc",
            "version": env!("CARGO_PKG_VERSION"),
        });
        if !emit(&header) {
            return ExitCode::RuntimeFailure as i32;
        }

        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<serde_json::Value>();
        let event_tx = Arc::new(event_tx);
        if let Some(harness) = self.harness.as_mut() {
            let etx = event_tx.clone();
            harness.subscribe(Box::new(move |event: &AgentEvent| {
                let _ = etx.send(agent_event_to_value(event));
            }));
        }

        let mut run_task: Option<tokio::task::JoinHandle<RunResult>> = None;

        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    if !emit(&event) {
                        return self
                            .runtime_failure_after_emit_failure(
                                &mut run_task,
                                &mut event_rx,
                                &mut emit,
                            )
                            .await;
                    }
                }
                input = input_rx.recv() => {
                    match input {
                        None => {
                            if !self
                                .shutdown_active_run(&mut run_task, &mut event_rx, &mut emit)
                                .await
                            {
                                return ExitCode::RuntimeFailure as i32;
                            }
                            drain_events(&mut event_rx, &mut emit);
                            return ExitCode::Success as i32;
                        }
                        Some(input) => match input {
                        RpcInput::ParseError(message) => {
                            let resp = response_error(None, "parse", &message);
                            if !emit(&resp) {
                                return self
                                    .runtime_failure_after_emit_failure(
                                        &mut run_task,
                                        &mut event_rx,
                                        &mut emit,
                                    )
                                    .await;
                            }
                        }
                        RpcInput::Command(command) => {
                            if command.is_quit() {
                                let cmd_id = command.id().map(String::from);
                                let cmd_name = command.command_name();
                                let resp = response_success(cmd_id.as_deref(), cmd_name);
                                if !emit(&resp) {
                                    return self
                                        .runtime_failure_after_emit_failure(
                                            &mut run_task,
                                            &mut event_rx,
                                            &mut emit,
                                        )
                                        .await;
                                }
                                if !self
                                    .shutdown_active_run(&mut run_task, &mut event_rx, &mut emit)
                                    .await
                                {
                                    return ExitCode::RuntimeFailure as i32;
                                }
                                drain_events(&mut event_rx, &mut emit);
                                return ExitCode::Success as i32;
                            }

                            if !self
                                .handle_command(command, &mut run_task, &mut emit)
                                .await
                            {
                                let _ = self
                                    .shutdown_active_run(&mut run_task, &mut event_rx, &mut emit)
                                    .await;
                                return ExitCode::RuntimeFailure as i32;
                            }
                        }
                        },
                    }
                }
                joined = async {
                    match run_task.as_mut() {
                        Some(task) => task.await,
                        None => std::future::pending().await,
                    }
                }, if run_task.is_some() => {
                    let _ = run_task.take();
                    if !self.complete_run_task(joined, &mut emit) {
                        return ExitCode::RuntimeFailure as i32;
                    }
                    drain_events(&mut event_rx, &mut emit);
                }
                else => {
                    if !self
                        .shutdown_active_run(&mut run_task, &mut event_rx, &mut emit)
                        .await
                    {
                        return ExitCode::RuntimeFailure as i32;
                    }
                    drain_events(&mut event_rx, &mut emit);
                    return ExitCode::Success as i32;
                }
            }
        }
    }

    async fn runtime_failure_after_emit_failure(
        &mut self,
        run_task: &mut Option<tokio::task::JoinHandle<RunResult>>,
        event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
        emit: &mut impl FnMut(&serde_json::Value) -> bool,
    ) -> i32 {
        let _ = self.shutdown_active_run(run_task, event_rx, emit).await;
        ExitCode::RuntimeFailure as i32
    }

    async fn shutdown_active_run(
        &mut self,
        run_task: &mut Option<tokio::task::JoinHandle<RunResult>>,
        event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
        emit: &mut impl FnMut(&serde_json::Value) -> bool,
    ) -> bool {
        if self.running {
            self.control.abort();
        }

        let Some(mut task) = run_task.take() else {
            self.running = false;
            return true;
        };

        match tokio::time::timeout(ACTIVE_RUN_SHUTDOWN_TIMEOUT, &mut task).await {
            Ok(joined) => {
                let ok = self.complete_run_task(joined, emit);
                drain_events(event_rx, emit);
                ok
            }
            Err(_) => {
                task.abort();
                let joined = task.await;
                let ok = self.complete_run_task(joined, emit);
                let timeout_event = serde_json::json!({
                    "type": "SessionPersistError",
                    "message": "rpc active run did not stop before shutdown timeout; task aborted",
                });
                drain_events(event_rx, emit);
                ok && emit(&timeout_event)
            }
        }
    }

    fn complete_run_task(
        &mut self,
        joined: Result<RunResult, tokio::task::JoinError>,
        emit: &mut impl FnMut(&serde_json::Value) -> bool,
    ) -> bool {
        self.running = false;
        match joined {
            Ok((harness, result)) => {
                self.harness = Some(harness);
                self.handle_agent_result(result);
                true
            }
            Err(e) => {
                let event = serde_json::json!({
                    "type": "SessionPersistError",
                    "message": format!("rpc run task failed: {e}"),
                });
                let _ = emit(&event);
                false
            }
        }
    }

    async fn handle_command(
        &mut self,
        command: SdkCommand,
        run_task: &mut Option<tokio::task::JoinHandle<RunResult>>,
        emit: &mut impl FnMut(&serde_json::Value) -> bool,
    ) -> bool {
        let cmd_id = command.id().map(String::from);
        let cmd_name = command.command_name();

        match command {
            SdkCommand::prompt { message, .. } => self.start_run(
                ActiveRun::Prompt(message),
                cmd_id.as_deref(),
                cmd_name,
                run_task,
                emit,
            ),
            SdkCommand::continue_ { message, .. } => self.start_run(
                ActiveRun::Continue(message),
                cmd_id.as_deref(),
                cmd_name,
                run_task,
                emit,
            ),
            SdkCommand::abort { .. } => {
                if self.running {
                    self.control.abort();
                }
                emit(&response_success(cmd_id.as_deref(), cmd_name))
            }
            SdkCommand::steer { message, .. } => {
                if self.running {
                    self.control.steer(message);
                } else if let Some(harness) = self.harness.as_ref() {
                    harness.steer(message);
                }
                emit(&response_success(cmd_id.as_deref(), cmd_name))
            }
            SdkCommand::follow_up { message, .. } => {
                if self.running {
                    self.control.follow_up(message);
                } else if let Some(harness) = self.harness.as_ref() {
                    harness.follow_up(message);
                }
                emit(&response_success(cmd_id.as_deref(), cmd_name))
            }
            SdkCommand::set_model { model, .. } => {
                if self.running {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "cannot change model while agent is running",
                    ));
                }
                if let Some(harness) = self.harness.as_mut() {
                    match harness.set_model_validated(model) {
                        Ok(model) => {
                            let data = serde_json::json!({ "model": model });
                            emit(&response_success_with_data(
                                cmd_id.as_deref(),
                                cmd_name,
                                data,
                            ))
                        }
                        Err(e) => emit(&response_error(cmd_id.as_deref(), cmd_name, &e)),
                    }
                } else {
                    emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "agent harness is unavailable",
                    ))
                }
            }
            SdkCommand::set_thinking_level { level, .. } => {
                if self.running {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "cannot change thinking level while agent is running",
                    ));
                }
                let Some(harness) = self.harness.as_mut() else {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "agent harness is unavailable",
                    ));
                };
                match harness.set_thinking_level(&level) {
                    Ok(state) => {
                        let data = serde_json::json!({
                            "level": state.level,
                            "enabled": state.enabled,
                            "budget_tokens": state.budget_tokens,
                        });
                        emit(&response_success_with_data(
                            cmd_id.as_deref(),
                            cmd_name,
                            data,
                        ))
                    }
                    Err(e) => emit(&response_error(cmd_id.as_deref(), cmd_name, &e)),
                }
            }
            SdkCommand::compact { .. } => {
                if self.running {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "cannot compact while agent is running",
                    ));
                }
                let Some(harness) = self.harness.as_mut() else {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "agent harness is unavailable",
                    ));
                };
                match harness.compact(CompactionReason::Manual) {
                    Ok(Some(result)) => {
                        let data = serde_json::json!({
                            "summary": result.summary,
                            "first_kept_entry_id": result.first_kept_entry_id,
                            "tokens_before": result.tokens_before,
                            "tokens_after": result.tokens_after,
                        });
                        emit(&response_success_with_data(
                            cmd_id.as_deref(),
                            cmd_name,
                            data,
                        ))
                    }
                    Ok(None) => emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "compaction produced no output",
                    )),
                    Err(e) => emit(&response_error(cmd_id.as_deref(), cmd_name, &e)),
                }
            }
            SdkCommand::session_info { .. } => {
                if self.running {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "cannot query session info while agent is running",
                    ));
                }
                let Some(harness) = self.harness.as_ref() else {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "agent harness is unavailable",
                    ));
                };
                let mut data = serde_json::json!({
                    "model": harness.model(),
                    "resources": harness.resource_metadata_json(),
                });
                if let Some(session) = harness.session() {
                    data["session_id"] = serde_json::Value::String(session.session_id().to_owned());
                }
                emit(&response_success_with_data(
                    cmd_id.as_deref(),
                    cmd_name,
                    data,
                ))
            }
            SdkCommand::extension_command { name, args, .. } => {
                if self.running {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "cannot dispatch extension command while agent is running",
                    ));
                }
                let Some(harness) = self.harness.as_ref() else {
                    return emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        "agent harness is unavailable",
                    ));
                };
                match harness
                    .dispatch_extension_command(&name, cmd_id.as_deref(), args)
                    .await
                {
                    Ok(Some(data)) => emit(&response_success_with_data(
                        cmd_id.as_deref(),
                        cmd_name,
                        data,
                    )),
                    Ok(None) => emit(&response_error(
                        cmd_id.as_deref(),
                        cmd_name,
                        &format!("extension command not handled: {name}"),
                    )),
                    Err(e) => emit(&response_error(cmd_id.as_deref(), cmd_name, &e)),
                }
            }
            SdkCommand::quit { .. } => true,
        }
    }

    fn start_run(
        &mut self,
        run: ActiveRun,
        id: Option<&str>,
        command: &str,
        run_task: &mut Option<tokio::task::JoinHandle<RunResult>>,
        emit: &mut impl FnMut(&serde_json::Value) -> bool,
    ) -> bool {
        if self.running {
            return emit(&response_error(
                id,
                command,
                "agent is already running; use steer or follow_up to queue messages",
            ));
        }

        if self.harness.is_none() {
            return emit(&response_error(id, command, "agent harness is unavailable"));
        }

        if !emit(&response_success(id, command)) {
            return false;
        }

        let mut harness = self.harness.take().expect("harness checked above");
        harness.reset_cancel_if_cancelled();
        self.control = harness.control_handle();
        self.running = true;

        *run_task = Some(tokio::spawn(async move {
            let result = match run {
                ActiveRun::Prompt(message) => harness.prompt(&message).await,
                ActiveRun::Continue(message) => harness.continue_(&message).await,
            };
            (harness, result)
        }));
        true
    }

    fn handle_agent_result(&self, result: Result<Vec<AgentMessage>, AgentError>) {
        match result {
            Ok(_) | Err(AgentError::Cancelled) => {}
            Err(_) => {}
        }
    }
}

fn response_success(id: Option<&str>, command: &str) -> serde_json::Value {
    serde_json::to_value(SdkResponse::success(id, command)).unwrap()
}

fn response_success_with_data(
    id: Option<&str>,
    command: &str,
    data: serde_json::Value,
) -> serde_json::Value {
    serde_json::to_value(SdkResponse::success_with_data(id, command, data)).unwrap()
}

fn response_error(id: Option<&str>, command: &str, message: &str) -> serde_json::Value {
    serde_json::to_value(SdkResponse::error(id, command, message)).unwrap()
}

/// Write a JSON value as a single line to the writer.
fn write_jsonl(writer: &mut dyn IoWrite, value: &serde_json::Value) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")
}

fn drain_events(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    emit: &mut impl FnMut(&serde_json::Value) -> bool,
) {
    while let Ok(event) = rx.try_recv() {
        if !emit(&event) {
            break;
        }
    }
}
