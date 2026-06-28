use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult, result};
use opi_ai::message::{OutputContent, ToolDef};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

const DEFAULT_MAX_ENTRIES: usize = 200;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LsArgs {
    /// Directory path to list (relative to workspace root, use "." for root).
    pub path: String,
    /// Maximum number of entries to return. Defaults to 200.
    #[serde(default)]
    pub max_entries: Option<usize>,
    /// Maximum recursion depth. 0 lists only the specified directory, 1 includes
    /// immediate children and their types, etc. Defaults to 0 (flat listing).
    #[serde(default)]
    pub max_depth: Option<usize>,
}

pub struct LsTool {
    workspace_root: PathBuf,
    schema: serde_json::Value,
}

impl LsTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        let schema = schemars::schema_for!(LsArgs);
        Self {
            workspace_root,
            schema: serde_json::to_value(&schema).unwrap_or_default(),
        }
    }
}

impl Tool for LsTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "ls".into(),
            description: "List directory contents with bounded output. Entries are sorted deterministically. Directories are indicated with a trailing /.".into(),
            input_schema: self.schema.clone(),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        arguments: serde_json::Value,
        _signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        let args: LsArgs = match serde_json::from_value(arguments) {
            Ok(a) => a,
            Err(e) => {
                return Box::pin(async move {
                    Ok(result::err(vec![OutputContent::Text {
                        text: format!("invalid arguments: {e}"),
                    }]))
                });
            }
        };
        let workspace_root = self.workspace_root.clone();
        let max_entries = args.max_entries.unwrap_or(DEFAULT_MAX_ENTRIES);
        let max_depth = args.max_depth.unwrap_or(0);
        let path_arg = args.path;

        Box::pin(async move {
            // Resolve the target directory through the shared resolver so the
            // workspace relation is recovered uniformly ("." resolves to root).
            let resolved = match super::resolve_tool_path(
                &workspace_root,
                &path_arg,
                super::PathPolicy::WorkspaceOnly,
            ) {
                Ok(r) => r,
                Err(msg) => {
                    return Ok(result::err(vec![OutputContent::Text { text: msg }]));
                }
            };
            let target = resolved.path;
            let workspace_relation = resolved.workspace_relation;

            if !target.exists() {
                return Ok(result::err(vec![OutputContent::Text {
                    text: format!("path '{}' does not exist", path_arg),
                }]));
            }

            if !target.is_dir() {
                return Ok(result::err(vec![OutputContent::Text {
                    text: format!("'{}' is not a directory", path_arg),
                }]));
            }

            // Read and sort directory entries
            let mut entries: Vec<Entry> = Vec::new();
            collect_entries(&workspace_root, &target, &mut entries, 0, max_depth);

            entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

            let total_entries = entries.len();
            let truncated = total_entries > max_entries;
            entries.truncate(max_entries);

            let mut lines: Vec<String> = entries
                .iter()
                .map(|e| {
                    if e.is_dir {
                        format!("{}/", e.relative_path)
                    } else {
                        e.relative_path.clone()
                    }
                })
                .collect();

            if truncated {
                lines.push(format!(
                    "... (truncated, {} entries omitted)",
                    total_entries - max_entries
                ));
            }

            let text = lines.join("\n");
            let details = serde_json::json!({
                "workspace_root": workspace_root.to_string_lossy(),
                "path": path_arg,
                "entry_count": entries.len(),
                "total_entries": total_entries,
                "truncated": truncated,
                "workspace_relation": workspace_relation,
            });

            let mut tool_result = result::ok(vec![OutputContent::Text { text }], details);
            tool_result.truncated = truncated;
            Ok(tool_result)
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Parallel
    }
}

struct Entry {
    relative_path: String,
    is_dir: bool,
}

fn collect_entries(
    workspace_root: &std::path::Path,
    dir: &std::path::Path,
    entries: &mut Vec<Entry>,
    current_depth: usize,
    max_depth: usize,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(workspace_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let is_dir = path.is_dir();

        // Skip gitignored entries
        if is_gitignored(workspace_root, &path) {
            continue;
        }

        entries.push(Entry {
            relative_path: relative.clone(),
            is_dir,
        });

        if is_dir && current_depth < max_depth {
            collect_entries(workspace_root, &path, entries, current_depth + 1, max_depth);
        }
    }
}

fn is_gitignored(workspace_root: &std::path::Path, path: &std::path::Path) -> bool {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(workspace_root);
    // Load .gitignore if present
    let gitignore_path = workspace_root.join(".gitignore");
    if gitignore_path.exists() {
        let _ = builder.add(&gitignore_path);
    }
    match builder.build() {
        Ok(gi) => {
            let relative = path.strip_prefix(workspace_root).unwrap_or(path);
            gi.matched(relative, path.is_dir()).is_ignore()
        }
        Err(_) => false,
    }
}
