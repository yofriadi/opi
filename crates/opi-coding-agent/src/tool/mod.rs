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

/// Path boundary policy for file tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPolicy {
    WorkspaceOnly,
    AllowOutsideWorkspace,
}

/// Resolved path metadata shared by file tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedToolPath {
    pub path: PathBuf,
    pub inside_workspace: bool,
}

/// Resolve a user-supplied file path for tool execution.
///
/// Relative paths are based on `workspace_root`; absolute paths are preserved.
/// A leading `@` is ignored for editor-style path mentions, and `~` expands
/// from HOME/USERPROFILE when available.
pub fn resolve_tool_path(
    workspace_root: &Path,
    user_path: &str,
    policy: PathPolicy,
) -> Result<ResolvedToolPath, String> {
    let expanded = expand_user_path(user_path);
    let resolved = if expanded.is_absolute() {
        expanded
    } else {
        workspace_root.join(expanded)
    };
    let canonical_root = std::fs::canonicalize(workspace_root)
        .map_err(|e| format!("cannot canonicalize workspace root: {e}"))?;
    let canonical = canonicalize_existing_or_nearest(&resolved)?;
    let inside_workspace = canonical.starts_with(&canonical_root);

    if policy == PathPolicy::WorkspaceOnly && !inside_workspace {
        return Err(format!(
            "path '{}' resolves outside the workspace",
            user_path
        ));
    }

    Ok(ResolvedToolPath {
        path: canonical,
        inside_workspace,
    })
}

pub fn validate_workspace_path(workspace_root: &Path, user_path: &str) -> Result<PathBuf, String> {
    resolve_tool_path(workspace_root, user_path, PathPolicy::WorkspaceOnly)
        .map(|resolved| resolved.path)
}

fn expand_user_path(user_path: &str) -> PathBuf {
    let path = user_path.strip_prefix('@').unwrap_or(user_path);
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"))
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn canonicalize_existing_or_nearest(path: &Path) -> Result<PathBuf, String> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Ok(canonical);
    }

    // Path doesn't exist, so canonicalize the nearest existing ancestor.
    // Preserve the original lexical suffix relative to that ancestor, then
    // normalize after joining so `..` segments are applied consistently.
    let mut ancestor = path;
    while let Some(parent) = ancestor.parent() {
        if let Ok(canonical_ancestor) = std::fs::canonicalize(parent) {
            let suffix = path.strip_prefix(parent).unwrap_or_else(|_| Path::new(""));
            return Ok(normalize_path_components(&canonical_ancestor.join(suffix)));
        }
        ancestor = parent;
    }

    Ok(normalize_path_components(path))
}

/// Resolve `.` and `..` components without touching the filesystem.
fn normalize_path_components(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            c => normalized.push(c.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_user_path_strips_at_prefix() {
        let path = expand_user_path("@Cargo.toml");
        assert_eq!(path, PathBuf::from("Cargo.toml"));
    }

    #[test]
    fn normalize_path_components_removes_parent_segments() {
        let path = normalize_path_components(Path::new("/tmp/a/../b"));
        assert!(path.ends_with(Path::new("tmp").join("b")));
    }

    #[test]
    fn canonicalize_existing_or_nearest_normalizes_missing_suffix_parent_segments() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("missing/child/../../target.txt");
        let resolved = canonicalize_existing_or_nearest(&path).unwrap();

        assert_eq!(
            resolved,
            std::fs::canonicalize(workspace.path())
                .unwrap()
                .join("target.txt")
        );
    }
}
