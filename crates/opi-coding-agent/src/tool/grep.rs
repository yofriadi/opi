use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult, result};
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
            description: "Gitignore-aware regex search over file contents. \
                Matches are sorted by relative path then line number and capped \
                at a fixed limit."
                .into(),
            input_schema: self.schema.clone(),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        arguments: serde_json::Value,
        signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        let args: GrepArgs = match serde_json::from_value(arguments) {
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
        Box::pin(async move {
            let re = match regex::Regex::new(&pattern) {
                Ok(r) => r,
                Err(e) => {
                    return Ok(result::err(vec![OutputContent::Text {
                        text: format!("invalid regex pattern: {e}"),
                    }]));
                }
            };

            // (relative path, line number, formatted line). Sorting by
            // (relative path, line number) gives the Phase 11.7 lexicographic
            // relative-path order with intra-file line order preserved (a
            // plain string sort would reorder same-file lines by line text).
            let mut matches: Vec<(String, usize, String)> = Vec::new();
            let mut files_oversized_skipped = 0usize;
            let mut cancelled = false;
            let walker = super::nav_walk_builder(&workspace_root).build();

            for entry in walker.flatten() {
                // Honor the cancellation token mid-walk. The walker is a
                // blocking iterator, so poll the SYNC is_cancelled() (not
                // cancelled().await) per entry; on cancel, return the partial
                // results collected so far (cooperative, not an error).
                if signal.is_cancelled() {
                    cancelled = true;
                    break;
                }
                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    continue;
                }
                let path = entry.path();
                // File-size guardrail: stat before reading so a giant file
                // cannot dominate memory/time.
                let Ok(meta) = std::fs::metadata(path) else {
                    continue;
                };
                if meta.len() > super::MAX_NAV_FILE_BYTES {
                    files_oversized_skipped += 1;
                    continue;
                }
                let content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let relative = path
                    .strip_prefix(&workspace_root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned();
                for (index, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        matches.push((relative.clone(), index + 1, format!("{relative}: {line}")));
                    }
                }
            }

            matches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let lines: Vec<String> = matches
                .into_iter()
                .map(|(_, _, formatted)| formatted)
                .collect();
            let total = lines.len();
            let (text, truncated, omitted_count) =
                super::cap_nav_results(lines, &format!("no matches for pattern: {pattern}"));

            let details = serde_json::json!({
                "workspace_root": workspace_root.to_string_lossy(),
                "pattern": pattern,
                "match_count": total,
                "workspace_relation": result::WorkspaceRelation::Inside,
                "truncated": truncated,
                "omitted_count": omitted_count,
                "cancelled": cancelled,
                "files_oversized_skipped": files_oversized_skipped,
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
