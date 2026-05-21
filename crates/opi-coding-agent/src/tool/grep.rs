use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GrepArgs {
    /// Regex pattern to search for.
    pub pattern: String,
}

pub struct GrepTool {
    workspace_root: PathBuf,
    schema: serde_json::Value,
}

impl GrepTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        let schema = schemars::schema_for!(GrepArgs);
        Self {
            workspace_root,
            schema: serde_json::to_value(&schema).unwrap_or_default(),
        }
    }
}

impl Tool for GrepTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "grep".into(),
            description: "Gitignore-aware regex search over file contents.".into(),
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
        let args: GrepArgs = match serde_json::from_value(arguments) {
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
        Box::pin(async move {
            let re = match regex::Regex::new(&pattern) {
                Ok(r) => r,
                Err(e) => {
                    return Ok(ToolResult {
                        content: vec![OutputContent::Text {
                            text: format!("invalid regex pattern: {e}"),
                        }],
                        details: None,
                        is_error: true,
                        terminate: false,
                    });
                }
            };

            let mut matches = Vec::new();
            let mut builder = ignore::WalkBuilder::new(&workspace_root);
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
                    let content = match std::fs::read_to_string(path) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    for line in content.lines() {
                        if re.is_match(line) {
                            let relative = path.strip_prefix(&workspace_root).unwrap_or(path);
                            matches.push(format!("{}: {}", relative.display(), line));
                        }
                    }
                }
            }

            let text = matches.join("\n");
            let details = serde_json::json!({
                "workspace_root": workspace_root.to_string_lossy(),
                "pattern": pattern,
                "match_count": matches.len(),
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
