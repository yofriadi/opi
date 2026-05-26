//! System prompt construction (S8.4).
//!
//! Assembles the layered system prompt sent to the provider:
//! 1. Base coding-agent instructions
//! 2. Tool descriptions from ToolDef
//! 3. User system prompt file
//! 4. Project context files (AGENTS.md / CLAUDE.md)

use opi_ai::message::ToolDef;

const BASE_INSTRUCTIONS: &str = "\
You are opi, an expert coding agent. You help users with software engineering \
tasks including reading, writing, and editing code, running commands, and \
searching codebases. Be concise and precise. Explain your reasoning when \
making changes.";

/// Builder for assembling the system prompt from layered components.
pub struct SystemPromptBuilder {
    tools: Vec<ToolDef>,
    user_system: Option<String>,
    context_files: Option<String>,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            user_system: None,
            context_files: None,
        }
    }

    /// Add tool definitions. Their names and descriptions are included in the prompt.
    pub fn tools(mut self, tools: Vec<ToolDef>) -> Self {
        self.tools = tools;
        self
    }

    /// Add user-provided system prompt content (from --system flag or config).
    pub fn user_system(mut self, content: impl Into<String>) -> Self {
        let s = content.into();
        self.user_system = if s.is_empty() { None } else { Some(s) };
        self
    }

    /// Add project context file content (from AGENTS.md / CLAUDE.md discovery).
    pub fn context_files(mut self, content: impl Into<String>) -> Self {
        let s = content.into();
        self.context_files = if s.is_empty() { None } else { Some(s) };
        self
    }

    /// Return the collected tool definitions for `Request.tools`.
    pub fn tool_definitions(&self) -> &[ToolDef] {
        &self.tools
    }

    /// Assemble and return the full system prompt string.
    pub fn build(self) -> String {
        let mut parts = Vec::new();

        // Layer 1: base instructions
        parts.push(BASE_INSTRUCTIONS.to_owned());

        // Layer 2: tool descriptions
        if !self.tools.is_empty() {
            let mut tool_section = String::from("Available tools:\n");
            for tool in &self.tools {
                tool_section.push_str(&format!("- {}: {}\n", tool.name, tool.description));
            }
            parts.push(tool_section);
        }

        // Layer 3: user system prompt
        if let Some(user) = self.user_system {
            parts.push(format!("User instructions:\n{}", user));
        }

        // Layer 4: project context files (AGENTS.md / CLAUDE.md)
        if let Some(context) = self.context_files {
            parts.push(format!("Project context:\n{}", context));
        }

        parts.join("\n\n")
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
