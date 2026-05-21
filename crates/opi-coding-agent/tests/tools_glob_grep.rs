//! Behavioral tests for glob and grep tools (task 1.10).
//!
//! DoD: "tests cover ignored dirs and regex errors"

use opi_agent::tool::{ExecutionMode, Tool, ToolResult};
use opi_coding_agent::tool::{GlobTool, GrepTool};
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

fn create_gitignore(dir: &std::path::Path, content: &str) {
    std::fs::write(dir.join(".gitignore"), content).unwrap();
}

// ---------------------------------------------------------------------------
// GlobTool tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn glob_tool_finds_files_by_pattern() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("foo.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.path().join("bar.rs"), "fn bar() {}").unwrap();
    std::fs::write(dir.path().join("baz.txt"), "hello").unwrap();

    let tool = GlobTool::new(dir.path().to_path_buf());
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
async fn glob_tool_ignores_gitignored_dirs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join("target")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.path().join("target/build.rs"), "fn build() {}").unwrap();
    create_gitignore(dir.path(), "target/\n");

    let tool = GlobTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c2",
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

#[tokio::test]
async fn glob_tool_empty_pattern_matches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file.txt"), "data").unwrap();

    let tool = GlobTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c3",
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

#[tokio::test]
async fn glob_tool_is_parallel() {
    let tool = GlobTool::new(std::path::PathBuf::from("."));
    assert_eq!(tool.execution_mode(), ExecutionMode::Parallel);
}

// ---------------------------------------------------------------------------
// GrepTool tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn grep_tool_finds_matching_lines() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("code.rs"),
        "fn hello() {}\nfn world() {}\nfn goodbye() {}\n",
    )
    .unwrap();

    let tool = GrepTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c4",
            json!({ "pattern": "hello" }),
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
    assert!(text.contains("hello"), "should match 'hello'");
    assert!(
        !text.contains("goodbye"),
        "should not match unrelated lines"
    );
}

#[tokio::test]
async fn grep_tool_regex_pattern() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.txt"), "foo123\nbar456\nbaz\n").unwrap();

    let tool = GrepTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c5",
            json!({ "pattern": "[a-z]+\\d+" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(text.contains("foo123"), "should match foo123");
    assert!(text.contains("bar456"), "should match bar456");
    assert!(!text.contains("baz"), "should not match baz (no digits)");
}

#[tokio::test]
async fn grep_tool_invalid_regex_is_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = GrepTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c6",
            json!({ "pattern": "[invalid" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(
        result.is_error,
        "invalid regex should be error: {}",
        tool_result_text(&result)
    );
    let text = tool_result_text(&result);
    assert!(
        text.to_lowercase().contains("regex") || text.to_lowercase().contains("pattern"),
        "error should mention regex issue: {text}"
    );
}

#[tokio::test]
async fn grep_tool_ignores_gitignored_dirs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "secret_token_here\n").unwrap();
    std::fs::write(
        dir.path().join("node_modules/pkg.js"),
        "secret_token_in_dep\n",
    )
    .unwrap();
    create_gitignore(dir.path(), "node_modules/\n");

    let tool = GrepTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c7",
            json!({ "pattern": "secret_token" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let text = tool_result_text(&result);
    assert!(text.contains("main.rs"), "should find match in src/main.rs");
    assert!(
        !text.contains("node_modules"),
        "should not search in gitignored node_modules"
    );
}

#[tokio::test]
async fn grep_tool_no_matches_is_not_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file.txt"), "hello world\n").unwrap();

    let tool = GrepTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "c8",
            json!({ "pattern": "nonexistent_pattern" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

    // No matches is not an error — just empty results
    assert!(!result.is_error);
}

#[tokio::test]
async fn grep_tool_is_parallel() {
    let tool = GrepTool::new(std::path::PathBuf::from("."));
    assert_eq!(tool.execution_mode(), ExecutionMode::Parallel);
}

// ---------------------------------------------------------------------------
// Tool definition tests
// ---------------------------------------------------------------------------

#[test]
fn glob_tool_has_valid_definition() {
    let tool = GlobTool::new(std::path::PathBuf::from("."));
    let def = tool.definition();
    assert_eq!(def.name, "glob");
    assert!(!def.description.is_empty());
    assert!(def.input_schema.is_object());
}

#[test]
fn grep_tool_has_valid_definition() {
    let tool = GrepTool::new(std::path::PathBuf::from("."));
    let def = tool.definition();
    assert_eq!(def.name, "grep");
    assert!(!def.description.is_empty());
    assert!(def.input_schema.is_object());
}
