use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::PathPolicy;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadArgs {
    /// Relative path within workspace to read.
    pub path: String,
    /// 1-based line offset (optional, defaults to 1).
    pub offset: Option<usize>,
    /// Maximum number of lines to read (optional).
    pub limit: Option<usize>,
}

pub struct ReadTool {
    workspace_root: PathBuf,
    path_policy: PathPolicy,
    schema: serde_json::Value,
}

impl ReadTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self::new_with_policy(workspace_root, PathPolicy::WorkspaceOnly)
    }

    pub fn new_with_policy(workspace_root: PathBuf, path_policy: PathPolicy) -> Self {
        let schema = schemars::schema_for!(ReadArgs);
        Self {
            workspace_root,
            path_policy,
            schema: serde_json::to_value(&schema).unwrap_or_default(),
        }
    }
}

impl Tool for ReadTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "read".into(),
            description: "Read file content with optional line range.".into(),
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
        let args: ReadArgs = match serde_json::from_value(arguments) {
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
        let resolved_path =
            match super::resolve_tool_path(&self.workspace_root, &args.path, self.path_policy) {
                Ok(p) => p,
                Err(msg) => {
                    return Box::pin(async move {
                        Ok(ToolResult {
                            content: vec![OutputContent::Text { text: msg }],
                            details: None,
                            is_error: true,
                            terminate: false,
                        })
                    });
                }
            };
        let file_path = resolved_path.path;
        let inside_workspace = resolved_path.inside_workspace;
        let workspace_root = self.workspace_root.clone();
        let path_for_display = args.path.clone();
        Box::pin(async move {
            let content = match tokio::fs::read_to_string(&file_path).await {
                Ok(c) => c,
                Err(e) => {
                    return Ok(ToolResult {
                        content: vec![OutputContent::Text {
                            text: format!("failed to read {}: {e}", file_path.display()),
                        }],
                        details: None,
                        is_error: true,
                        terminate: false,
                    });
                }
            };

            let lines: Vec<&str> = content.lines().collect();
            let offset = args.offset.unwrap_or(1).saturating_sub(1);
            let offset = offset.min(lines.len());
            let selected: Vec<&str> = if let Some(limit) = args.limit {
                lines[offset..].iter().take(limit).copied().collect()
            } else {
                lines[offset..].to_vec()
            };

            let output = selected.join("\n");
            let details = serde_json::json!({
                "workspace_root": workspace_root.to_string_lossy(),
                "path": path_for_display,
                "resolved_path": file_path.to_string_lossy(),
                "inside_workspace": inside_workspace,
            });

            let text = format!("{}\n{}", file_path.display(), output);

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
