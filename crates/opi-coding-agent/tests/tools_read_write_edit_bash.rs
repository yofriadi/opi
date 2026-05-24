//! Behavioral tests for read, write, edit, bash tools (task 1.9).
//!
//! DoD: "temp-dir tests cover success, failure, timeout/cancellation,
//!       cwd/env reporting, and minimal confirmation policy"

use std::time::Duration;

use opi_agent::tool::{ExecutionMode, Tool, ToolResult};
use opi_coding_agent::tool::{BashTool, EditTool, ReadTool, WriteTool};
use serde_json::json;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tool_result_text(result: &ToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| match c {
            opi_ai::message::OutputContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

// ---------------------------------------------------------------------------
// ReadTool tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn read_tool_reads_file_content() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("hello.txt");
    std::fs::write(&file_path, "Hello, world!").unwrap();

    let tool = ReadTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c1",
            json!({ "path": "hello.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        !result.is_error,
        "unexpected error: {}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(
        text.contains("Hello, world!"),
        "should contain file content"
    );
}

#[tokio::test]
async fn read_tool_reads_with_line_range() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("lines.txt");
    std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5").unwrap();

    let tool = ReadTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c2",
            json!({ "path": "lines.txt", "offset": 2, "limit": 2 }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(text.contains("line2"), "should contain line2");
    assert!(text.contains("line3"), "should contain line3");
    assert!(!text.contains("line1"), "should not contain line1");
    assert!(!text.contains("line4"), "should not contain line4");
}

#[tokio::test]
async fn read_tool_file_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ReadTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c3",
            json!({ "path": "nonexistent.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should be error for missing file");
}

#[tokio::test]
async fn read_tool_is_parallel() {
    let tool = ReadTool::new(std::path::PathBuf::from("."));
    assert_eq!(tool.execution_mode(), ExecutionMode::Parallel);
}

#[tokio::test]
async fn read_tool_reports_workspace_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("inside.txt");
    std::fs::write(&file_path, "data").unwrap();

    let tool = ReadTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c4",
            json!({ "path": "inside.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    let text = tool_result_text(&result);
    assert!(
        text.contains("inside.txt"),
        "result should reference the file path"
    );
}

// ---------------------------------------------------------------------------
// WriteTool tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn write_tool_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let tool = WriteTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c5",
            json!({ "path": "new.txt", "content": "Hello!" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        !result.is_error,
        "unexpected error: {}",
        tool_result_text(&result)
    );
    let written = std::fs::read_to_string(dir.path().join("new.txt")).unwrap();
    assert_eq!(written, "Hello!");
}

#[tokio::test]
async fn write_tool_replaces_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("existing.txt");
    std::fs::write(&file_path, "old content").unwrap();

    let tool = WriteTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c6",
            json!({ "path": "existing.txt", "content": "new content" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let written = std::fs::read_to_string(file_path).unwrap();
    assert_eq!(written, "new content");
}

#[tokio::test]
async fn write_tool_is_sequential() {
    let tool = WriteTool::new(std::path::PathBuf::from("."));
    assert_eq!(tool.execution_mode(), ExecutionMode::Sequential);
}

#[tokio::test]
async fn write_tool_safety_context_in_details() {
    let dir = tempfile::tempdir().unwrap();
    let tool = WriteTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c7",
            json!({ "path": "safe.txt", "content": "data" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    // Safety boundary: details should include workspace root and path
    let details = result.details.expect("should have details");
    assert!(
        details.get("workspace_root").is_some(),
        "details should include workspace_root"
    );
    assert!(details.get("path").is_some(), "details should include path");
}

// ---------------------------------------------------------------------------
// EditTool tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn edit_tool_exact_string_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("edit.txt");
    std::fs::write(&file_path, "Hello, world!").unwrap();

    let tool = EditTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c8",
            json!({
                "path": "edit.txt",
                "old_string": "world",
                "new_string": "Rust"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        !result.is_error,
        "unexpected error: {}",
        tool_result_text(&result)
    );
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Hello, Rust!");
}

#[tokio::test]
async fn edit_tool_old_string_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("edit2.txt");
    std::fs::write(&file_path, "Hello").unwrap();

    let tool = EditTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c9",
            json!({
                "path": "edit2.txt",
                "old_string": "not present",
                "new_string": "replacement"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should be error when old_string not found");
}

#[tokio::test]
async fn edit_tool_is_sequential() {
    let tool = EditTool::new(std::path::PathBuf::from("."));
    assert_eq!(tool.execution_mode(), ExecutionMode::Sequential);
}

#[tokio::test]
async fn edit_tool_safety_context_in_details() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("safe_edit.txt");
    std::fs::write(&file_path, "content").unwrap();

    let tool = EditTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c10",
            json!({
                "path": "safe_edit.txt",
                "old_string": "content",
                "new_string": "changed"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    let details = result.details.expect("should have details");
    assert!(details.get("workspace_root").is_some());
    assert!(details.get("path").is_some());
}

// ---------------------------------------------------------------------------
// BashTool tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bash_tool_runs_command() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let cmd = "echo hello";
    let result = tool
        .execute(
            "c11",
            json!({ "command": cmd }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        !result.is_error,
        "unexpected error: {}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(text.contains("hello"), "output should contain 'hello'");
}

#[tokio::test]
async fn bash_tool_nonzero_exit_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let cmd = if cfg!(windows) {
        "cmd /C exit 1"
    } else {
        "exit 1"
    };
    let result = tool
        .execute(
            "c12",
            json!({ "command": cmd }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "non-zero exit should be error");
}

// ---------------------------------------------------------------------------
// Path boundary enforcement
// ---------------------------------------------------------------------------

#[tokio::test]
async fn write_tool_rejects_path_outside_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let tool = WriteTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "escape-1",
            json!({ "path": "../outside.txt", "content": "escaped" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should reject path outside workspace");
    let text = tool_result_text(&result);
    assert!(
        text.contains("outside the workspace"),
        "error should mention workspace boundary, got: {text}"
    );
}

#[tokio::test]
async fn edit_tool_rejects_path_outside_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let tool = EditTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "escape-2",
            json!({
                "path": "../escape.txt",
                "old_string": "x",
                "new_string": "y"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should reject path outside workspace");
    let text = tool_result_text(&result);
    assert!(
        text.contains("outside the workspace"),
        "error should mention workspace boundary, got: {text}"
    );
}

#[tokio::test]
async fn read_tool_rejects_path_outside_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ReadTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "escape-3",
            json!({ "path": "../etc/passwd" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should reject path outside workspace");
    let text = tool_result_text(&result);
    assert!(
        text.contains("outside the workspace"),
        "error should mention workspace boundary, got: {text}"
    );
}

#[tokio::test]
async fn bash_tool_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let cmd = if cfg!(windows) {
        "ping -n 30 127.0.0.1 >nul"
    } else {
        "sleep 30"
    };
    let result = tool
        .execute(
            "c13",
            json!({ "command": cmd, "timeout_secs": 1 }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "timeout should produce error");
    let text = tool_result_text(&result);
    assert!(
        text.to_lowercase().contains("timeout") || text.to_lowercase().contains("timed out"),
        "error should mention timeout: {text}"
    );
}

#[tokio::test]
async fn bash_tool_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let token = CancellationToken::new();
    let cmd = if cfg!(windows) {
        "ping -n 30 127.0.0.1 >nul"
    } else {
        "sleep 30"
    };

    let handle = {
        let token = token.clone();
        tokio::spawn(async move {
            tool.execute(
                "c14",
                json!({ "command": cmd, "timeout_secs": 60 }),
                token,
                None,
            )
            .await
        })
    };

    // Give the process time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    token.cancel();

    let result = handle.await.unwrap().unwrap();
    assert!(result.is_error, "cancelled process should be error");
}

#[tokio::test]
async fn bash_tool_cwd_reporting() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let cmd = if cfg!(windows) { "cd" } else { "pwd" };
    let result = tool
        .execute(
            "c15",
            json!({ "command": cmd }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    // Check that details reports the correct cwd
    let details = result.details.expect("should have details");
    let cwd = details.get("cwd").unwrap().as_str().unwrap();
    assert!(
        cwd.contains(dir.path().file_name().unwrap().to_str().unwrap()),
        "details.cwd should contain temp dir name, got: {cwd}"
    );
}

#[tokio::test]
async fn bash_tool_is_sequential() {
    let tool = BashTool::new(std::path::PathBuf::from("."));
    assert_eq!(tool.execution_mode(), ExecutionMode::Sequential);
}

#[tokio::test]
async fn bash_tool_safety_context_in_details() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c16",
            json!({ "command": "echo test" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    let details = result.details.expect("should have details");
    assert!(
        details.get("command").is_some(),
        "details should include command"
    );
    assert!(details.get("cwd").is_some(), "details should include cwd");
}

#[tokio::test]
async fn bash_tool_env_inheritance_reporting() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let cmd = if cfg!(windows) { "set" } else { "env" };
    let result = tool
        .execute(
            "c17",
            json!({ "command": cmd }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    // Should be able to see inherited environment
    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(!text.is_empty(), "should have env output");
}

// ---------------------------------------------------------------------------
// Symlink / junction escape regression tests
// ---------------------------------------------------------------------------

/// Helper: create a directory junction (Windows) or symlink (Unix) from
/// `link_path` pointing to `target`. Returns true if the link was created.
fn create_dir_link(link_path: &std::path::Path, target: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        // Use junction on Windows — no special privileges needed.
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                &link_path.to_string_lossy(),
                &target.to_string_lossy(),
            ])
            .output();
        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link_path).is_ok()
    }
}

#[tokio::test]
async fn write_tool_rejects_symlink_escape_via_new_subpath() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();

    // workspace/link -> outside
    let link = workspace.path().join("link");
    if !create_dir_link(&link, outside.path()) {
        eprintln!("skipping: could not create directory link");
        return;
    }

    let tool = WriteTool::new(workspace.path().to_path_buf());
    let result = tool
        .execute(
            "symlink-escape-1",
            json!({ "path": "link/new/file.txt", "content": "escaped" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        result.is_error,
        "should reject symlink escape, got: {:?}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(
        text.contains("outside the workspace"),
        "error should mention workspace boundary, got: {text}"
    );

    // Verify nothing was written outside
    assert!(
        !outside.path().join("new/file.txt").exists(),
        "file must not exist outside workspace"
    );
}

#[tokio::test]
async fn read_tool_rejects_symlink_escape_via_new_subpath() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();

    let link = workspace.path().join("link");
    if !create_dir_link(&link, outside.path()) {
        eprintln!("skipping: could not create directory link");
        return;
    }

    let tool = ReadTool::new(workspace.path().to_path_buf());
    let result = tool
        .execute(
            "symlink-escape-2",
            json!({ "path": "link/new/file.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        result.is_error,
        "should reject symlink escape, got: {:?}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(
        text.contains("outside the workspace"),
        "error should mention workspace boundary, got: {text}"
    );
}

#[tokio::test]
async fn edit_tool_rejects_symlink_escape_via_new_subpath() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();

    let link = workspace.path().join("link");
    if !create_dir_link(&link, outside.path()) {
        eprintln!("skipping: could not create directory link");
        return;
    }

    let tool = EditTool::new(workspace.path().to_path_buf());
    let result = tool
        .execute(
            "symlink-escape-3",
            json!({
                "path": "link/new/file.txt",
                "old_string": "x",
                "new_string": "y"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        result.is_error,
        "should reject symlink escape, got: {:?}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(
        text.contains("outside the workspace"),
        "error should mention workspace boundary, got: {text}"
    );
}

// ---------------------------------------------------------------------------
// Tool definition tests
// ---------------------------------------------------------------------------

#[test]
fn read_tool_has_valid_definition() {
    let tool = ReadTool::new(std::path::PathBuf::from("."));
    let def = tool.definition();
    assert_eq!(def.name, "read");
    assert!(!def.description.is_empty());
    assert!(def.input_schema.is_object());
}

#[test]
fn write_tool_has_valid_definition() {
    let tool = WriteTool::new(std::path::PathBuf::from("."));
    let def = tool.definition();
    assert_eq!(def.name, "write");
    assert!(!def.description.is_empty());
}

#[test]
fn edit_tool_has_valid_definition() {
    let tool = EditTool::new(std::path::PathBuf::from("."));
    let def = tool.definition();
    assert_eq!(def.name, "edit");
    assert!(!def.description.is_empty());
}

#[test]
fn bash_tool_has_valid_definition() {
    let tool = BashTool::new(std::path::PathBuf::from("."));
    let def = tool.definition();
    assert_eq!(def.name, "bash");
    assert!(!def.description.is_empty());
}
