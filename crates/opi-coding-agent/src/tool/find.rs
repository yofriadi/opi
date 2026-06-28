use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use opi_agent::diagnostic::FsToolError;
use opi_agent::tool::result::WorkspaceRelation;
use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult, result};
use opi_ai::message::{OutputContent, ToolDef};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindArgs {
    /// Glob pattern to search for (e.g. "**/*.rs", "*.toml").
    pub pattern: String,
    /// Optional subdirectory to scope the search to (relative to workspace root).
    #[serde(default)]
    pub path: Option<String>,
}

pub struct FindTool {
    workspace_root: PathBuf,
    schema: serde_json::Value,
}

impl FindTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        let schema = schemars::schema_for!(FindArgs);
        Self {
            workspace_root,
            schema: serde_json::to_value(&schema).unwrap_or_default(),
        }
    }
}

impl Tool for FindTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "find".into(),
            description: "Gitignore-aware file discovery by glob pattern. Optionally scope search to a subdirectory.".into(),
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
        let args: FindArgs = match serde_json::from_value(arguments) {
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
        let pattern = args.pattern;
        let scope_path = args.path;

        Box::pin(async move {
            let glob_matcher = match globset::Glob::new(&pattern) {
                Ok(g) => g.compile_matcher(),
                Err(e) => {
                    return Ok(result::err(vec![OutputContent::Text {
                        text: format!("invalid glob pattern: {e}"),
                    }]));
                }
            };

            // Resolve the optional scope path through the shared resolver so the
            // workspace relation is recovered uniformly; unscoped searches walk
            // the workspace root (relation `inside`).
            let (search_root, workspace_relation) = if let Some(ref p) = scope_path {
                match super::resolve_tool_path(&workspace_root, p, super::PathPolicy::WorkspaceOnly)
                {
                    Ok(resolved) => {
                        if !resolved.path.exists() {
                            return Ok(super::fs_error_result(FsToolError::NotFound {
                                user_path: p.clone(),
                                resolved_path: Some(resolved.path.clone()),
                            }));
                        }
                        if !resolved.path.is_dir() {
                            return Ok(super::fs_error_result(FsToolError::NotADirectory {
                                path: resolved.path.clone(),
                            }));
                        }
                        (resolved.path, resolved.workspace_relation)
                    }
                    Err(e) => {
                        return Ok(super::fs_error_result(e));
                    }
                }
            } else {
                (workspace_root.clone(), WorkspaceRelation::Inside)
            };

            // Surface an unreadable search root instead of silently walking it to
            // zero entries. Traversal-level permission errors in nested dirs are
            // deferred to the nav-consistency task.
            if let Err(e) = std::fs::read_dir(&search_root)
                && e.kind() == std::io::ErrorKind::PermissionDenied
            {
                return Ok(super::fs_error_result(FsToolError::PermissionDenied {
                    path: search_root.clone(),
                }));
            }

            let mut matched_paths = Vec::new();
            let mut builder = ignore::WalkBuilder::new(&search_root);
            builder
                .hidden(false)
                .git_ignore(true)
                .git_global(false)
                .git_exclude(false)
                .add_custom_ignore_filename(".gitignore");
            let walker = builder.build();

            let mut non_utf8 = 0usize;
            for entry in walker.flatten() {
                if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    let path = entry.path();
                    let relative = path.strip_prefix(&workspace_root).unwrap_or(path);
                    if glob_matcher.is_match(relative) || glob_matcher.is_match(path) {
                        if path.to_str().is_none() {
                            // Non-UTF-8 entry name (Unix-only in practice): skip
                            // and report via UnsupportedEncoding, not U+FFFD.
                            non_utf8 += 1;
                            continue;
                        }
                        matched_paths.push(path.to_string_lossy().into_owned());
                    }
                }
            }

            let text = matched_paths.join("\n");
            let details = serde_json::json!({
                "workspace_root": workspace_root.to_string_lossy(),
                "pattern": pattern,
                "match_count": matched_paths.len(),
                "workspace_relation": workspace_relation,
            });

            let mut tool_result = result::ok(vec![OutputContent::Text { text }], details);
            if non_utf8 > 0 {
                tool_result.diagnostics.push(
                    FsToolError::UnsupportedEncoding {
                        omitted_count: non_utf8,
                    }
                    .to_diagnostic(),
                );
            }
            Ok(tool_result)
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Parallel
    }
}
