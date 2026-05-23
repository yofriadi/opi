//! Integration tests for edit-tool diff capture (DiffView runtime wiring).
//!
//! Verifies that the edit tool includes before/after file content in its
//! result details so the TUI can render a diff view.

use opi_agent::tool::Tool;
use opi_coding_agent::tool::EditTool;
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn tool_result_text(result: &opi_agent::tool::ToolResult) -> String {
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

#[tokio::test]
async fn edit_tool_captures_before_content() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("src.rs");
    std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    let tool = EditTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "diff1",
            json!({
                "path": "src.rs",
                "old_string": "hello",
                "new_string": "world"
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

    let details = result.details.expect("should have details");
    let before = details
        .get("before")
        .expect("should have before")
        .as_str()
        .unwrap();
    assert!(
        before.contains("hello"),
        "before should contain original content"
    );
    assert!(
        !before.contains("world"),
        "before should not contain replacement content"
    );
}

#[tokio::test]
async fn edit_tool_captures_after_content() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("lib.rs");
    std::fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

    let tool = EditTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "diff2",
            json!({
                "path": "lib.rs",
                "old_string": "a + b",
                "new_string": "a + b + 0"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);

    let details = result.details.expect("should have details");
    let after = details
        .get("after")
        .expect("should have after")
        .as_str()
        .unwrap();
    assert!(
        after.contains("a + b + 0"),
        "after should contain new content"
    );
    assert_eq!(
        after.matches("a + b + 0").count(),
        1,
        "replacement should appear exactly once"
    );
}

#[tokio::test]
async fn edit_tool_before_after_preserve_full_file() {
    let dir = tempfile::tempdir().unwrap();
    let original = "line1\nline2\nline3\nline4\nline5\n";
    let file_path = dir.path().join("multi.txt");
    std::fs::write(&file_path, original).unwrap();

    let tool = EditTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "diff3",
            json!({
                "path": "multi.txt",
                "old_string": "line3",
                "new_string": "LINE_THREE"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);

    let details = result.details.expect("should have details");
    let before = details.get("before").unwrap().as_str().unwrap();
    let after = details.get("after").unwrap().as_str().unwrap();

    // before is the full original file
    assert_eq!(
        before, original,
        "before should be the complete original file"
    );
    // after preserves context around the change
    assert!(
        after.contains("line1")
            && after.contains("line2")
            && after.contains("line4")
            && after.contains("line5"),
        "after should preserve unchanged lines"
    );
    assert!(
        after.contains("LINE_THREE"),
        "after should contain the replacement"
    );
    assert!(
        !after.contains("line3"),
        "after should not contain the old text"
    );
}

#[tokio::test]
async fn edit_tool_no_diff_on_error() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("exists.txt");
    std::fs::write(&file_path, "content").unwrap();

    let tool = EditTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "diff4",
            json!({
                "path": "exists.txt",
                "old_string": "not_present",
                "new_string": "whatever"
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(result.is_error, "should be error when old_string not found");
    assert!(
        result.details.is_none(),
        "error results should not have details"
    );
}
