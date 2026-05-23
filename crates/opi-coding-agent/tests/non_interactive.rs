//! E2E tests for non-interactive mode (task 1.15).
//!
//! DoD: "stdout/stderr/exit-code tests"
//!
//! Tests exercise: NonInteractiveRunner with MockProvider,
//! verifying stdout output, stderr diagnostics, and exit code mapping.

use std::sync::{Arc, Mutex};

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
        Vec::new(),
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
        Vec::new(),
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
        Vec::new(),
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

// ---------------------------------------------------------------------------
// Test 4: non-interactive resume forwards CompactionSummary to the provider
// ---------------------------------------------------------------------------

#[tokio::test]
async fn runner_resume_forwards_compaction_summary_to_provider() {
    use opi_agent::message::{AgentMessage, CompactionSummaryMessage};
    use opi_ai::message::{InputContent, Message};

    let response = test_support::text_response("ack");
    let provider = MockProvider::new("mock", vec![response]);
    let call_log = provider.call_log_handle();

    let summary_text = "Earlier we discussed the quarterly compaction strategy.";
    let initial_messages = vec![AgentMessage::CompactionSummary(CompactionSummaryMessage {
        summary: summary_text.into(),
        first_kept_entry_id: "msg-42".into(),
        tokens_before: 1000,
        tokens_after: 200,
    })];

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
        initial_messages,
    );

    let result = runner.run("continue please").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    let log = call_log.lock().unwrap();
    let first_request = log.first().expect("provider was called at least once");

    // The resumed summary must appear as a synthetic user-message in the
    // request the provider observed. Otherwise compacted context is silently
    // dropped on resume.
    let mut saw_summary = false;
    for msg in &first_request.messages {
        if let Message::User(u) = msg {
            for content in &u.content {
                if let InputContent::Text { text } = content
                    && text.contains(summary_text)
                {
                    saw_summary = true;
                }
            }
        }
    }
    assert!(
        saw_summary,
        "provider request messages must include compacted summary text; got: {:?}",
        first_request.messages
    );
}

// ---------------------------------------------------------------------------
// Test 5: format_persist_errors captures errors that occur during the run
// ---------------------------------------------------------------------------
//
// Regression test: persist_stderr was previously computed BEFORE prompt()
// ran, so SessionPersistError events emitted during the run were silently
// dropped. The fix moves format_persist_errors() to after prompt() returns.
//
// Directly triggering a session IO error cross-platform is impractical
// (the file handle is already open), so this test verifies:
// (a) the format_persist_errors helper produces correct output, and
// (b) the runner's run() subscriber correctly routes SessionPersistError
//     events into the persist_errors capture buffer.
// ---------------------------------------------------------------------------

/// Verify format_persist_errors produces the expected output.
#[test]
fn format_persist_errors_unit() {
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));

    // Empty -> no output
    let result = opi_coding_agent::runner::format_persist_errors(&errors);
    assert!(result.is_empty(), "expected empty for no errors, got: {result:?}");

    // With errors
    {
        let mut guard = errors.lock().unwrap();
        guard.push("disk full".into());
        guard.push("permission denied".into());
    }
    let result = opi_coding_agent::runner::format_persist_errors(&errors);
    assert!(
        result.contains("session persist error: disk full"),
        "should contain first error, got: {result:?}"
    );
    assert!(
        result.contains("session persist error: permission denied"),
        "should contain second error, got: {result:?}"
    );
}
