use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
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
                    Ok(ToolResult {
                        content: vec![OutputContent::Text {
                            text: format!("invalid arguments: {e}"),
                        }],
                        details: None,
                        is_error: true,
                        terminate: false,
                    })
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
                    return Ok(ToolResult {
                        content: vec![OutputContent::Text {
                            text: format!("invalid glob pattern: {e}"),
                        }],
                        details: None,
                        is_error: true,
                        terminate: false,
                    });
                }
            };

            let search_root = if let Some(ref p) = scope_path {
                // Validate the scope path is within workspace
                match super::validate_workspace_path(&workspace_root, p) {
                    Ok(canonical) => {
                        // Must be a directory or the path prefix is valid
                        if canonical.is_file() {
                            return Ok(ToolResult {
                                content: vec![OutputContent::Text {
                                    text: format!("'{}' is not a directory", p),
                                }],
                                details: None,
                                is_error: true,
                                terminate: false,
                            });
                        }
                        canonical
                    }
                    Err(msg) => {
                        return Ok(ToolResult {
                            content: vec![OutputContent::Text { text: msg }],
                            details: None,
                            is_error: true,
                            terminate: false,
                        });
                    }
                }
            } else {
                workspace_root.clone()
            };

            let mut matched_paths = Vec::new();
            let mut builder = ignore::WalkBuilder::new(&search_root);
            builder
                .hidden(false)
                .git_ignore(true)
                .git_global(false)
                .git_exclude(false)
                .add_custom_ignore_filename(".gitignore");
            let walker = builder.build();

            for entry in walker.flatten() {
                if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    let path = entry.path();
                    let relative = path.strip_prefix(&workspace_root).unwrap_or(path);
                    if glob_matcher.is_match(relative) || glob_matcher.is_match(path) {
                        matched_paths.push(path.to_string_lossy().into_owned());
                    }
                }
            }

            let text = matched_paths.join("\n");
            let details = serde_json::json!({
                "workspace_root": workspace_root.to_string_lossy(),
                "pattern": pattern,
                "match_count": matched_paths.len(),
            });

            Ok(ToolResult {
                content: vec![OutputContent::Text { text }],
                details: Some(details),
                is_error: false,
                terminate: false,
            })
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Parallel
    }
}
