//! Behavioral tests for read, write, edit, bash tools (task 1.9).
//!
//! DoD: "temp-dir tests cover success, failure, timeout/cancellation,
//!       cwd/env reporting, and minimal confirmation policy"

use std::time::Duration;

use opi_agent::diagnostic::code;
use opi_agent::tool::{ExecutionMode, Tool, ToolResult};
use opi_coding_agent::tool::{BashTool, EditTool, PathPolicy, ReadTool, WriteTool};
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

fn details_string<'a>(details: &'a serde_json::Value, key: &str) -> &'a str {
    details
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_else(|| panic!("details should include string key '{key}'"))
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

    let details = result.details.expect("should have details");
    assert_eq!(
        details
            .get("workspace_relation")
            .and_then(|value| value.as_str()),
        Some("inside")
    );
    assert!(
        !details
            .as_object()
            .map(|o| o.contains_key("inside_workspace"))
            .unwrap_or(false),
        "inside_workspace key must be superseded by workspace_relation"
    );
    assert!(details_string(&details, "resolved_path").ends_with("inside.txt"));
}

#[tokio::test]
async fn read_tool_allow_outside_policy_reads_absolute_outside_path() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("outside.txt");
    std::fs::write(&outside_file, "outside data").unwrap();

    let tool = ReadTool::new_with_policy(
        workspace.path().to_path_buf(),
        PathPolicy::AllowOutsideWorkspace,
    );
    let result = tool
        .execute(
            "outside-read-allow",
            json!({ "path": outside_file }),
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
    assert!(tool_result_text(&result).contains("outside data"));
    let details = result.details.expect("should have details");
    assert_eq!(
        details
            .get("workspace_relation")
            .and_then(|value| value.as_str()),
        Some("outside")
    );
    let resolved = details_string(&details, "resolved_path");
    assert!(
        !resolved.contains(r"\\?\"),
        "resolved_path must not leak the Windows verbatim prefix: {resolved}"
    );
    // The stripped display path must still resolve to the same file.
    assert_eq!(
        std::fs::canonicalize(resolved).unwrap(),
        std::fs::canonicalize(&outside_file).unwrap()
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
    assert_eq!(
        details
            .get("workspace_relation")
            .and_then(|value| value.as_str()),
        Some("inside")
    );
    assert!(
        !details
            .as_object()
            .map(|o| o.contains_key("inside_workspace"))
            .unwrap_or(false),
        "inside_workspace key must be superseded by workspace_relation"
    );
    assert!(details_string(&details, "resolved_path").ends_with("safe.txt"));
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
    assert_eq!(
        details
            .get("workspace_relation")
            .and_then(|value| value.as_str()),
        Some("inside")
    );
    assert!(
        !details
            .as_object()
            .map(|o| o.contains_key("inside_workspace"))
            .unwrap_or(false),
        "inside_workspace key must be superseded by workspace_relation"
    );
    assert!(details_string(&details, "resolved_path").ends_with("safe_edit.txt"));
    assert!(details.get("before").is_some());
    assert!(details.get("after").is_some());
}

// ---------------------------------------------------------------------------
// Uniform tool-result contract (Phase 11.1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn uniform_tool_result_details_contract() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "hello world").unwrap();

    // read success: base contract + uniform path metadata
    let read = ReadTool::new(dir.path().to_path_buf())
        .execute(
            "u1",
            json!({ "path": "f.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(!read.is_error, "read: {}", tool_result_text(&read));
    assert!(!read.truncated);
    assert!(read.diagnostics.is_empty());
    let rd = read.details.expect("read details");
    for key in [
        "workspace_root",
        "path",
        "resolved_path",
        "workspace_relation",
    ] {
        assert!(rd.get(key).is_some(), "read details missing {key}");
    }
    assert_eq!(
        rd.get("workspace_relation").and_then(|v| v.as_str()),
        Some("inside")
    );

    // write success: base contract + uniform path metadata
    let write = WriteTool::new(dir.path().to_path_buf())
        .execute(
            "u2",
            json!({ "path": "out.txt", "content": "x" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(!write.is_error, "write: {}", tool_result_text(&write));
    assert!(!write.truncated);
    assert_eq!(
        write
            .details
            .as_ref()
            .and_then(|d| d.get("workspace_relation"))
            .and_then(|v| v.as_str()),
        Some("inside")
    );

    // edit success: base contract + uniform path metadata + before/after
    let edit = EditTool::new(dir.path().to_path_buf())
        .execute(
            "u3",
            json!({ "path": "f.txt", "old_string": "hello", "new_string": "HI" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(!edit.is_error, "edit: {}", tool_result_text(&edit));
    let ed = edit.details.expect("edit details");
    assert_eq!(
        ed.get("workspace_relation").and_then(|v| v.as_str()),
        Some("inside")
    );
    assert!(ed.get("before").is_some());
    assert!(ed.get("after").is_some());

    // representative failure: base contract, no path metadata, not truncated
    let missing = ReadTool::new(dir.path().to_path_buf())
        .execute(
            "u4",
            json!({ "path": "nope.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(missing.is_error);
    assert!(!missing.truncated);
    assert!(
        missing
            .diagnostics
            .iter()
            .any(|d| d.code == code::CODE_TOOL_PATH_NOT_FOUND),
        "missing-file failure should carry tool_path_not_found: {:?}",
        missing.diagnostics
    );
    assert!(missing.details.is_none(), "failure details must stay None");
}

// ---------------------------------------------------------------------------
// Filesystem error taxonomy (Phase 11.2)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn filesystem_error_taxonomy_path_failures() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file.txt"), "x").unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();
    let read = ReadTool::new(dir.path().to_path_buf());

    // NotFound: missing file carries tool_path_not_found.
    let missing = read
        .execute(
            "tax-1",
            json!({ "path": "nope.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(missing.is_error);
    assert!(
        missing
            .diagnostics
            .iter()
            .any(|d| d.code == code::CODE_TOOL_PATH_NOT_FOUND),
        "missing file should carry tool_path_not_found: {:?}",
        missing.diagnostics
    );
    assert!(tool_result_text(&missing).contains("does not exist"));

    // NotAFile: reading a directory carries tool_not_a_file.
    let dir_read = read
        .execute(
            "tax-2",
            json!({ "path": "subdir" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(dir_read.is_error);
    assert!(
        dir_read
            .diagnostics
            .iter()
            .any(|d| d.code == code::CODE_TOOL_NOT_A_FILE),
        "reading a directory should carry tool_not_a_file: {:?}",
        dir_read.diagnostics
    );
    assert!(tool_result_text(&dir_read).contains("not a file"));

    // OutsideWorkspace: workspace-only read of an outside absolute path.
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("o.txt");
    std::fs::write(&outside_file, "x").unwrap();
    let abs = outside_file.to_string_lossy().to_string();
    let outside_read = read
        .execute(
            "tax-3",
            json!({ "path": abs }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(outside_read.is_error);
    assert!(
        outside_read
            .diagnostics
            .iter()
            .any(|d| d.code == code::CODE_TOOL_OUTSIDE_WORKSPACE),
        "outside-workspace read should carry tool_outside_workspace: {:?}",
        outside_read.diagnostics
    );
    assert!(tool_result_text(&outside_read).contains("outside the workspace"));
}

#[tokio::test]
async fn unicode_path_metadata_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let name = "café-日本語.txt";
    std::fs::write(dir.path().join(name), "x").unwrap();
    let read = ReadTool::new(dir.path().to_path_buf());
    let result = read
        .execute(
            "uni-1",
            json!({ "path": name }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(!result.is_error, "{}", tool_result_text(&result));
    let details = result.details.expect("read details");
    let resolved = details_string(&details, "resolved_path");
    assert!(
        resolved.contains("café") && resolved.contains("日本語"),
        "unicode filename must round-trip in resolved_path: {resolved}"
    );
    assert!(
        !resolved.contains('\u{FFFD}'),
        "no lossy U+FFFD replacement: {resolved}"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn windows_drive_prefix() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("f.txt"), "x").unwrap();
    // Feed the verbatim canonical form back to the tool and require the
    // user-facing resolved path to be clean (no \\?\ leak).
    let verbatim = std::fs::canonicalize(workspace.path().join("f.txt")).unwrap();
    let read = ReadTool::new_with_policy(
        workspace.path().to_path_buf(),
        PathPolicy::AllowOutsideWorkspace,
    );
    let result = read
        .execute(
            "win-1",
            json!({ "path": verbatim.to_string_lossy() }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(!result.is_error, "{}", tool_result_text(&result));
    let details = result.details.expect("details");
    let resolved = details_string(&details, "resolved_path");
    assert!(
        !resolved.contains(r"\\?\"),
        "verbatim prefix must not leak into resolved_path: {resolved}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn read_tool_permission_denied_is_classified() {
    extern "C" {
        fn getuid() -> u32;
    }
    if unsafe { getuid() } == 0 {
        eprintln!("skipping permission test under root");
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("locked.txt");
    std::fs::write(&file, "x").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let read = ReadTool::new(dir.path().to_path_buf());
    let result = read
        .execute(
            "pd-1",
            json!({ "path": "locked.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644));
    assert!(result.is_error);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == code::CODE_TOOL_PERMISSION_DENIED),
        "unreadable file should carry tool_permission_denied: {:?}",
        result.diagnostics
    );
    assert!(tool_result_text(&result).contains("permission denied"));
}

#[cfg(windows)]
#[tokio::test]
async fn windows_drive_prefix_forms() {
    let workspace = tempfile::tempdir().unwrap();
    let read = ReadTool::new(workspace.path().to_path_buf());
    // C:\foo (drive-absolute) is outside the workspace -> OutsideWorkspace, and
    // the message must not leak the verbatim prefix.
    let result = read
        .execute(
            "win-form-1",
            json!({ "path": "C:\\nonexistent-opi-11-2-probe.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == code::CODE_TOOL_OUTSIDE_WORKSPACE),
        "drive-absolute outside path should be denied: {:?}",
        result.diagnostics
    );
    assert!(!tool_result_text(&result).contains(r"\\?\"));
    // C:foo (drive-relative) is handled deterministically without leaking the
    // verbatim prefix (treated as relative under the workspace root).
    let rel = read
        .execute(
            "win-form-2",
            json!({ "path": "C:nonexistent.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    if let Some(p) = rel
        .details
        .as_ref()
        .and_then(|d| d.get("resolved_path"))
        .and_then(|v| v.as_str())
    {
        assert!(!p.contains(r"\\?\"));
    }
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
async fn write_tool_rejects_absolute_path_outside_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("outside-write.txt");

    let tool = WriteTool::new(workspace.path().to_path_buf());
    let result = tool
        .execute(
            "escape-absolute-write",
            json!({ "path": outside_file, "content": "escaped" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should reject path outside workspace");
    assert!(
        tool_result_text(&result).contains("outside the workspace"),
        "error should mention workspace boundary"
    );
    assert!(
        !outside_file.exists(),
        "write must not create files outside workspace"
    );
}

#[tokio::test]
async fn write_tool_normalizes_parent_segment_after_missing_component() {
    let workspace = tempfile::tempdir().unwrap();
    let tool = WriteTool::new(workspace.path().to_path_buf());
    let result = tool
        .execute(
            "normalize-missing-parent",
            json!({ "path": "missing/../target.txt", "content": "normalized" }),
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
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("target.txt")).unwrap(),
        "normalized"
    );
    assert!(
        !workspace.path().join("missing/target.txt").exists(),
        "path should normalize to workspace target, not missing/target"
    );
}

#[tokio::test]
async fn write_tool_rejects_parent_escape_after_missing_component() {
    let parent = tempfile::tempdir().unwrap();
    let workspace = parent.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let outside_file = parent.path().join("outside.txt");

    let tool = WriteTool::new(workspace);
    let result = tool
        .execute(
            "normalize-missing-parent-escape",
            json!({ "path": "missing/../../outside.txt", "content": "escaped" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should reject path outside workspace");
    assert!(
        tool_result_text(&result).contains("outside the workspace"),
        "error should mention workspace boundary"
    );
    assert!(
        !outside_file.exists(),
        "write must not create files outside workspace"
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
async fn edit_tool_rejects_absolute_path_outside_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("outside-edit.txt");
    std::fs::write(&outside_file, "before").unwrap();

    let tool = EditTool::new(workspace.path().to_path_buf());
    let result = tool
        .execute(
            "escape-absolute-edit",
            json!({
                "path": outside_file,
                "old_string": "before",
                "new_string": "after"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should reject path outside workspace");
    assert!(
        tool_result_text(&result).contains("outside the workspace"),
        "error should mention workspace boundary"
    );
    assert_eq!(std::fs::read_to_string(&outside_file).unwrap(), "before");
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
async fn read_tool_workspace_policy_rejects_absolute_outside_path() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("outside-read.txt");
    std::fs::write(&outside_file, "outside").unwrap();

    let tool = ReadTool::new(workspace.path().to_path_buf());
    let result = tool
        .execute(
            "escape-absolute-read",
            json!({ "path": outside_file }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should reject path outside workspace");
    assert!(
        tool_result_text(&result).contains("outside the workspace"),
        "error should mention workspace boundary"
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
async fn bash_cancel_timeout_details_contract() {
    let dir = tempfile::tempdir().unwrap();
    let long_cmd = if cfg!(windows) {
        "ping -n 30 127.0.0.1 >nul"
    } else {
        "sleep 30"
    };

    // Timeout branch: timed_out=true, cancelled=false, exit_code=null, shell present.
    let timed = BashTool::new(dir.path().to_path_buf())
        .execute(
            "b-timeout",
            json!({ "command": long_cmd, "timeout_secs": 1 }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(timed.is_error);
    let td = timed.details.expect("timeout details");
    assert_eq!(
        td.get("timed_out").and_then(|v| v.as_bool()),
        Some(true),
        "timeout: timed_out must be true"
    );
    assert_eq!(
        td.get("cancelled").and_then(|v| v.as_bool()),
        Some(false),
        "timeout: cancelled must be false"
    );
    assert_eq!(
        td.get("exit_code"),
        Some(&serde_json::Value::Null),
        "timeout: exit_code must be null"
    );
    assert!(td.get("shell").is_some(), "timeout: shell must be present");
    for key in [
        "workspace_root",
        "command",
        "cwd",
        "shell",
        "exit_code",
        "timed_out",
        "cancelled",
        "truncated",
    ] {
        assert!(
            td.get(key).is_some(),
            "timeout details missing stable key: {key}"
        );
    }

    // Cancellation branch: cancelled=true, timed_out=false, exit_code=null.
    let cancel_tool = BashTool::new(dir.path().to_path_buf());
    let token = CancellationToken::new();
    let handle = {
        let token = token.clone();
        tokio::spawn(async move {
            cancel_tool
                .execute(
                    "b-cancel",
                    json!({ "command": long_cmd, "timeout_secs": 60 }),
                    token,
                    None,
                )
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(150)).await;
    token.cancel();
    let cancelled = handle.await.unwrap().unwrap();
    assert!(cancelled.is_error);
    let cd = cancelled.details.expect("cancel details");
    assert_eq!(
        cd.get("cancelled").and_then(|v| v.as_bool()),
        Some(true),
        "cancel: cancelled must be true"
    );
    assert_eq!(
        cd.get("timed_out").and_then(|v| v.as_bool()),
        Some(false),
        "cancel: timed_out must be false"
    );
    assert_eq!(cd.get("exit_code"), Some(&serde_json::Value::Null));
    assert!(cd.get("shell").is_some(), "cancel: shell must be present");
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
// Phase 11.1 source-structure guard
// ---------------------------------------------------------------------------

fn phase11_workspace_root() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

/// Strip `//` line comments and `/* */` block comments (char-based, UTF-8 safe).
fn phase11_strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' {
            match chars.peek() {
                Some('/') => {
                    for cc in chars.by_ref() {
                        if cc == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    while let Some(cc) = chars.next() {
                        if cc == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

/// Built-in tools must not hand-write `details: Some(..)`; every ToolResult is
/// built through `opi_agent::tool::result`. Scans the coding-agent tool sources
/// plus `opi_agent::tool` (the validation-error constructor), excluding
/// `result.rs` itself (the builder). Includes a vacuous-allowlist so the guard
/// cannot pass against an empty builder module.
#[test]
fn tool_result_details_use_shared_builders_guard() {
    let root = phase11_workspace_root();
    let mut scanned: Vec<std::path::PathBuf> = Vec::new();
    let tool_dir = root.join("crates/opi-coding-agent/src/tool");
    for entry in std::fs::read_dir(&tool_dir).expect("tool dir") {
        let p = entry.unwrap().path();
        if p.extension().is_some_and(|e| e == "rs") {
            scanned.push(p);
        }
    }
    scanned.push(root.join("crates/opi-agent/src/tool.rs"));

    let mut offenders: Vec<String> = Vec::new();
    for path in &scanned {
        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let stripped = phase11_strip_comments(&src);
        if stripped.contains("details: Some(") || stripped.contains("details:Some(") {
            offenders.push(path.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "these tool sources hand-write `details: Some(..)` instead of routing through          opi_agent::tool::result builders: {offenders:?}"
    );

    // Vacuous-allowlist: the builder module must actually define the stable keys.
    let builder = std::fs::read_to_string(root.join("crates/opi-agent/src/tool/result.rs"))
        .expect("result.rs");
    for key in [
        "workspace_root",
        "resolved_path",
        "workspace_relation",
        "command",
        "shell",
        "exit_code",
        "timed_out",
        "cancelled",
        "truncated",
    ] {
        assert!(
            builder.contains(key),
            "result.rs builder missing expected details key: {key}"
        );
    }
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
