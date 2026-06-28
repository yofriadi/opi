//! Behavioral tests for the find tool (task 3.9).
//!
//! Validates gitignore-aware file discovery with pi-style argument naming,
//! path scoping, hidden-file handling, and error cases.

use std::fs;

use opi_agent::tool::{ExecutionMode, Tool, ToolResult};
use opi_coding_agent::tool::FindTool;
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

fn create_gitignore(dir: &std::path::Path, content: &str) {
    fs::write(dir.join(".gitignore"), content).unwrap();
}

// --- Basic pattern matching ---

#[tokio::test]
async fn find_tool_matches_files_by_glob_pattern() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("foo.rs"), "fn main() {}").unwrap();
    fs::write(dir.path().join("bar.rs"), "fn bar() {}").unwrap();
    fs::write(dir.path().join("baz.txt"), "hello").unwrap();

    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c1",
            json!({ "pattern": "**/*.rs" }),
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
    assert!(text.contains("foo.rs"), "should find foo.rs");
    assert!(text.contains("bar.rs"), "should find bar.rs");
    assert!(!text.contains("baz.txt"), "should not find baz.txt");
}

#[tokio::test]
async fn find_tool_no_match_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("file.txt"), "data").unwrap();

    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c2",
            json!({ "pattern": "*.nonexistent" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(
        !text.contains("file.txt"),
        "should not match unrelated files"
    );
}

// --- Path scoping ---

#[tokio::test]
async fn find_tool_scopes_to_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::create_dir_all(dir.path().join("tests")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(dir.path().join("tests/main.rs"), "#[test] fn t() {}").unwrap();

    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c3",
            json!({ "pattern": "**/*.rs", "path": "src" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(
        text.contains("src/main.rs") || text.contains("main.rs"),
        "should find file in src subdir"
    );
    assert!(
        !text.contains("tests"),
        "should not find files outside scoped path"
    );
}

#[tokio::test]
async fn find_tool_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("inside.txt"), "data").unwrap();

    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c4",
            json!({ "pattern": "*", "path": "../" }),
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

// --- Gitignore awareness ---

#[tokio::test]
async fn find_tool_ignores_gitignored_dirs() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::create_dir_all(dir.path().join("target")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(dir.path().join("target/build.rs"), "fn build() {}").unwrap();
    create_gitignore(dir.path(), "target/\n");

    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c5",
            json!({ "pattern": "**/*.rs" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(text.contains("main.rs"), "should find src/main.rs");
    assert!(
        !text.contains("target"),
        "should not find files in gitignored target dir"
    );
}

// --- Hidden files ---

#[tokio::test]
async fn find_tool_finds_hidden_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".hidden.cfg"), "secret").unwrap();
    fs::write(dir.path().join("visible.txt"), "data").unwrap();

    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c6",
            json!({ "pattern": "*" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(
        text.contains(".hidden.cfg"),
        "should find hidden files when not gitignored"
    );
}

// --- Invalid pattern ---

#[tokio::test]
async fn find_tool_invalid_glob_pattern_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c7",
            json!({ "pattern": "[invalid" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        result.is_error,
        "invalid glob pattern should be error: {}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(
        text.to_lowercase().contains("pattern") || text.to_lowercase().contains("glob"),
        "error should mention pattern issue: {text}"
    );
}

// --- Invalid arguments ---

#[tokio::test]
async fn find_tool_missing_pattern_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute("c8", json!({}), CancellationToken::new(), None)
        .await
        .unwrap();

    assert!(
        result.is_error,
        "missing pattern should be error: {}",
        tool_result_text(&result)
    );
}

// --- Tool definition ---

#[test]
fn find_tool_has_valid_definition() {
    let tool = FindTool::new(std::path::PathBuf::from("."));
    let def = tool.definition();
    assert_eq!(def.name, "find");
    assert!(!def.description.is_empty());
    assert!(def.input_schema.is_object());
}

#[test]
fn find_tool_is_parallel() {
    let tool = FindTool::new(std::path::PathBuf::from("."));
    assert_eq!(tool.execution_mode(), ExecutionMode::Parallel);
}

// --- Details metadata ---

#[tokio::test]
async fn find_tool_includes_details_metadata() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rs"), "").unwrap();
    fs::write(dir.path().join("b.rs"), "").unwrap();

    let tool = FindTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c9",
            json!({ "pattern": "*.rs" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let details = result.details.as_ref().expect("should have details");
    assert!(
        details.get("match_count").is_some(),
        "details should include match_count"
    );
    assert_eq!(
        details.get("workspace_relation").and_then(|v| v.as_str()),
        Some("inside"),
        "find details should include workspace_relation"
    );
}
