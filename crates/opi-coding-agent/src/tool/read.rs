use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use opi_agent::diagnostic::{FsToolError, code};
use opi_agent::tool::ToolDiagnostic;
use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult, result};
use opi_ai::message::{OutputContent, ToolDef};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::PathPolicy;

/// Default number of lines returned when the caller omits `limit`.
///
/// Bounds output for the model without special-casing the explicit-window
/// contract: when the caller supplies `limit`, that value is honored exactly
/// (an explicit `limit > DEFAULT_READ_LINES` is returned in full). Byte-level
/// capping of pathological single-line files is intentionally out of scope for
/// Phase 11.3; the cap is line-based, matching the line-oriented API.
const DEFAULT_READ_LINES: usize = 2000;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadArgs {
    /// Relative path within workspace to read.
    pub path: String,
    /// 1-based line offset (optional, defaults to 1).
    pub offset: Option<usize>,
    /// Maximum number of lines to read (optional, defaults to
    /// [`DEFAULT_READ_LINES`]).
    ///
    /// When omitted, output is capped at `DEFAULT_READ_LINES` lines and the
    /// remainder is reported via `truncated`/`omitted`. When supplied, that many
    /// lines are returned exactly (no default cap is reapplied); `limit: 0`
    /// returns no lines and flags the result truncated.
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
                    Ok(result::err(vec![OutputContent::Text {
                        text: format!("invalid arguments: {e}"),
                    }]))
                });
            }
        };
        let resolved_path =
            match super::resolve_tool_path(&self.workspace_root, &args.path, self.path_policy) {
                Ok(p) => p,
                Err(e) => {
                    return Box::pin(async move { Ok(super::fs_error_result(e)) });
                }
            };
        let workspace_relation = resolved_path.workspace_relation;
        let file_path = resolved_path.path;
        let workspace_root = self.workspace_root.clone();
        let path_for_display = args.path.clone();
        Box::pin(async move {
            // Directories are rejected before any byte read so the NotAFile cause
            // is reported instead of a binary/encoding error from reading a dir.
            if file_path.is_dir() {
                return Ok(super::fs_error_result(FsToolError::NotAFile {
                    path: file_path.clone(),
                }));
            }
            let bytes = match tokio::fs::read(&file_path).await {
                Ok(b) => b,
                Err(e) => match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        return Ok(super::fs_error_result(FsToolError::NotFound {
                            user_path: path_for_display.clone(),
                            resolved_path: Some(file_path.clone()),
                        }));
                    }
                    std::io::ErrorKind::PermissionDenied => {
                        return Ok(super::fs_error_result(FsToolError::PermissionDenied {
                            path: file_path.clone(),
                        }));
                    }
                    _ => {
                        return Ok(result::err(vec![OutputContent::Text {
                            text: format!("failed to read {}: {e}", file_path.display()),
                        }]));
                    }
                },
            };

            // NUL bytes are the binary-content heuristic (Phase 11 design wording
            // "detects NUL-byte binary files"). Checked before UTF-8 so a file
            // that is both NUL-bearing and invalid UTF-8 reports as binary, the
            // more accurate diagnosis.
            if bytes.contains(&0u8) {
                return Ok(super::fs_error_result(FsToolError::BinaryFile {
                    path: file_path.clone(),
                }));
            }

            let content = match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(e) => {
                    // File-content encoding failure is reported with the shared
                    // tool_unsupported_encoding code but a content-appropriate
                    // message: the FsToolError::UnsupportedEncoding variant is
                    // directory/entry-shaped and reused by ls/find, so the
                    // single-file case builds the diagnostic directly. The agent
                    // loop lifts this into Phase 7 traces (task 11.8).
                    let byte_offset = e.utf8_error().valid_up_to();
                    let message = format!("'{}' is not valid UTF-8", file_path.display());
                    let mut unsupported = result::err(vec![OutputContent::Text {
                        text: message.clone(),
                    }]);
                    unsupported.diagnostics.push(ToolDiagnostic {
                        code: code::CODE_TOOL_UNSUPPORTED_ENCODING.to_string(),
                        message,
                        context: json!({
                            "path": file_path.display().to_string(),
                            "byte_offset": byte_offset,
                        }),
                    });
                    return Ok(unsupported);
                }
            };

            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();
            // offset is 1-based; values below 1 floor to 1 so the reported offset
            // always matches the effective start line.
            let offset_1 = args.offset.unwrap_or(1).max(1);
            // The clamp keeps the slice index in range and makes the `available`
            // subtraction below safe (offset_idx <= total_lines).
            let offset_idx = offset_1.saturating_sub(1).min(total_lines);
            let take_n = args.limit.unwrap_or(DEFAULT_READ_LINES);
            let available = total_lines - offset_idx;
            let returned = take_n.min(available);
            let selected: Vec<&str> = lines[offset_idx..].iter().take(returned).copied().collect();
            let omitted = available - returned;
            let truncated = omitted > 0;

            let mut body = selected.join("\n");
            if truncated {
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(&format!("... {omitted} lines omitted"));
            } else if total_lines > 0 && offset_1 > total_lines {
                // The window started past the end of the file; the read itself
                // succeeded but no lines apply. Surface the mismatch rather than
                // returning an empty body, without marking the result an error.
                body = format!(
                    "offset {offset_1} is past end of file (line_count {total_lines}); no lines returned"
                );
            }

            let mut details = result::path_metadata(
                &workspace_root,
                &path_for_display,
                &file_path,
                workspace_relation,
            );
            details["line_count"] = json!(total_lines);
            details["offset"] = json!(offset_1);
            details["limit"] = json!(take_n);
            details["truncated"] = json!(truncated);
            details["omitted"] = json!(omitted);

            let text = format!("{}\n{}", file_path.display(), body);
            let mut res = result::ok(vec![OutputContent::Text { text }], details);
            res.truncated = truncated;
            Ok(res)
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Parallel
    }
}
