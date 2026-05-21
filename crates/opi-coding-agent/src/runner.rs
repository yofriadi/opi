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
use opi_ai::message::Message;
use opi_ai::provider::Provider;
use opi_ai::stream::AssistantStreamEvent;

use crate::config::OpiConfig;
use crate::harness::CodingHarness;
use crate::policy::is_mutating_tool;

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
    ) -> Self {
        let hooks = Box::new(NonInteractiveHooks { allow_mutating });
        let harness = CodingHarness::new_with_hooks(
            provider,
            model,
            config,
            workspace_root,
            hooks,
            user_system_prompt,
        );
        Self { harness }
    }

    /// Cancel the running operation.
    pub fn cancel(&self) {
        self.harness.cancel();
    }

    /// Run a single prompt and return captured output.
    pub async fn run(&mut self, prompt: &str) -> NonInteractiveResult {
        // Subscribe to capture text from TextDelta events
        let text_parts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let tp = text_parts.clone();
        self.harness.subscribe(Box::new(move |event| {
            if let AgentEvent::MessageUpdate {
                assistant_event, ..
            } = event
                && let AssistantStreamEvent::TextDelta { delta, .. } = assistant_event.as_ref()
                && let Ok(mut guard) = tp.lock()
            {
                guard.push(delta.clone());
            }
        }));

        match self.harness.prompt(prompt).await {
            Ok(messages) => {
                // Check for provider errors in assistant messages
                if let Some(error) = find_error_message(&messages) {
                    return NonInteractiveResult {
                        stdout: String::new(),
                        stderr: error,
                        exit_code: ExitCode::ProviderFailure as i32,
                    };
                }

                let stdout = text_parts.lock().map(|g| g.join("")).unwrap_or_default();
                NonInteractiveResult {
                    stdout,
                    stderr: String::new(),
                    exit_code: ExitCode::Success as i32,
                }
            }
            Err(AgentError::Cancelled) => NonInteractiveResult {
                stdout: String::new(),
                stderr: "cancelled".into(),
                exit_code: ExitCode::Interrupted as i32,
            },
            Err(AgentError::AuthFailed(e)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("authentication error: {e}"),
                exit_code: ExitCode::AuthFailure as i32,
            },
            Err(AgentError::Provider(e)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("provider error: {e}"),
                exit_code: ExitCode::ProviderFailure as i32,
            },
            Err(AgentError::Tool(e)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("tool error: {e}"),
                exit_code: ExitCode::ToolFailure as i32,
            },
            Err(AgentError::Hook(e)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("hook error: {e}"),
                exit_code: ExitCode::RuntimeFailure as i32,
            },
            Err(AgentError::MaxTurnsExceeded(n)) => NonInteractiveResult {
                stdout: String::new(),
                stderr: format!("max turns exceeded ({n})"),
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

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

/// Hooks for non-interactive mode with tool safety policy.
struct NonInteractiveHooks {
    allow_mutating: bool,
}

impl AgentHooks for NonInteractiveHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        let mut result = Vec::new();
        for msg in messages {
            if let AgentMessage::Llm(m) = msg {
                result.push(m.clone());
            }
        }
        Ok(result)
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
