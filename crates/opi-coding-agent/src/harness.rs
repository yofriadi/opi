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
use crate::prompt::SystemPromptBuilder;
use crate::tool::{BashTool, EditTool, GlobTool, GrepTool, ReadTool, WriteTool};

/// Harness wiring config, tools, system prompt, hooks, and Agent.
pub struct CodingHarness {
    agent: Agent,
    config: OpiConfig,
    system_prompt: String,
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
        )
    }

    /// Create a new harness with custom hooks.
    pub fn new_with_hooks(
        provider: Box<dyn Provider>,
        model: String,
        config: OpiConfig,
        workspace_root: PathBuf,
        hooks: Box<dyn AgentHooks>,
        user_system_prompt: Option<String>,
    ) -> Self {
        let tools = Self::build_tools(&workspace_root);
        let tool_defs: Vec<_> = tools.iter().map(|t| t.definition()).collect();
        let mut builder = SystemPromptBuilder::new().tools(tool_defs);
        if let Some(content) = user_system_prompt {
            builder = builder.user_system(content);
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

        let agent = Agent::new(
            provider,
            tools,
            model,
            Some(system_prompt.clone()),
            agent_config,
            hooks,
        );

        Self {
            agent,
            config,
            system_prompt,
        }
    }

    /// Add an extra tool to the harness (for testing with mock tools).
    pub fn add_tool(&mut self, tool: Box<dyn Tool>) {
        self.agent.add_tool(tool);
    }

    /// Send a user prompt and run the agent loop.
    pub async fn prompt(&mut self, text: &str) -> Result<Vec<AgentMessage>, AgentError> {
        self.agent.prompt(text).await
    }

    /// Continue the conversation with an additional message.
    pub async fn continue_(&mut self, text: &str) -> Result<Vec<AgentMessage>, AgentError> {
        self.agent.continue_(text).await
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

    fn build_tools(workspace_root: &Path) -> Vec<Box<dyn Tool>> {
        vec![
            Box::new(ReadTool::new(workspace_root.to_path_buf())),
            Box::new(WriteTool::new(workspace_root.to_path_buf())),
            Box::new(EditTool::new(workspace_root.to_path_buf())),
            Box::new(BashTool::new(workspace_root.to_path_buf())),
            Box::new(GlobTool::new(workspace_root.to_path_buf())),
            Box::new(GrepTool::new(workspace_root.to_path_buf())),
        ]
    }
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

/// Default hooks for the coding agent — pass-through message conversion.
struct CodingAgentHooks;

impl AgentHooks for CodingAgentHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        let mut result = Vec::new();
        for msg in messages {
            if let AgentMessage::Llm(m) = msg {
                result.push(m.clone());
            }
        }
        Ok(result)
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
