use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult, result};
use opi_ai::message::{OutputContent, ToolDef};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BashArgs {
    /// Command to execute.
    pub command: String,
    /// Timeout in seconds (optional, defaults to 30).
    pub timeout_secs: Option<u64>,
}

pub struct BashTool {
    workspace_root: PathBuf,
    schema: serde_json::Value,
}

impl BashTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        let schema = schemars::schema_for!(BashArgs);
        Self {
            workspace_root,
            schema: serde_json::to_value(&schema).unwrap_or_default(),
        }
    }
}

impl Tool for BashTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "bash".into(),
            description: "Execute a shell command with timeout and streamed output.".into(),
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
        let args: BashArgs = match serde_json::from_value(arguments) {
            Ok(a) => a,
            Err(e) => {
                return Box::pin(async move {
                    Ok(result::err(vec![OutputContent::Text {
                        text: format!("invalid arguments: {e}"),
                    }]))
                });
            }
        };
        let timeout = Duration::from_secs(args.timeout_secs.unwrap_or(30));
        let command = args.command;
        let cwd = self.workspace_root.clone();
        let workspace_root = self.workspace_root.clone();
        Box::pin(async move {
            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let flag = if cfg!(windows) { "/C" } else { "-c" };
            let mut cmd = tokio::process::Command::new(shell);
            cmd.arg(flag).arg(&command).current_dir(&cwd);
            let mut child = match cmd
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    return Ok(result::err(vec![OutputContent::Text {
                        text: format!("failed to spawn command: {e}"),
                    }]));
                }
            };

            let timeout_future = tokio::time::sleep(timeout);
            let cancel_future = signal.cancelled();

            tokio::pin!(timeout_future);
            tokio::pin!(cancel_future);

            // Each execution branch yields (content, details, is_error); the result is
            // assembled once afterwards so success / nonzero / timeout / cancellation
            // all share one stable operation-metadata key set. Timeout reports
            // `timed_out=true, cancelled=false`; cancellation reports the inverse.
            let (content, details, is_error): (Vec<OutputContent>, serde_json::Value, bool) = tokio::select! {
                status = child.wait() => match status {
                    Ok(s) => {
                        let exit_code = s.code();
                        let stdout = child.stdout.take();
                        let stderr = child.stderr.take();
                        let output_fut = async {
                            let out = match stdout {
                                Some(mut s) => {
                                    let mut buf = Vec::new();
                                    use tokio::io::AsyncReadExt;
                                    let _ = s.read_to_end(&mut buf).await;
                                    String::from_utf8_lossy(&buf).into_owned()
                                }
                                None => String::new(),
                            };
                            let err = match stderr {
                                Some(mut s) => {
                                    let mut buf = Vec::new();
                                    use tokio::io::AsyncReadExt;
                                    let _ = s.read_to_end(&mut buf).await;
                                    String::from_utf8_lossy(&buf).into_owned()
                                }
                                None => String::new(),
                            };
                            (out, err)
                        };
                        let (stdout, stderr) = output_fut.await;
                        let mut output = stdout;
                        if !stderr.is_empty() {
                            if !output.is_empty() {
                                output.push('\n');
                            }
                            output.push_str(&stderr);
                        }
                        let details = result::bash_operation_metadata(
                            &workspace_root,
                            &command,
                            &cwd,
                            shell,
                            exit_code,
                            false,
                            false,
                            false,
                            None,
                        );
                        (
                            vec![OutputContent::Text { text: output }],
                            details,
                            exit_code != Some(0),
                        )
                    }
                    Err(e) => {
                        return Ok(result::err(vec![OutputContent::Text {
                            text: format!("failed to wait for process: {e}"),
                        }]));
                    }
                },
                _ = &mut timeout_future => {
                    let _ = child.kill().await;
                    let details = result::bash_operation_metadata(
                        &workspace_root,
                        &command,
                        &cwd,
                        shell,
                        None,
                        true,
                        false,
                        false,
                        None,
                    );
                    (
                        vec![OutputContent::Text {
                            text: "command timed out".to_string(),
                        }],
                        details,
                        true,
                    )
                }
                _ = &mut cancel_future => {
                    let _ = child.kill().await;
                    let details = result::bash_operation_metadata(
                        &workspace_root,
                        &command,
                        &cwd,
                        shell,
                        None,
                        false,
                        true,
                        false,
                        None,
                    );
                    (
                        vec![OutputContent::Text {
                            text: "command cancelled".to_string(),
                        }],
                        details,
                        true,
                    )
                }
            };

            let mut tool_result = result::ok(content, details);
            tool_result.is_error = is_error;
            Ok(tool_result)
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sequential
    }
}
