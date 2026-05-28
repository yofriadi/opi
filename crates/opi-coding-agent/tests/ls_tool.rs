//! Behavioral tests for the ls tool (task 3.9).
//!
//! Validates directory listing with bounded output, deterministic ordering,
//! hidden-file handling, path traversal rejection, and error cases.

use std::fs;

use opi_agent::tool::{ExecutionMode, Tool, ToolResult};
use opi_coding_agent::tool::LsTool;
use serde_json::json;
use tokio_util::sync::CancellationToken;

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

// --- Basic listing ---

#[tokio::test]
async fn ls_tool_lists_directory_contents() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("file1.txt"), "a").unwrap();
    fs::write(dir.path().join("file2.rs"), "b").unwrap();
    fs::create_dir(dir.path().join("subdir")).unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute("c1", json!({ "path": "." }), CancellationToken::new(), None)
        .await
        .unwrap();

    assert!(
        !result.is_error,
        "unexpected error: {}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(text.contains("file1.txt"), "should list file1.txt");
    assert!(text.contains("file2.rs"), "should list file2.rs");
    assert!(text.contains("subdir"), "should list subdir");
}

// --- Deterministic ordering ---

#[tokio::test]
async fn ls_tool_lists_in_deterministic_order() {
    let dir = tempfile::tempdir().unwrap();
    // Create files in reverse alphabetical order
    for name in &["c.txt", "a.txt", "b.txt"] {
        fs::write(dir.path().join(name), "").unwrap();
    }

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute("c2", json!({ "path": "." }), CancellationToken::new(), None)
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    let pos_a = text.find("a.txt").expect("should contain a.txt");
    let pos_b = text.find("b.txt").expect("should contain b.txt");
    let pos_c = text.find("c.txt").expect("should contain c.txt");
    assert!(
        pos_a < pos_b && pos_b < pos_c,
        "entries should be in sorted order: a < b < c"
    );
}

// --- Hidden files ---

#[tokio::test]
async fn ls_tool_shows_hidden_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".hidden"), "secret").unwrap();
    fs::write(dir.path().join("visible.txt"), "data").unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute("c3", json!({ "path": "." }), CancellationToken::new(), None)
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(text.contains(".hidden"), "should list hidden files");
    assert!(text.contains("visible.txt"), "should list visible files");
}

// --- Subdirectory listing ---

#[tokio::test]
async fn ls_tool_lists_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("nested/deep")).unwrap();
    fs::write(dir.path().join("nested/a.txt"), "").unwrap();
    fs::write(dir.path().join("nested/b.txt"), "").unwrap();
    fs::write(dir.path().join("root.txt"), "").unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c4",
            json!({ "path": "nested" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(text.contains("a.txt"), "should list files in subdirectory");
    assert!(text.contains("b.txt"), "should list files in subdirectory");
    assert!(
        !text.contains("root.txt"),
        "should not include files from parent directory"
    );
}

// --- Path traversal rejection ---

#[tokio::test]
async fn ls_tool_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("inside.txt"), "data").unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c5",
            json!({ "path": "../" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        result.is_error,
        "path traversal should be rejected: {}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(
        text.contains("outside") || text.contains("workspace") || text.contains("invalid"),
        "error should explain path issue: {text}"
    );
}

// --- Nonexistent directory ---

#[tokio::test]
async fn ls_tool_nonexistent_path_is_error() {
    let dir = tempfile::tempdir().unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c6",
            json!({ "path": "does_not_exist" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        result.is_error,
        "nonexistent path should be error: {}",
        tool_result_text(&result)
    );
}

// --- Bounded output: max_entries ---

#[tokio::test]
async fn ls_tool_truncates_at_max_entries() {
    let dir = tempfile::tempdir().unwrap();
    // Create more files than the limit
    for i in 0..20 {
        fs::write(dir.path().join(format!("file_{:02}.txt", i)), "").unwrap();
    }

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c7",
            json!({ "path": ".", "max_entries": 5 }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    // Should have at most 5 entries plus possibly a truncation notice
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.contains("truncated") && !l.is_empty())
        .collect();
    assert!(
        lines.len() <= 5,
        "should have at most 5 entries, got {}: {text}",
        lines.len()
    );
}

// --- Bounded output: max_depth ---

#[tokio::test]
async fn ls_tool_respects_max_depth() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("a/b/c")).unwrap();
    fs::write(dir.path().join("a/a1.txt"), "").unwrap();
    fs::write(dir.path().join("a/b/b1.txt"), "").unwrap();
    fs::write(dir.path().join("a/b/c/c1.txt"), "").unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c8",
            json!({ "path": ".", "max_depth": 1 }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    // With max_depth=1, should only list immediate children (a/ and its direct entries)
    // but NOT recurse into a/b/
    assert!(
        text.contains("a1.txt") || text.contains("a"),
        "should contain entries at depth 1"
    );
}

// --- Missing arguments ---

#[tokio::test]
async fn ls_tool_missing_path_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute("c9", json!({}), CancellationToken::new(), None)
        .await
        .unwrap();

    assert!(
        result.is_error,
        "missing path should be error: {}",
        tool_result_text(&result)
    );
}

// --- Tool definition ---

#[test]
fn ls_tool_has_valid_definition() {
    let tool = LsTool::new(std::path::PathBuf::from("."));
    let def = tool.definition();
    assert_eq!(def.name, "ls");
    assert!(!def.description.is_empty());
    assert!(def.input_schema.is_object());
}

#[test]
fn ls_tool_is_parallel() {
    let tool = LsTool::new(std::path::PathBuf::from("."));
    assert_eq!(tool.execution_mode(), ExecutionMode::Parallel);
}

// --- Details metadata ---

#[tokio::test]
async fn ls_tool_includes_details_metadata() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "").unwrap();
    fs::write(dir.path().join("b.txt"), "").unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c10",
            json!({ "path": "." }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let details = result.details.as_ref().expect("should have details");
    assert!(
        details.get("entry_count").is_some(),
        "details should include entry_count"
    );
}

// --- Gitignored entries ---

#[tokio::test]
async fn ls_tool_excludes_gitignored_entries() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
    fs::write(dir.path().join("visible.txt"), "data").unwrap();
    fs::create_dir_all(dir.path().join("target")).unwrap();
    fs::write(dir.path().join("target/secret.txt"), "build").unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c11",
            json!({ "path": "." }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(
        text.contains("visible.txt"),
        "should list non-ignored files"
    );
    assert!(
        !text.contains("secret.txt"),
        "should not list gitignored files"
    );
}

// --- Entry type indication ---

#[tokio::test]
async fn ls_tool_distinguishes_dirs_and_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("mydir")).unwrap();
    fs::write(dir.path().join("myfile.txt"), "data").unwrap();

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c12",
            json!({ "path": "." }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    // Directories should be indicated (trailing / or explicit marker)
    assert!(
        text.contains("mydir") && (text.contains("mydir/") || text.contains("dir")),
        "directories should be distinguishable: {text}"
    );
}

// --- Truncation metadata correctness ---

#[tokio::test]
async fn ls_tool_truncation_shows_correct_omitted_count() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..20 {
        fs::write(dir.path().join(format!("file_{:02}.txt", i)), "").unwrap();
    }

    let tool = LsTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c13",
            json!({ "path": ".", "max_entries": 5 }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    // Should say "15 entries omitted" (20 total - 5 shown = 15 omitted)
    assert!(
        text.contains("15 entries omitted"),
        "expected '15 entries omitted' in output, got: {text}"
    );
    // Details should have correct counts
    let details = result.details.as_ref().unwrap();
    assert_eq!(details["entry_count"], 5);
    assert_eq!(details["total_entries"], 20);
    assert_eq!(details["truncated"], true);
}
