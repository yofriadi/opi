//! Interactive CLI harness (S8.4).
//!
//! Wires together config, tools, system prompt, hooks, and Agent into a
//! single entry point for the interactive coding agent.

use std::path::{Path, PathBuf};

use opi_agent::Agent;
use opi_agent::event::AgentEvent;
use opi_agent::hooks::AgentHooks;
use opi_agent::loop_types::{AgentError, AgentLoopConfig};
use opi_agent::message::AgentMessage;
use opi_agent::tool::Tool;
use opi_ai::message::Message;
use opi_ai::provider::Provider;

use crate::config::OpiConfig;
use crate::context_files;
use crate::policy::ToolSelection;
use crate::prompt::SystemPromptBuilder;
use crate::session_coordinator::{SessionCoordinator, to_wire_result};
use crate::tool::{BashTool, EditTool, FindTool, GlobTool, GrepTool, LsTool, ReadTool, WriteTool};

/// Optional pre-existing session the harness can adopt instead of creating
/// a new JSONL file. Produced by `--resume` flows.
pub struct ResumeInfo {
    pub path: PathBuf,
    pub session_id: String,
    pub entries: Vec<opi_agent::session::SessionEntry>,
    /// The workspace cwd recorded in the session header. Used to restore the
    /// correct workspace root when resuming from a different directory.
    pub original_cwd: PathBuf,
}

/// Harness wiring config, tools, system prompt, hooks, and Agent.
pub struct CodingHarness {
    agent: Agent,
    config: OpiConfig,
    system_prompt: String,
    session: Option<SessionCoordinator>,
    /// Message count before the current turn — used to slice only new messages for persistence.
    turn_offset: usize,
}

impl CodingHarness {
    /// Create a new harness with the given provider, model, config, and workspace root.
    pub fn new(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
    ) -> Self {
        Self::new_with_hooks(
            provider,
            model,
            config,
            workspace_root,
            Box::new(CodingAgentHooks),
            None,
            Vec::new(),
            ToolSelection::Default,
        )
    }

    /// Create a new harness with an explicit tool selection.
    pub fn new_with_selection(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        tool_selection: ToolSelection,
    ) -> Self {
        Self::new_with_hooks(
            provider,
            model,
            config,
            workspace_root,
            Box::new(CodingAgentHooks),
            None,
            Vec::new(),
            tool_selection,
        )
    }

    /// Create a new harness with custom hooks.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_hooks(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        hooks: Box<dyn AgentHooks>,
        user_system_prompt: Option<String>,
        initial_messages: Vec<AgentMessage>,
        tool_selection: ToolSelection,
    ) -> Self {
        Self::new_with_hooks_and_resume(
            provider,
            model,
            config,
            workspace_root,
            hooks,
            user_system_prompt,
            initial_messages,
            None,
            tool_selection,
        )
    }

    /// Create a new harness, optionally adopting an existing session (resume).
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_hooks_and_resume(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        hooks: Box<dyn AgentHooks>,
        user_system_prompt: Option<String>,
        initial_messages: Vec<AgentMessage>,
        resume: Option<ResumeInfo>,
        tool_selection: ToolSelection,
    ) -> Self {
        let tools = Self::build_tools(&workspace_root, &tool_selection);
        let tool_defs: Vec<_> = tools.iter().map(|t| t.definition()).collect();
        let mut builder = SystemPromptBuilder::new().tools(tool_defs);
        if let Some(content) = user_system_prompt {
            builder = builder.user_system(content);
        }
        let context = context_files::discover_context_files(&workspace_root, None);
        if !context.content.is_empty() {
            builder = builder.context_files(context.content);
        }
        let system_prompt = builder.build();

        let agent_config = AgentLoopConfig {
            max_turns: config.defaults.max_iterations,
            retry: Some(config.retry.clone()),
            thinking: if config.thinking.enabled {
                Some(opi_ai::provider::ThinkingConfig {
                    enabled: true,
                    budget_tokens: Some(config.thinking.budget_tokens as u64),
                })
            } else {
                None
            },
            ..Default::default()
        };

        let mut agent = Agent::new(
            provider,
            tools,
            model.clone(),
            Some(system_prompt.clone()),
            agent_config,
            hooks,
        );

        let initial_len = initial_messages.len();
        if !initial_messages.is_empty() {
            agent.set_initial_messages(initial_messages);
        }

        let cwd = if let Some(ref info) = resume {
            // When resuming, use the workspace cwd from the session header so
            // tools operate in the correct workspace even if the process was
            // launched from a different directory.
            info.original_cwd.to_string_lossy().into_owned()
        } else {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let compaction_config = opi_agent::compaction::CompactionConfig {
            enabled: config.compaction.enabled,
            threshold_tokens: config.compaction.threshold_tokens,
        };

        let session = if let Some(info) = resume {
            SessionCoordinator::open_existing(
                info.path,
                info.session_id,
                &info.entries,
                initial_len,
                compaction_config,
                model.clone(),
            )
            .ok()
        } else {
            let session_dir = crate::session_cli::session_dir();
            SessionCoordinator::new(&session_dir, &cwd, compaction_config, model.clone()).ok()
        };

        Self {
            agent,
            config,
            system_prompt,
            session,
            turn_offset: initial_len,
        }
    }

    /// Add an extra tool to the harness (for testing with mock tools).
    pub fn add_tool(&mut self, tool: Box<dyn Tool>) {
        self.agent.add_tool(tool);
    }

    /// Send a user prompt and run the agent loop.
    pub async fn prompt(&mut self, text: &str) -> Result<Vec<AgentMessage>, AgentError> {
        let offset = self.turn_offset;
        let messages = self.agent.prompt(text).await?;
        let new = &messages[offset..];
        self.persist_turn(new, offset);
        let final_messages = self.current_messages();
        self.turn_offset = final_messages.len();
        Ok(final_messages)
    }

    /// Continue the conversation with an additional message.
    pub async fn continue_(&mut self, text: &str) -> Result<Vec<AgentMessage>, AgentError> {
        let offset = self.turn_offset;
        let messages = self.agent.continue_(text).await?;
        let new = &messages[offset..];
        self.persist_turn(new, offset);
        let final_messages = self.current_messages();
        self.turn_offset = final_messages.len();
        Ok(final_messages)
    }

    /// Sum usage across every assistant message produced during a turn.
    ///
    /// A single user prompt can drive multiple provider calls (e.g.
    /// tool-call response followed by a final response). Each emitted
    /// assistant message carries its own `usage`; the cumulative session
    /// total must include all of them, not just the last one.
    fn aggregate_turn_usage(messages: &[AgentMessage]) -> opi_ai::stream::Usage {
        let mut total = opi_ai::stream::Usage::default();
        for m in messages {
            if let AgentMessage::Llm(Message::Assistant(a)) = m {
                total.input_tokens = total.input_tokens.saturating_add(a.usage.input_tokens);
                total.output_tokens = total.output_tokens.saturating_add(a.usage.output_tokens);
                total.cache_read_tokens = total
                    .cache_read_tokens
                    .saturating_add(a.usage.cache_read_tokens);
                total.cache_write_tokens = total
                    .cache_write_tokens
                    .saturating_add(a.usage.cache_write_tokens);
            }
        }
        total
    }

    /// Aggregate usage across all assistant messages in a turn and persist.
    ///
    /// If compaction was triggered during persistence, this also rewrites
    /// the Agent's message buffer to `[summary, ...kept]` so subsequent
    /// provider calls no longer carry the compacted history. Emits
    /// `CompactionStart`/`CompactionEnd` events for subscribers.
    fn persist_turn(&mut self, messages: &[AgentMessage], turn_start_agent_index: usize) {
        if let Some(session) = &mut self.session {
            let usage = Self::aggregate_turn_usage(messages);
            let compaction_reason =
                match session.on_turn_end(messages, &usage, turn_start_agent_index) {
                    Ok(reason) => reason,
                    Err(e) => {
                        self.agent.emit_event(AgentEvent::SessionPersistError {
                            message: format!("session write failed: {e}"),
                        });
                        return;
                    }
                };

            if let Some(reason) = compaction_reason {
                self.agent
                    .emit_event(AgentEvent::CompactionStart { reason });
                match session.execute_compaction(reason) {
                    Ok(Some(out)) => {
                        let wire = to_wire_result(&out);
                        self.agent.replace_messages(out.new_agent_messages);
                        self.agent.emit_event(AgentEvent::CompactionEnd {
                            reason,
                            result: Some(wire),
                            aborted: false,
                            error_message: None,
                        });
                    }
                    Ok(None) => {
                        self.agent.emit_event(AgentEvent::CompactionEnd {
                            reason,
                            result: None,
                            aborted: true,
                            error_message: Some("compaction produced no output".into()),
                        });
                    }
                    Err(e) => {
                        // Compaction marker failed to persist — leave in-memory
                        // state un-compacted (SessionCoordinator already skipped
                        // the mutation) and surface the error to subscribers.
                        self.agent.emit_event(AgentEvent::CompactionEnd {
                            reason,
                            result: None,
                            aborted: true,
                            error_message: Some(format!("compaction persist failed: {e}")),
                        });
                        self.agent.emit_event(AgentEvent::SessionPersistError {
                            message: format!("compaction write failed: {e}"),
                        });
                    }
                }
            }
        }
    }

    /// Return the current message buffer (after any compaction).
    fn current_messages(&self) -> Vec<AgentMessage> {
        // The Agent's `set_initial_messages` / `replace_messages` API doesn't
        // expose a getter, so we re-derive the buffer from what was returned
        // by the loop plus any post-loop mutation. Simplest correct option:
        // ask the Agent via a new getter.
        self.agent.messages_snapshot()
    }

    /// Register an event subscriber.
    pub fn subscribe(&mut self, callback: Box<dyn Fn(&AgentEvent) + Send + Sync>) {
        self.agent.subscribe(callback);
    }

    /// Return the assembled system prompt (for testing).
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Return a reference to the config.
    pub fn config(&self) -> &OpiConfig {
        &self.config
    }

    /// Cancel the running operation.
    pub fn cancel(&self) {
        self.agent.abort();
    }

    /// Return a clonable cancellation token for external cancellation.
    pub fn cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.agent.cancel_token()
    }

    /// Return the session coordinator, if active.
    pub fn session(&self) -> Option<&SessionCoordinator> {
        self.session.as_ref()
    }

    fn build_tools(workspace_root: &Path, selection: &ToolSelection) -> Vec<Box<dyn Tool>> {
        let all_tools: Vec<Box<dyn Tool>> = vec![
            Box::new(ReadTool::new(workspace_root.to_path_buf())),
            Box::new(WriteTool::new(workspace_root.to_path_buf())),
            Box::new(EditTool::new(workspace_root.to_path_buf())),
            Box::new(BashTool::new(workspace_root.to_path_buf())),
            Box::new(GlobTool::new(workspace_root.to_path_buf())),
            Box::new(GrepTool::new(workspace_root.to_path_buf())),
            Box::new(FindTool::new(workspace_root.to_path_buf())),
            Box::new(LsTool::new(workspace_root.to_path_buf())),
        ];

        match selection {
            ToolSelection::Default => all_tools,
            ToolSelection::Disabled | ToolSelection::NoBuiltin => Vec::new(),
            ToolSelection::Allowlist(names) => all_tools
                .into_iter()
                .filter(|t| names.iter().any(|n| *n == t.definition().name))
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

/// Shared conversion of agent-level messages to the provider-facing Message
/// stream. Used by every hook in this crate so resume/compaction semantics
/// stay consistent between interactive and non-interactive paths.
///
/// - `AgentMessage::Llm` is forwarded directly.
/// - `AgentMessage::CompactionSummary` is rendered as a synthetic user
///   message so the provider sees a textual marker for context that was
///   compacted away.
/// - Other variants (`BranchSummary`, `Custom`) are dropped — they have no
///   provider-facing representation yet.
pub(crate) fn agent_messages_to_llm(messages: &[AgentMessage]) -> Vec<Message> {
    let mut result = Vec::with_capacity(messages.len());
    for msg in messages {
        match msg {
            AgentMessage::Llm(m) => result.push(m.clone()),
            AgentMessage::CompactionSummary(summary) => {
                result.push(Message::User(opi_ai::message::UserMessage {
                    content: vec![opi_ai::message::InputContent::Text {
                        text: format!(
                            "[Context was compacted. Summary of earlier conversation: {}]",
                            summary.summary
                        ),
                    }],
                    timestamp_ms: 0,
                }));
            }
            _ => {}
        }
    }
    result
}

/// Default hooks for the coding agent -- pass-through message conversion.
pub struct CodingAgentHooks;

impl AgentHooks for CodingAgentHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(agent_messages_to_llm(messages))
    }
}

/// Interactive hooks that deny mutating tools unless auto-allowed.
pub struct InteractiveCodingHooks {
    pub allow_mutating: bool,
}

impl InteractiveCodingHooks {
    pub fn new(allow_mutating: bool) -> Self {
        Self { allow_mutating }
    }

    fn is_mutating_tool(name: &str) -> bool {
        matches!(name, "write" | "edit" | "bash")
    }
}

impl AgentHooks for InteractiveCodingHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(agent_messages_to_llm(messages))
    }

    fn before_tool_call(
        &self,
        ctx: opi_agent::hooks::BeforeToolCallContext,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = opi_agent::hooks::BeforeToolCallResult> + Send>,
    > {
        use opi_agent::hooks::BeforeToolCallResult;
        let allow = self.allow_mutating || !Self::is_mutating_tool(&ctx.tool_name);
        Box::pin(async move {
            if allow {
                BeforeToolCallResult::Allow
            } else {
                BeforeToolCallResult::Deny {
                    reason: format!(
                        "mutating tool '{}' blocked in interactive mode (use --allow-mutating to override)",
                        ctx.tool_name
                    ),
                }
            }
        })
    }
}
