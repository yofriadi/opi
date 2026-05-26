//! Non-interactive runner (S10).
//!
//! Takes a single prompt, runs it through the agent, captures assistant text
//! for stdout, diagnostics for stderr, and returns an exit code.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use opi_agent::event::AgentEvent;
use opi_agent::hooks::{
    AfterToolCallContext, AfterToolCallResult, AgentHooks, BeforeToolCallContext,
    BeforeToolCallResult, PrepareNextTurnContext, ShouldStopAfterTurnContext,
};
use opi_agent::loop_types::AgentError;
use opi_agent::message::AgentMessage;
use opi_agent::session_event::{AgentSessionEvent, SessionCostTotals, SessionTokenTotals};
use opi_ai::message::Message;
use opi_ai::provider::Provider;
use opi_ai::stream::AssistantStreamEvent;

use crate::config::OpiConfig;
use crate::harness::{CodingHarness, ResumeInfo};
use crate::policy::{ToolSelection, is_mutating_tool};

/// NDJSON output schema version.
pub const NDJSON_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Exit codes (S10)
// ---------------------------------------------------------------------------

/// Exit codes for the non-interactive runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    RuntimeFailure = 1,
    ConfigError = 2,
    AuthFailure = 3,
    ProviderFailure = 4,
    ToolFailure = 5,
    Interrupted = 130,
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Captured output from a non-interactive run.
#[derive(Debug, Clone)]
pub struct NonInteractiveResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// Non-interactive runner that executes a single prompt and captures output.
pub struct NonInteractiveRunner {
    harness: CodingHarness,
}

impl NonInteractiveRunner {
    /// Create a new non-interactive runner.
    pub fn new(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        allow_mutating: bool,
        user_system_prompt: Option<String>,
        initial_messages: Vec<AgentMessage>,
    ) -> Self {
        Self::new_with_resume(
            provider,
            model,
            config,
            workspace_root,
            allow_mutating,
            user_system_prompt,
            initial_messages,
            None,
            ToolSelection::Default,
        )
    }

    /// Create a new non-interactive runner, optionally adopting an existing
    /// session (resume).
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_resume(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        allow_mutating: bool,
        user_system_prompt: Option<String>,
        initial_messages: Vec<AgentMessage>,
        resume_info: Option<ResumeInfo>,
        tool_selection: ToolSelection,
    ) -> Self {
        let hooks = Box::new(NonInteractiveHooks { allow_mutating });
        let harness = CodingHarness::new_with_hooks_and_resume(
            provider,
            model,
            config,
            workspace_root,
            hooks,
            user_system_prompt,
            initial_messages,
            resume_info,
            tool_selection,
        );
        Self { harness }
    }

    /// Run a single prompt in JSON mode, returning NDJSON output in stdout.
    pub async fn run_json(&mut self, prompt: &str) -> NonInteractiveResult {
        let output: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

        // Schema version header line
        {
            let header = serde_json::json!({
                "type": "session_header",
                "schema_version": NDJSON_SCHEMA_VERSION,
            });
            let mut out = output.lock().unwrap();
            out.push_str(&header.to_string());
            out.push('\n');
        }

        let out = output.clone();
        self.harness.subscribe(Box::new(move |event| {
            let session_event = match event {
                AgentEvent::AutoRetryStart {
                    attempt,
                    max_attempts,
                    delay_ms,
                    error_message,
                } => AgentSessionEvent::AutoRetryStart {
                    attempt: *attempt,
                    max_attempts: *max_attempts,
                    delay_ms: *delay_ms,
                    error_message: error_message.clone(),
                },
                AgentEvent::AutoRetryEnd {
                    success,
                    attempt,
                    final_error,
                } => AgentSessionEvent::AutoRetryEnd {
                    success: *success,
                    attempt: *attempt,
                    final_error: final_error.clone(),
                },
                AgentEvent::CompactionStart { reason } => {
                    AgentSessionEvent::CompactionStart { reason: *reason }
                }
                AgentEvent::CompactionEnd {
                    reason,
                    result,
                    aborted,
                    error_message,
                } => AgentSessionEvent::CompactionEnd {
                    reason: *reason,
                    result: result.clone(),
                    aborted: *aborted,
                    will_retry: false,
                    error_message: error_message.clone(),
                },
                _ => AgentSessionEvent::Agent {
                    event: event.clone(),
                },
            };
            if let Ok(json) = serde_json::to_string(&session_event)
                && let Ok(mut guard) = out.lock()
            {
                guard.push_str(&json);
                guard.push('\n');
            }
        }));

        let prompt_result = self.harness.prompt(prompt).await;

        // Emit a final `SessionSummary` event with cumulative token totals
        // and (when known) cost breakdown. Emitted before the result match so
        // even error paths surface what the user spent before failing.
        if let Some(session) = self.harness.session() {
            let usage = session.usage();
            let cost = session.cost_summary().map(|c| SessionCostTotals {
                input: c.input_cost,
                output: c.output_cost,
                cache_read: c.cache_read_cost,
                cache_write: c.cache_write_cost,
                total: c.total_cost(),
            });
            let summary_event = AgentSessionEvent::SessionSummary {
                session_id: session.session_id().to_owned(),
                model: session.model().to_owned(),
                turns: usage.turn_count(),
                tokens: SessionTokenTotals {
                    input: usage.total_input_tokens(),
                    output: usage.total_output_tokens(),
                    cache_read: usage.total_cache_read_tokens(),
                    cache_write: usage.total_cache_write_tokens(),
                },
                cost_usd: cost,
            };
            if let Ok(json) = serde_json::to_string(&summary_event)
                && let Ok(mut guard) = output.lock()
            {
                guard.push_str(&json);
                guard.push('\n');
            }
        }

        match prompt_result {
            Ok(messages) => {
                if let Some(error) = find_error_message(&messages) {
                    return NonInteractiveResult {
                        stdout: output.lock().map(|g| g.clone()).unwrap_or_default(),
                        stderr: error,
                        exit_code: ExitCode::ProviderFailure as i32,
                    };
                }
                NonInteractiveResult {
                    stdout: output.lock().map(|g| g.clone()).unwrap_or_default(),
                    stderr: String::new(),
                    exit_code: ExitCode::Success as i32,
                }
            }
            Err(AgentError::Cancelled) => NonInteractiveResult {
                stdout: output.lock().map(|g| g.clone()).unwrap_or_default(),
                stderr: "cancelled".into(),
                exit_code: ExitCode::Interrupted as i32,
            },
            Err(AgentError::AuthFailed(e)) => NonInteractiveResult {
                stdout: output.lock().map(|g| g.clone()).unwrap_or_default(),
                stderr: format!("authentication error: {e}"),
                exit_code: ExitCode::AuthFailure as i32,
            },
            Err(AgentError::Provider(e)) => NonInteractiveResult {
                stdout: output.lock().map(|g| g.clone()).unwrap_or_default(),
                stderr: format!("provider error: {e}"),
                exit_code: ExitCode::ProviderFailure as i32,
            },
            Err(AgentError::Tool(e)) => NonInteractiveResult {
                stdout: output.lock().map(|g| g.clone()).unwrap_or_default(),
                stderr: format!("tool error: {e}"),
                exit_code: ExitCode::ToolFailure as i32,
            },
            Err(AgentError::Hook(e)) => NonInteractiveResult {
                stdout: output.lock().map(|g| g.clone()).unwrap_or_default(),
                stderr: format!("hook error: {e}"),
                exit_code: ExitCode::RuntimeFailure as i32,
            },
            Err(AgentError::MaxTurnsExceeded(n)) => NonInteractiveResult {
                stdout: output.lock().map(|g| g.clone()).unwrap_or_default(),
                stderr: format!("max turns exceeded ({n})"),
                exit_code: ExitCode::RuntimeFailure as i32,
            },
        }
    }

    /// Cancel the running operation.
    pub fn cancel(&self) {
        self.harness.cancel();
    }

    /// Return the session coordinator, if active.
    pub fn session(&self) -> Option<&crate::session_coordinator::SessionCoordinator> {
        self.harness.session()
    }

    /// Run a single prompt and return captured output.
    pub async fn run(&mut self, prompt: &str) -> NonInteractiveResult {
        // Subscribe to capture text from TextDelta events and persist errors
        let text_parts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let persist_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let tp = text_parts.clone();
        let pe = persist_errors.clone();
        self.harness.subscribe(Box::new(move |event| match event {
            AgentEvent::MessageUpdate {
                assistant_event, ..
            } => {
                if let AssistantStreamEvent::TextDelta { delta, .. } = assistant_event.as_ref()
                    && let Ok(mut guard) = tp.lock()
                {
                    guard.push(delta.clone());
                }
            }
            AgentEvent::SessionPersistError { message } => {
                if let Ok(mut guard) = pe.lock() {
                    guard.push(message.clone());
                }
            }
            _ => {}
        }));

        let prompt_result = self.harness.prompt(prompt).await;

        // Format persist errors AFTER prompt returns so events emitted
        // during the run are captured.
        let persist_stderr = format_persist_errors(&persist_errors);

        match prompt_result {
            Ok(messages) => {
                // Check for provider errors in assistant messages
                if let Some(error) = find_error_message(&messages) {
                    let mut stderr = error;
                    stderr.push_str(&persist_stderr);
                    return NonInteractiveResult {
                        stdout: String::new(),
                        stderr,
                        exit_code: ExitCode::ProviderFailure as i32,
                    };
                }

                let stdout = text_parts.lock().map(|g| g.join("")).unwrap_or_default();
                NonInteractiveResult {
                    stdout,
                    stderr: persist_stderr,
                    exit_code: ExitCode::Success as i32,
                }
            }
            Err(AgentError::Cancelled) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("cancelled{persist_stderr}"),
                exit_code: ExitCode::Interrupted as i32,
            },
            Err(AgentError::AuthFailed(e)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("authentication error: {e}{persist_stderr}"),
                exit_code: ExitCode::AuthFailure as i32,
            },
            Err(AgentError::Provider(e)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("provider error: {e}{persist_stderr}"),
                exit_code: ExitCode::ProviderFailure as i32,
            },
            Err(AgentError::Tool(e)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("tool error: {e}{persist_stderr}"),
                exit_code: ExitCode::ToolFailure as i32,
            },
            Err(AgentError::Hook(e)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("hook error: {e}{persist_stderr}"),
                exit_code: ExitCode::RuntimeFailure as i32,
            },
            Err(AgentError::MaxTurnsExceeded(n)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("max turns exceeded ({n}){persist_stderr}"),
                exit_code: ExitCode::RuntimeFailure as i32,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the first error_message in assistant messages.
fn find_error_message(messages: &[AgentMessage]) -> Option<String> {
    for msg in messages {
        if let AgentMessage::Llm(Message::Assistant(asst)) = msg
            && let Some(err) = &asst.error_message
        {
            return Some(err.clone());
        }
    }
    None
}

/// Format any captured session persist errors into a stderr suffix.
pub fn format_persist_errors(errors: &Arc<Mutex<Vec<String>>>) -> String {
    let guard = errors.lock().unwrap();
    if guard.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for e in guard.iter() {
        out.push_str("\nsession persist error: ");
        out.push_str(e);
    }
    out
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

/// Hooks for non-interactive mode with tool safety policy.
struct NonInteractiveHooks {
    allow_mutating: bool,
}

impl AgentHooks for NonInteractiveHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(crate::harness::agent_messages_to_llm(messages))
    }

    fn before_tool_call(
        &self,
        ctx: BeforeToolCallContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BeforeToolCallResult> + Send>> {
        let allowed = self.allow_mutating;
        let tool_name = ctx.tool_name.clone();
        Box::pin(async move {
            if !allowed && is_mutating_tool(&tool_name) {
                return BeforeToolCallResult::Deny {
                    reason: format!(
                        "tool '{}' is not allowed in non-interactive mode without --allow-mutating",
                        tool_name
                    ),
                };
            }
            BeforeToolCallResult::Allow
        })
    }

    fn after_tool_call(
        &self,
        _ctx: AfterToolCallContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AfterToolCallResult> + Send>> {
        Box::pin(async { AfterToolCallResult::Keep })
    }

    fn should_stop_after_turn(
        &self,
        _ctx: ShouldStopAfterTurnContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        Box::pin(async { false })
    }

    fn prepare_next_turn(
        &self,
        _ctx: PrepareNextTurnContext,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Option<opi_agent::loop_types::AgentLoopTurnUpdate>>
                + Send,
        >,
    > {
        Box::pin(async { None })
    }
}
