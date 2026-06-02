//! Tool safety policy for non-interactive mode (S8.4, S10).
//!
//! Also contains tool selection types and resolution for --tools / --no-tools /
//! --no-builtin-tools CLI flags (task 3.8).

/// Returns `true` if the tool is considered mutating (write, edit, bash).
pub fn is_mutating_tool(name: &str) -> bool {
    matches!(name, "write" | "edit" | "bash")
}

/// Application mode used to resolve default active tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Interactive,
    NonInteractive,
}

/// Pi-aligned built-in policy order consumed by the harness after Task 2 wiring.
pub const BUILTIN_TOOL_NAMES: &[&str] = &[
    "read", "write", "edit", "bash", "grep", "find", "ls", "glob",
];

const CODING_DEFAULT_TOOLS: &[&str] = &["read", "write", "edit", "bash"];
const READ_ONLY_DEFAULT_TOOLS: &[&str] = &["read", "grep", "find", "ls", "glob"];

/// Resolved tool runtime config used to choose active tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRuntimeConfig {
    pub run_mode: RunMode,
    pub active_tool_names: Vec<String>,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ToolPolicyError {
    #[error("mutating tool '{tool}' requires --allow-mutating in non-interactive mode")]
    MutatingToolRequiresOptIn { tool: String },
}

impl ToolRuntimeConfig {
    pub fn resolve(
        run_mode: RunMode,
        allow_mutating: bool,
        selection: ToolSelection,
    ) -> Result<Self, ToolPolicyError> {
        let active_tool_names = resolve_active_tool_names(run_mode, allow_mutating, &selection)?;
        Ok(Self {
            run_mode,
            active_tool_names,
        })
    }
}

fn resolve_active_tool_names(
    run_mode: RunMode,
    allow_mutating: bool,
    selection: &ToolSelection,
) -> Result<Vec<String>, ToolPolicyError> {
    match selection {
        ToolSelection::Disabled | ToolSelection::NoBuiltin => Ok(Vec::new()),
        ToolSelection::Allowlist(names) => {
            if run_mode == RunMode::NonInteractive
                && !allow_mutating
                && let Some(tool) = names.iter().find(|name| is_mutating_tool(name))
            {
                return Err(ToolPolicyError::MutatingToolRequiresOptIn { tool: tool.clone() });
            }
            Ok(filter_tool_names(BUILTIN_TOOL_NAMES, selection))
        }
        ToolSelection::Default => {
            let names = match (run_mode, allow_mutating) {
                (RunMode::Interactive, _) | (RunMode::NonInteractive, true) => CODING_DEFAULT_TOOLS,
                (RunMode::NonInteractive, false) => READ_ONLY_DEFAULT_TOOLS,
            };
            Ok(names.iter().map(|name| (*name).to_owned()).collect())
        }
    }
}

// ---------------------------------------------------------------------------
// Tool selection (task 3.8)
// ---------------------------------------------------------------------------

/// Resolved tool selection state driven by CLI flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSelection {
    /// Use all default built-in tools.
    Default,
    /// Only include tools whose names appear in the allowlist.
    Allowlist(Vec<String>),
    /// No tools at all.
    Disabled,
    /// No built-in tools (reserved for Phase 4 extension/custom tools).
    NoBuiltin,
}

/// CLI tool flags to be resolved into a `ToolSelection`.
pub struct ToolFlags {
    /// Tool allowlist from `--tools <comma-separated-list>`.
    pub tools: Option<Vec<String>>,
    /// Disable all tools (`--no-tools`).
    pub no_tools: bool,
    /// Disable built-in tools (`--no-builtin-tools`).
    pub no_builtin_tools: bool,
}

/// Resolve tool flags into a `ToolSelection` with deterministic precedence:
///
/// `--no-tools` > `--tools` > `--no-builtin-tools` > default
pub fn resolve_tool_selection(flags: ToolFlags) -> ToolSelection {
    if flags.no_tools {
        ToolSelection::Disabled
    } else if let Some(tools) = flags.tools {
        ToolSelection::Allowlist(tools)
    } else if flags.no_builtin_tools {
        ToolSelection::NoBuiltin
    } else {
        ToolSelection::Default
    }
}

/// Filter a list of tool names based on the given selection.
///
/// Returns the subset of `all_names` that pass the selection filter,
/// preserving the original order.
pub fn filter_tool_names(all_names: &[&str], selection: &ToolSelection) -> Vec<String> {
    match selection {
        ToolSelection::Default => all_names.iter().map(|s| (*s).to_owned()).collect(),
        ToolSelection::Disabled | ToolSelection::NoBuiltin => Vec::new(),
        ToolSelection::Allowlist(names) => all_names
            .iter()
            .filter(|n| names.iter().any(|a| a == *n))
            .map(|s| (*s).to_owned())
            .collect(),
    }
}
