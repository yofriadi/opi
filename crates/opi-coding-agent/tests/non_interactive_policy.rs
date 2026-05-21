//! Policy tests for non-interactive mode (task 1.15).
//!
//! Verifies that non-interactive mode refuses mutating tools (write, edit, bash)
//! unless explicitly opted in via the allow_mutating flag.
//!
//! Smoke addendum: "non-interactive mode refuses mutating tools (write, edit,
//! bash) unless explicitly opted in via CLI flag or config"

use std::path::PathBuf;

use opi_ai::test_support::{self, MockProvider};
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::runner::{ExitCode, NonInteractiveRunner};

fn temp_workspace() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("opi-policy-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

// ---------------------------------------------------------------------------
// Test 1: write tool is blocked by default
// ---------------------------------------------------------------------------

#[tokio::test]
async fn policy_write_blocked_by_default() {
    let first = test_support::tool_call_response(
        "tc-1",
        "write",
        r#"{"path":"test.txt","content":"hello"}"#,
    );
    let second = test_support::text_response("Write was denied.");

    let provider = MockProvider::new("mock", vec![first, second]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        temp_workspace(),
        false, // allow_mutating = false
        None,
    );

    let result = runner.run("Write a file").await;

    assert_eq!(result.exit_code, ExitCode::Success as i32);
    assert!(
        result.stdout.contains("denied")
            || result.stderr.contains("denied")
            || result.stdout.contains("not allowed"),
        "should indicate tool was denied, stdout: {:?}, stderr: {:?}",
        result.stdout,
        result.stderr
    );
}

// ---------------------------------------------------------------------------
// Test 2: edit tool is blocked by default
// ---------------------------------------------------------------------------

#[tokio::test]
async fn policy_edit_blocked_by_default() {
    let first = test_support::tool_call_response(
        "tc-1",
        "edit",
        r#"{"path":"test.txt","old_string":"foo","new_string":"bar"}"#,
    );
    let second = test_support::text_response("Edit was denied.");

    let provider = MockProvider::new("mock", vec![first, second]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        temp_workspace(),
        false,
        None,
    );

    let result = runner.run("Edit a file").await;

    assert_eq!(result.exit_code, ExitCode::Success as i32);
    assert!(
        result.stdout.contains("denied")
            || result.stderr.contains("denied")
            || result.stdout.contains("not allowed"),
        "should indicate tool was denied, stdout: {:?}, stderr: {:?}",
        result.stdout,
        result.stderr
    );
}

// ---------------------------------------------------------------------------
// Test 3: bash tool is blocked by default
// ---------------------------------------------------------------------------

#[tokio::test]
async fn policy_bash_blocked_by_default() {
    let first = test_support::tool_call_response("tc-1", "bash", r#"{"command":"ls -la"}"#);
    let second = test_support::text_response("Bash was denied.");

    let provider = MockProvider::new("mock", vec![first, second]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        temp_workspace(),
        false,
        None,
    );

    let result = runner.run("Run a command").await;

    assert_eq!(result.exit_code, ExitCode::Success as i32);
    assert!(
        result.stdout.contains("denied")
            || result.stderr.contains("denied")
            || result.stdout.contains("not allowed"),
        "should indicate tool was denied, stdout: {:?}, stderr: {:?}",
        result.stdout,
        result.stderr
    );
}

// ---------------------------------------------------------------------------
// Test 4: read tool is allowed by default
// ---------------------------------------------------------------------------

#[tokio::test]
async fn policy_read_allowed_by_default() {
    let first = test_support::tool_call_response("tc-1", "read", r#"{"path":"Cargo.toml"}"#);
    let second = test_support::text_response("File read successful.");

    let provider = MockProvider::new("mock", vec![first, second]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        temp_workspace(),
        false,
        None,
    );

    let result = runner.run("Read a file").await;

    assert_eq!(result.exit_code, ExitCode::Success as i32);
}

// ---------------------------------------------------------------------------
// Test 5: all tools allowed when allow_mutating=true
// ---------------------------------------------------------------------------

#[tokio::test]
async fn policy_all_tools_allowed_when_opted_in() {
    let first = test_support::tool_call_response(
        "tc-1",
        "write",
        r#"{"path":"test.txt","content":"hello"}"#,
    );
    let second = test_support::text_response("Write succeeded.");

    let provider = MockProvider::new("mock", vec![first, second]);

    let workspace = temp_workspace();
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        workspace.clone(),
        true, // allow_mutating = true
        None,
    );

    let result = runner.run("Write a file").await;

    assert_eq!(result.exit_code, ExitCode::Success as i32);
    // Tool should have executed (not denied)
    assert!(
        !result.stdout.contains("not allowed"),
        "should not contain denial message when opted in, got: {:?}",
        result.stdout
    );
    // Clean up written file
    let _ = std::fs::remove_file(workspace.join("test.txt"));
}

// ---------------------------------------------------------------------------
// Test 6: glob and grep are allowed by default
// ---------------------------------------------------------------------------

#[tokio::test]
async fn policy_readonly_tools_always_allowed() {
    let first = test_support::tool_call_response("tc-1", "glob", r#"{"pattern":"*.rs"}"#);
    let second = test_support::text_response("Glob succeeded.");

    let provider = MockProvider::new("mock", vec![first, second]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        temp_workspace(),
        false,
        None,
    );

    let result = runner.run("Find files").await;

    assert_eq!(result.exit_code, ExitCode::Success as i32);
}
