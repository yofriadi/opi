use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
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
        let timeout = Duration::from_secs(args.timeout_secs.unwrap_or(30));
        let command = args.command;
        let cwd = self.workspace_root.clone();
        let workspace_root = self.workspace_root.clone();
        Box::pin(async move {
            let (program, args) = if cfg!(windows) {
                ("cmd".to_string(), vec!["/C", &command])
            } else {
                ("sh".to_string(), vec!["-c", &command])
            };

            let mut cmd = tokio::process::Command::new(&program);
            cmd.args(&args).current_dir(&cwd);
            let child = cmd
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn();

            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    return Ok(ToolResult {
                        content: vec![OutputContent::Text {
                            text: format!("failed to spawn command: {e}"),
                        }],
                        details: None,
                        is_error: true,
                        terminate: false,
                    });
                }
            };

            let timeout_future = tokio::time::sleep(timeout);
            let cancel_future = signal.cancelled();

            tokio::pin!(timeout_future);
            tokio::pin!(cancel_future);

            let result = tokio::select! {
                status = child.wait() => {
                    match status {
                        Ok(s) => {
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
                            Ok((stdout, stderr, s.code()))
                        }
                        Err(e) => Err(format!("failed to wait for process: {e}")),
                    }
                }
                _ = &mut timeout_future => {
                    let _ = child.kill().await;
                    Err("command timed out".into())
                }
                _ = &mut cancel_future => {
                    let _ = child.kill().await;
                    Err("command cancelled".into())
                }
            };

            match result {
                Ok((stdout, stderr, exit_code)) => {
                    let mut output = stdout;
                    if !stderr.is_empty() {
                        if !output.is_empty() {
                            output.push('\n');
                        }
                        output.push_str(&stderr);
                    }

                    let is_error = exit_code != Some(0);
                    let details = serde_json::json!({
                        "command": command,
                        "cwd": cwd.to_string_lossy(),
                        "exit_code": exit_code,
                        "workspace_root": workspace_root.to_string_lossy(),
                    });

                    Ok(ToolResult {
                        content: vec![OutputContent::Text { text: output }],
                        details: Some(details),
                        is_error,
                        terminate: false,
                    })
                }
                Err(msg) => Ok(ToolResult {
                    content: vec![OutputContent::Text { text: msg }],
                    details: Some(serde_json::json!({
                        "command": command,
                        "cwd": cwd.to_string_lossy(),
                        "timed_out": true,
                    })),
                    is_error: true,
                    terminate: false,
                }),
            }
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sequential
    }
}
