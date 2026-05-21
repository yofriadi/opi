//! Tool safety policy for non-interactive mode (S8.4, S10).

/// Returns `true` if the tool is considered mutating (write, edit, bash).
pub fn is_mutating_tool(name: &str) -> bool {
    matches!(name, "write" | "edit" | "bash")
}
