//! CLI argument parsing (S8.4).

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// Supported shells for completion generation.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShellName {
    Bash,
    Zsh,
    Fish,
    #[clap(name = "powershell")]
    PowerShell,
    Elvish,
}

impl From<ShellName> for clap_complete::Shell {
    fn from(s: ShellName) -> Self {
        match s {
            ShellName::Bash => clap_complete::Shell::Bash,
            ShellName::Zsh => clap_complete::Shell::Zsh,
            ShellName::Fish => clap_complete::Shell::Fish,
            ShellName::PowerShell => clap_complete::Shell::PowerShell,
            ShellName::Elvish => clap_complete::Shell::Elvish,
        }
    }
}

/// opi — AI coding agent.
#[derive(Debug, Parser)]
#[command(name = "opi", version, about = "AI coding agent")]
pub struct Cli {
    /// Model spec, e.g. anthropic:claude-sonnet-4.
    #[arg(short = 'm', long)]
    pub model: Option<String>,

    /// Config file path.
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// System prompt file.
    #[arg(short = 's', long)]
    pub system: Option<PathBuf>,

    /// Single prompt mode (non-interactive).
    #[arg(long)]
    pub non_interactive: bool,

    /// Allow mutating tools (write, edit, bash) in non-interactive mode.
    #[arg(long)]
    pub allow_mutating: bool,

    /// Output NDJSON events to stdout (non-interactive mode).
    #[arg(long)]
    pub json: bool,

    /// List all sessions.
    #[arg(long)]
    pub list_sessions: bool,

    /// Resume a session by ID.
    #[arg(long)]
    pub resume: Option<String>,

    /// Delete a session by ID.
    #[arg(long)]
    pub delete_session: Option<String>,

    /// Generate shell completions to stdout.
    #[arg(long, value_name = "SHELL")]
    pub generate_completion: Option<ShellName>,

    /// Enable debug tracing.
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Tool allowlist (comma-separated, e.g. "read,glob").
    #[arg(long, value_delimiter = ',')]
    pub tools: Option<Vec<String>>,

    /// Disable all tools.
    #[arg(long)]
    pub no_tools: bool,

    /// Disable built-in tools (reserved for Phase 4 extension tools).
    #[arg(long)]
    pub no_builtin_tools: bool,

    /// Attach image file(s) to the prompt.
    #[arg(long)]
    pub image: Vec<PathBuf>,

    /// List available models and exit.
    #[arg(long)]
    pub list_models: bool,

    /// Initial prompt (positional).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub prompt: Vec<String>,
}
