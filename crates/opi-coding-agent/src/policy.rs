//! Tool safety policy for non-interactive mode (S8.4, S10).
//!
//! Also contains tool selection types and resolution for --tools / --no-tools /
//! --no-builtin-tools CLI flags (task 3.8).

/// Returns `true` if the tool is considered mutating (write, edit, bash).
pub fn is_mutating_tool(name: &str) -> bool {
    matches!(name, "write" | "edit" | "bash")
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
