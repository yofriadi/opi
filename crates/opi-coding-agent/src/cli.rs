//! CLI argument parsing (S8.4).

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

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
#[command(
    name = "opi",
    version,
    about = "AI coding agent",
    after_long_help = "\
Tool policy:
  Interactive mode enables read, write, edit, and bash.
  Non-interactive/RPC mode defaults to read, grep, find, ls, and glob.
  write, edit, and bash require --allow-mutating or defaults.allow_mutating_tools = true outside interactive mode.
  --no-tools disables all tools; --tools is an allowlist; --no-builtin-tools removes built-ins.

Bash policy:
  bash runs one foreground command from the workspace root.
  Windows uses cmd /C; Unix uses sh -c.
  The default timeout is 30 seconds; timeout_secs overrides it per call.
  Combined stdout/stderr are capped at 64 KiB. Larger output sets truncated and may write the complete output path in details.full_output.
  This is a tool-selection check, not a permission popup or sandbox subsystem.
"
)]
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

    /// RPC JSONL mode: bidirectional command/event protocol over stdin/stdout.
    #[arg(long)]
    pub rpc: bool,

    /// List all sessions.
    #[arg(long)]
    pub list_sessions: bool,

    /// Resume a session by ID.
    #[arg(long)]
    pub resume: Option<String>,

    /// Fork a session by ID into a new session.
    #[arg(long)]
    pub fork: Option<String>,

    /// Delete a session by ID.
    #[arg(long)]
    pub delete_session: Option<String>,

    /// Generate shell completions to stdout.
    #[arg(long, value_name = "SHELL")]
    pub generate_completion: Option<ShellName>,

    /// Enable debug tracing.
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Write a versioned, redacted trace envelope to PATH for the run
    /// (non-interactive / `--json` only; 0.x unstable, opt-in).
    #[arg(long, value_name = "PATH")]
    pub trace: Option<PathBuf>,

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

    /// Package subcommand group.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Initial prompt (positional).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub prompt: Vec<String>,
}

/// Top-level subcommands for opi.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage extension packages.
    Package {
        #[command(subcommand)]
        command: PackageCommand,
    },
    /// Summarize local health across config, provider, package, session, tui, rpc.
    ///
    /// Makes no paid model calls or network checks by default. Distinct from
    /// `opi package doctor`.
    Doctor {
        /// Output diagnostics as NDJSON (one JSON object per line).
        #[arg(long)]
        json: bool,
        /// Comma-separated scope list (config,provider,package,session,tui,rpc).
        /// Default: all scopes.
        #[arg(long)]
        scope: Option<String>,
    },
}

/// Package management subcommands.
#[derive(Debug, Subcommand)]
pub enum PackageCommand {
    /// Add a package to the store.
    Add {
        /// Package source (local path or git URL).
        source: String,
        /// Use project-local scope (`.opi/packages.toml`).
        #[arg(short = 'l', long = "local")]
        local: bool,
    },
    /// Remove a package from the store.
    Remove {
        /// Package name or source to remove.
        name_or_source: String,
        /// Use project-local scope.
        #[arg(short = 'l', long = "local")]
        local: bool,
    },
    /// List installed packages.
    List {
        /// Output as JSON (one JSON object per line).
        #[arg(long)]
        json: bool,
    },
    /// Validate installed packages and report diagnostics.
    Doctor {
        /// Output diagnostics as JSON.
        #[arg(long)]
        json: bool,
    },
}
