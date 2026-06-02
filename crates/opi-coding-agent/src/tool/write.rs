use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteArgs {
    /// Relative path within workspace to write.
    pub path: String,
    /// Content to write.
    pub content: String,
}

pub struct WriteTool {
    workspace_root: PathBuf,
    schema: serde_json::Value,
}

impl WriteTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        let schema = schemars::schema_for!(WriteArgs);
        Self {
            workspace_root,
            schema: serde_json::to_value(&schema).unwrap_or_default(),
        }
    }
}

impl Tool for WriteTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "write".into(),
            description: "Create or replace a file with the given content.".into(),
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
        let args: WriteArgs = match serde_json::from_value(arguments) {
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
        let resolved_path = match super::resolve_tool_path(
            &self.workspace_root,
            &args.path,
            super::PathPolicy::WorkspaceOnly,
        ) {
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
            // Create parent directories if needed
            if let Some(parent) = file_path.parent()
                && let Err(e) = tokio::fs::create_dir_all(parent).await
            {
                return Ok(ToolResult {
                    content: vec![OutputContent::Text {
                        text: format!("failed to create directories: {e}"),
                    }],
                    details: None,
                    is_error: true,
                    terminate: false,
                });
            }

            if let Err(e) = tokio::fs::write(&file_path, &args.content).await {
                return Ok(ToolResult {
                    content: vec![OutputContent::Text {
                        text: format!("failed to write {}: {e}", file_path.display()),
                    }],
                    details: None,
                    is_error: true,
                    terminate: false,
                });
            }

            let details = serde_json::json!({
                "workspace_root": workspace_root.to_string_lossy(),
                "path": path_for_display,
                "resolved_path": file_path.to_string_lossy(),
                "inside_workspace": inside_workspace,
            });

            Ok(ToolResult {
                content: vec![OutputContent::Text {
                    text: format!("wrote {}", path_for_display),
                }],
                details: Some(details),
                is_error: false,
                terminate: false,
            })
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sequential
    }
}
