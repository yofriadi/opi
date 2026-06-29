use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult, result};
use opi_ai::message::{OutputContent, ToolDef};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

/// Maximum number of bytes of merged stdout+stderr returned inline in the tool
/// result content and mirrored into the stable operation-metadata preview.
///
/// Output beyond this cap is truncated: `ToolResult.truncated` and
/// `details.truncated` are set, and the COMPLETE merged output is spilled to a
/// temp file whose path is reported in `details.full_output`, so no output is
/// lost. Applies to the success and nonzero-exit branches; timeout/cancellation
/// report no captured output (consistent with the prior contract). The value is
/// mirrored into `details.truncated`; tests import this constant rather than
/// hard-coding a byte count.
pub const MAX_BASH_OUTPUT_BYTES: usize = 64 * 1024; // 64 KiB

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

            // Take the pipes so two drain futures can read them concurrently with
            // `child.wait()`. Draining both pipes while the child writes avoids the
            // stdout-then-stderr pipe deadlock (a command writing >pipe-buffer to
            // stderr while stdout is still being read would otherwise block forever).
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            let timeout_future = tokio::time::sleep(timeout);
            let cancel_future = signal.cancelled();
            tokio::pin!(timeout_future);
            tokio::pin!(cancel_future);

            // Bounded captures. Each keeps the first `cap` bytes in memory and
            // spills the COMPLETE stream to a temp file once it overflows, so memory
            // stays bounded while no output is lost. See [`StreamCapture`].
            let mut out_cap = StreamCapture::new(MAX_BASH_OUTPUT_BYTES);
            let mut err_cap = StreamCapture::new(MAX_BASH_OUTPUT_BYTES);

            // Drain stdout and stderr concurrently with the wait/timeout/cancel race.
            // The drains run in EVERY branch; on timeout/cancel the child is killed,
            // the pipes hit EOF, the drains finish, and their captures are discarded
            // (with spill files cleaned up) so no temp artifact leaks.
            let drain_out = async {
                if let Some(mut s) = stdout {
                    let mut buf = [0u8; 8192];
                    loop {
                        match s.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if out_cap.append(&buf[..n]).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            };
            let drain_err = async {
                if let Some(mut s) = stderr {
                    let mut buf = [0u8; 8192];
                    loop {
                        match s.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if err_cap.append(&buf[..n]).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            };
            let control = async {
                tokio::select! {
                    biased;
                    _ = &mut cancel_future => {
                        let _ = child.kill().await;
                        Control::Cancelled
                    }
                    _ = &mut timeout_future => {
                        let _ = child.kill().await;
                        Control::TimedOut
                    }
                    status = child.wait() => match status {
                        Ok(s) => Control::Done(s),
                        Err(_) => Control::WaitFailed,
                    },
                }
            };

            // Three-way join: both drains and the control race are polled concurrently.
            let (_, _, ctrl) = tokio::join!(drain_out, drain_err, control);

            match ctrl {
                Control::Cancelled => {
                    cleanup_spill(&mut out_cap);
                    cleanup_spill(&mut err_cap);
                    let details = with_env_policy(result::bash_operation_metadata(
                        &workspace_root,
                        &command,
                        &cwd,
                        shell,
                        None,
                        false,
                        true,
                        false,
                        None,
                    ));
                    Ok(bash_result(
                        vec![OutputContent::Text {
                            text: "command cancelled".to_string(),
                        }],
                        details,
                        true,
                        false,
                    ))
                }
                Control::TimedOut => {
                    cleanup_spill(&mut out_cap);
                    cleanup_spill(&mut err_cap);
                    let details = with_env_policy(result::bash_operation_metadata(
                        &workspace_root,
                        &command,
                        &cwd,
                        shell,
                        None,
                        true,
                        false,
                        false,
                        None,
                    ));
                    Ok(bash_result(
                        vec![OutputContent::Text {
                            text: "command timed out".to_string(),
                        }],
                        details,
                        true,
                        false,
                    ))
                }
                Control::WaitFailed => {
                    cleanup_spill(&mut out_cap);
                    cleanup_spill(&mut err_cap);
                    Ok(result::err(vec![OutputContent::Text {
                        text: "failed to wait for process".to_string(),
                    }]))
                }
                Control::Done(status) => {
                    let exit_code = status.code();
                    let total = out_cap.total + err_cap.total;
                    let truncated = total > MAX_BASH_OUTPUT_BYTES as u64;

                    // Merged preview = stdout preview ++ stderr preview (deterministic
                    // stdout-then-stderr order; each preview is <= cap bytes).
                    let mut merged: Vec<u8> =
                        Vec::with_capacity(out_cap.preview.len() + err_cap.preview.len());
                    merged.extend_from_slice(&out_cap.preview);
                    merged.extend_from_slice(&err_cap.preview);
                    let cap = MAX_BASH_OUTPUT_BYTES.min(merged.len());
                    let text = String::from_utf8_lossy(&merged[..cap]).into_owned();

                    // On truncation, spill the COMPLETE merged output (stdout-then-
                    // stderr) to one temp file and report its path. The per-stream
                    // spill files are then removed; this merged file is the keeper.
                    let full_output = if truncated {
                        write_merged_full_output(&out_cap, &err_cap)
                    } else {
                        None
                    };
                    cleanup_spill(&mut out_cap);
                    cleanup_spill(&mut err_cap);

                    let details = with_env_policy(result::bash_operation_metadata(
                        &workspace_root,
                        &command,
                        &cwd,
                        shell,
                        exit_code,
                        false,
                        false,
                        truncated,
                        full_output.as_deref(),
                    ));
                    let is_error = exit_code != Some(0);
                    Ok(bash_result(
                        vec![OutputContent::Text { text }],
                        details,
                        is_error,
                        truncated,
                    ))
                }
            }
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sequential
    }
}

/// Which control branch won the wait/timeout/cancel race.
enum Control {
    Done(std::process::ExitStatus),
    TimedOut,
    Cancelled,
    WaitFailed,
}

/// Assemble a bash tool result from the shared success builder, then override
/// `is_error` (nonzero exit) and `truncated` (output cap). Mirrors the Phase
/// 11.1 bash pattern: nonzero-exit keeps the success-shape result with details
/// present (the stable operation-metadata contract); only `is_error` flips.
fn bash_result(
    content: Vec<OutputContent>,
    details: Value,
    is_error: bool,
    truncated: bool,
) -> ToolResult {
    let mut tool_result = result::ok(content, details);
    tool_result.is_error = is_error;
    tool_result.truncated = truncated;
    tool_result
}

/// Inject the environment-handling policy token into bash operation metadata.
///
/// `details.env = { "inheritance": "inherited", "values_included": false }`.
/// `values_included: false` is the machine-checkable invariant that no
/// inherited environment values are dumped into details/diagnostics (the secret
/// no-leak test asserts it). This key is bash-local and is intentionally NOT
/// promoted into the shared `bash_operation_metadata` builder in opi-agent: the
/// env policy is bash-specific and the existing `tool_result_details_use_*
/// guard` only forbids hand-written `details: Some(..)`, not in-place
/// mutation of the returned Value.
fn with_env_policy(mut details: Value) -> Value {
    details["env"] = json!({ "inheritance": "inherited", "values_included": false });
    details
}

/// Bounded capture of one output stream (stdout or stderr).
///
/// Holds the first `cap` bytes in memory as `preview` and, once the stream
/// exceeds `cap`, spills the COMPLETE stream to a temp file (`spill` /
/// `spill_path`). Memory is bounded to ~`cap` bytes regardless of total output
/// size; the spill file is byte-for-byte complete so it can serve as the
/// `full_output` reference.
///
/// The append logic enforces a single-cursor invariant: every input byte routes
/// to exactly one sink. While `preview.len() < cap`, incoming bytes fill the
/// preview; the remainder of the chunk that crosses the cap boundary, plus all
/// subsequent bytes, go to the spill file (which is seeded with the frozen
/// `cap`-byte preview so the file is the complete stream, not just the tail).
/// This avoids both byte drops and double-writes across single-huge-chunk,
/// mid-chunk-overflow, exact-boundary, and straddle cases.
struct StreamCapture {
    preview: Vec<u8>,
    spill: Option<std::fs::File>,
    spill_path: Option<PathBuf>,
    total: u64,
    cap: usize,
}

impl StreamCapture {
    fn new(cap: usize) -> Self {
        Self {
            preview: Vec::new(),
            spill: None,
            spill_path: None,
            total: 0,
            cap,
        }
    }

    /// Append one read chunk. Single-cursor invariant; see struct docs.
    fn append(&mut self, chunk: &[u8]) -> std::io::Result<()> {
        let n = chunk.len();
        if self.preview.len() < self.cap {
            // Fill the preview up to `cap`.
            let room = self.cap - self.preview.len();
            let take = n.min(room);
            self.preview.extend_from_slice(&chunk[..take]);
            let rest = &chunk[take..];
            if !rest.is_empty() {
                // This chunk crossed the cap. ensure_spill seeds the file with
                // the frozen preview prefix on first creation; append the rest.
                self.ensure_spill()?;
                self.spill
                    .as_mut()
                    .expect("spill ensured")
                    .write_all(rest)?;
            }
        } else {
            // Preview already frozen at `cap`: every byte goes straight to spill.
            // ensure_spill seeds the frozen preview the first time it opens the
            // file, so the spill is the COMPLETE stream even when the cap was
            // reached by an earlier exact-fit chunk with no crossing remainder.
            self.ensure_spill()?;
            self.spill
                .as_mut()
                .expect("spill ensured")
                .write_all(chunk)?;
        }
        self.total += n as u64;
        Ok(())
    }

    /// Lazily create the spill file the first time output overflows. The file is
    /// seeded with the frozen `cap`-byte preview so it is the COMPLETE stream
    /// (preview prefix + every subsequent byte), regardless of which append
    /// branch first overflows (in-chunk crossing, or a later chunk after an
    /// exact-fit freeze). `preview.len()` is exactly `cap` whenever this runs.
    fn ensure_spill(&mut self) -> std::io::Result<()> {
        if self.spill.is_none() {
            let path = bash_output_temp_path();
            let mut file = std::fs::File::create(&path)?;
            file.write_all(&self.preview)?;
            self.spill = Some(file);
            self.spill_path = Some(path);
        }
        Ok(())
    }

    /// The complete stream bytes: the spill file contents if the stream
    /// overflowed, otherwise the in-memory preview (which holds the whole
    /// stream because `total <= cap`).
    fn complete_bytes(&self) -> std::io::Result<Vec<u8>> {
        match &self.spill_path {
            Some(path) => std::fs::read(path),
            None => Ok(self.preview.clone()),
        }
    }
}

/// Drop the spill file handle (if any) and best-effort remove the temp file.
fn cleanup_spill(cap: &mut StreamCapture) {
    cap.spill.take();
    if let Some(path) = cap.spill_path.take() {
        let _ = std::fs::remove_file(path);
    }
}

/// Write the COMPLETE merged output (stdout-then-stderr) to one temp file and
/// return its path as a string (the builder's `full_output` is `Option<&str>`).
/// Returns `None` only if the merged file cannot be created or written
/// (truncation is still signaled via the flags; the reference is simply absent).
/// Per-stream spill files remain owned by the caller and are cleaned up
/// separately.
fn write_merged_full_output(out: &StreamCapture, err: &StreamCapture) -> Option<String> {
    let out_bytes = out.complete_bytes().ok()?;
    let err_bytes = err.complete_bytes().ok()?;
    let path = bash_output_temp_path();
    let mut file = std::fs::File::create(&path).ok()?;
    file.write_all(&out_bytes).ok()?;
    file.write_all(&err_bytes).ok()?;
    let _ = file.sync_all();
    drop(file);
    Some(path.to_string_lossy().into_owned())
}

/// A unique OS-temp path for a bash full-output spill file.
///
/// Lives in the OS temp dir (outside the workspace, so it never appears in
/// `git status` and is reaped by the OS). The absolute path may encode the OS
/// username on some platforms; this discloses nothing beyond `workspace_root`,
/// which is already present in the same details object on every platform.
fn bash_output_temp_path() -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("opi-bash-output-{pid}-{nanos}.log"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_capture_holds_small_stream_in_preview() {
        let mut c = StreamCapture::new(8);
        c.append(b"abc").unwrap();
        c.append(b"de").unwrap();
        assert_eq!(c.total, 5);
        assert_eq!(c.preview, b"abcde");
        assert!(c.spill.is_none());
        assert_eq!(c.complete_bytes().unwrap(), b"abcde");
    }

    #[test]
    fn stream_capture_spills_complete_stream_on_overflow() {
        let mut c = StreamCapture::new(4);
        // Single huge chunk (6 bytes, cap 4): preview freezes at 4, spill holds all 6.
        c.append(b"abcdef").unwrap();
        assert_eq!(c.total, 6);
        assert_eq!(c.preview, b"abcd");
        assert!(c.spill.is_some());
        assert_eq!(c.complete_bytes().unwrap(), b"abcdef");
    }

    #[test]
    fn stream_capture_mid_chunk_overflow_is_byte_complete() {
        let mut c = StreamCapture::new(4);
        c.append(b"ab").unwrap(); // preview=2, no spill
        c.append(b"cdefgh").unwrap(); // fills preview to 4 (cd), spills complete (abcdefgh)
        assert_eq!(c.total, 8);
        assert_eq!(c.preview, b"abcd");
        assert_eq!(c.complete_bytes().unwrap(), b"abcdefgh");
    }

    #[test]
    fn stream_capture_exact_boundary_does_not_spill() {
        let mut c = StreamCapture::new(4);
        c.append(b"abcd").unwrap(); // exactly cap, not overflow
        assert_eq!(c.total, 4);
        assert_eq!(c.preview, b"abcd");
        assert!(c.spill.is_none());
    }

    #[test]
    fn stream_capture_cap_plus_one_overflows() {
        let mut c = StreamCapture::new(4);
        c.append(b"abcde").unwrap(); // cap+1 -> overflow
        assert_eq!(c.total, 5);
        assert_eq!(c.complete_bytes().unwrap(), b"abcde");
    }

    /// Regression: preview frozen at EXACTLY cap by an earlier fitting chunk (no
    /// crossing remainder), then a LATER chunk overflows. The spill must be
    /// seeded with the frozen preview so complete_bytes() is the full stream.
    /// (Before the ensure_spill-seeds-preview fix, this returned b"e".)
    #[test]
    fn stream_capture_exact_fit_then_overflow_is_byte_complete() {
        let mut c = StreamCapture::new(4);
        c.append(b"abcd").unwrap(); // freezes preview at exactly cap, no spill
        c.append(b"e").unwrap(); // ELSE branch -> first overflow
        assert_eq!(c.total, 5);
        assert_eq!(c.complete_bytes().unwrap(), b"abcde");
    }

    /// Regression (many small chunks): preview reaches cap across several chunks
    /// with no crossing remainder, then a later chunk overflows.
    #[test]
    fn stream_capture_many_small_exact_fit_then_overflow_is_byte_complete() {
        let mut c = StreamCapture::new(4);
        c.append(b"ab").unwrap();
        c.append(b"cd").unwrap(); // freezes preview at exactly cap, no spill
        c.append(b"efg").unwrap(); // ELSE branch -> first overflow
        assert_eq!(c.total, 7);
        assert_eq!(c.complete_bytes().unwrap(), b"abcdefg");
    }
}
