//! CLI argument parsing (S8.4).

use std::path::PathBuf;

use clap::Parser;

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

    /// Enable debug tracing.
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Initial prompt (positional).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub prompt: Vec<String>,
}
