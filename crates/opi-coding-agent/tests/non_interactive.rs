//! E2E tests for non-interactive mode (task 1.15).
//!
//! DoD: "stdout/stderr/exit-code tests"
//!
//! Tests exercise: NonInteractiveRunner with MockProvider,
//! verifying stdout output, stderr diagnostics, and exit code mapping.

use opi_ai::test_support::{self, MockProvider};
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::runner::{ExitCode, NonInteractiveRunner};

// ---------------------------------------------------------------------------
// Test 1: text prompt produces stdout output with exit code 0
// ---------------------------------------------------------------------------

#[tokio::test]
async fn runner_text_prompt_stdout_exit0() {
    let response = test_support::text_response("Hello from runner!");
    let provider = MockProvider::new("mock", vec![response]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run("Hi there").await;

    assert_eq!(result.exit_code, ExitCode::Success as i32, "should exit 0");
    assert!(
        result.stdout.contains("Hello from runner!"),
        "stdout should contain assistant text, got: {:?}",
        result.stdout
    );
}

// ---------------------------------------------------------------------------
// Test 2: tool call (read-only) succeeds in non-interactive mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn runner_readonly_tool_succeeds() {
    let first = test_support::tool_call_response(
        "tc-1",
        "read",
        r#"{"path":"Cargo.toml","offset":1,"limit":5}"#,
    );
    let second = test_support::text_response("The file contains workspace config.");

    let provider = MockProvider::new("mock", vec![first, second]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run("Read the Cargo.toml").await;

    assert_eq!(result.exit_code, ExitCode::Success as i32, "should exit 0");
    assert!(
        result.stdout.contains("workspace config"),
        "stdout should contain tool result text, got: {:?}",
        result.stdout
    );
}

// ---------------------------------------------------------------------------
// Test 3: provider error response produces stderr and exit code 4
// ---------------------------------------------------------------------------

#[tokio::test]
async fn runner_provider_error_stderr_exit4() {
    let response = test_support::error_response("connection refused");
    let provider = MockProvider::new("mock", vec![response]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run("Do something").await;

    assert_eq!(
        result.exit_code,
        ExitCode::ProviderFailure as i32,
        "should exit 4 on provider error"
    );
    assert!(
        result.stderr.contains("connection refused"),
        "stderr should contain error message, got: {:?}",
        result.stderr
    );
}
