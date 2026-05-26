mod bash;
mod edit;
mod find;
mod glob;
mod grep;
mod ls;
mod read;
mod write;

pub use bash::BashTool;
pub use edit::EditTool;
pub use find::FindTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use ls::LsTool;
pub use read::ReadTool;
pub use write::WriteTool;

use std::path::{Path, PathBuf};

/// Verify that `user_path` resolves within `workspace_root`. Returns the
/// canonicalized file path on success, or an error message suitable for the
/// tool response.
///
/// Walks up the ancestor chain to find the nearest existing directory, then
/// canonicalizes that ancestor to resolve symlinks/junctions. This prevents
/// an intermediate symlink pointing outside the workspace from bypassing the
/// check when the deeper path components don't exist yet.
pub fn validate_workspace_path(workspace_root: &Path, user_path: &str) -> Result<PathBuf, String> {
    let resolved = workspace_root.join(user_path);
    let canonical_root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("cannot canonicalize workspace root: {e}"))?;

    // Try canonicalize first (handles symlinks, existing files).
    if let Ok(canonical) = std::fs::canonicalize(&resolved) {
        return if canonical.starts_with(&canonical_root) {
            Ok(canonical)
        } else {
            Err(format!(
                "path '{}' resolves outside the workspace",
                user_path
            ))
        };
    }

    // Path doesn't exist — walk up ancestors to find the nearest existing one.
    // Canonicalizing that ancestor resolves any symlinks/junctions in the chain.
    // Components are pushed in reverse order (leaf first), then reversed.
    let mut ancestor = resolved.as_path();
    let mut suffix_components: Vec<std::ffi::OsString> = Vec::new();
    while let Some(parent) = ancestor.parent() {
        if let Some(name) = ancestor.file_name() {
            suffix_components.push(name.to_os_string());
        }
        if let Ok(canonical_ancestor) = std::fs::canonicalize(parent) {
            suffix_components.reverse();
            let suffix: PathBuf = suffix_components.iter().collect();
            let canonical = canonical_ancestor.join(suffix);
            return if canonical.starts_with(&canonical_root) {
                Ok(canonical)
            } else {
                Err(format!(
                    "path '{}' resolves outside the workspace",
                    user_path
                ))
            };
        }
        ancestor = parent;
    }

    // No ancestor exists on disk — normalize by resolving `..` components
    // and check against the canonical root.
    let normalized = normalize_path_components(&resolved);
    if normalized.starts_with(&canonical_root) {
        Ok(normalized)
    } else {
        Err(format!(
            "path '{}' resolves outside the workspace",
            user_path
        ))
    }
}

/// Resolve `.` and `..` components without touching the filesystem.
fn normalize_path_components(path: &Path) -> PathBuf {
    let mut stack = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                stack.pop();
            }
            std::path::Component::CurDir => {}
            c => stack.push(c.as_os_str()),
        }
    }
    stack.iter().collect()
}
