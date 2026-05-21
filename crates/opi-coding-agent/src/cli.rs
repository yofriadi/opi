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

    /// Enable debug tracing.
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Initial prompt (positional).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub prompt: Vec<String>,
}
