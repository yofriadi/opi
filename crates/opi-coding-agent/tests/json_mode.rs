//! JSON mode contract tests (task 2.14).
//!
//! DoD: "one AgentSessionEvent JSON object per line to stdout,
//!       schema version field, contract tests for framing"

use opi_agent::session_event::AgentSessionEvent;
use opi_ai::test_support::{self, MockProvider};
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::runner::{ExitCode, NDJSON_SCHEMA_VERSION, NonInteractiveRunner};

/// Parse NDJSON output into individual JSON values, one per line.
fn parse_ndjson(output: &str) -> Vec<serde_json::Value> {
    output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|_| panic!("invalid JSON: {line}")))
        .collect()
}

// ---------------------------------------------------------------------------
// Schema version header
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_mode_schema_version_header() {
    let response = test_support::text_response("hi");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run_json("hello").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    let lines = parse_ndjson(&result.stdout);
    assert!(!lines.is_empty(), "should have at least a header line");

    let header = &lines[0];
    assert_eq!(header["type"], "session_header");
    assert_eq!(header["schema_version"], NDJSON_SCHEMA_VERSION);
}

// ---------------------------------------------------------------------------
// Each line is valid JSON with a type field
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_mode_each_line_valid_json_with_type() {
    let response = test_support::text_response("hello world");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run_json("test").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    let lines = parse_ndjson(&result.stdout);
    assert!(lines.len() > 1, "should have header + at least one event");

    for (i, line) in lines.iter().enumerate() {
        assert!(
            line.get("type").is_some(),
            "line {i} missing 'type' field: {line}"
        );
    }
}

// ---------------------------------------------------------------------------
// Agent events are wrapped in AgentSessionEvent::Agent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_mode_agent_events_emitted() {
    let response = test_support::text_response("response text");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run_json("prompt").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    let lines = parse_ndjson(&result.stdout);

    // After header, all lines should be valid AgentSessionEvent::Agent
    let agent_events: Vec<_> = lines[1..].iter().filter(|v| v["type"] == "Agent").collect();
    assert!(
        !agent_events.is_empty(),
        "should have at least one Agent event"
    );
}

// ---------------------------------------------------------------------------
// AgentSessionEvent round-trip deserialization
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_mode_events_deserialize_as_session_events() {
    let response = test_support::text_response("hello");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run_json("test").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    // Every line after header should deserialize as AgentSessionEvent
    for line in result.stdout.lines().skip(1) {
        if line.is_empty() {
            continue;
        }
        let parsed: Result<AgentSessionEvent, _> = serde_json::from_str(line);
        assert!(
            parsed.is_ok(),
            "failed to deserialize as AgentSessionEvent: {line}"
        );
    }
}

// ---------------------------------------------------------------------------
// NDJSON framing: no blank lines between events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_mode_no_blank_lines() {
    let response = test_support::text_response("ok");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run_json("test").await;

    for (i, line) in result.stdout.lines().enumerate() {
        assert!(
            !line.trim().is_empty(),
            "line {i} is blank — NDJSON framing violation"
        );
    }
}

// ---------------------------------------------------------------------------
// Provider error still emits events with proper exit code
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_mode_provider_error_exit_code() {
    let response = test_support::error_response("rate limited");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run_json("test").await;

    assert_eq!(
        result.exit_code,
        ExitCode::ProviderFailure as i32,
        "should exit 4 on provider error"
    );
    // Error info goes to stderr, not stdout
    assert!(
        result.stderr.contains("rate limited"),
        "stderr should contain error: {:?}",
        result.stderr
    );
    // Still should have header even on error
    let lines = parse_ndjson(&result.stdout);
    assert_eq!(lines[0]["type"], "session_header");
}

// ---------------------------------------------------------------------------
// Tool call events emitted in JSON mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_mode_tool_call_events() {
    let first = test_support::tool_call_response(
        "tc-1",
        "read",
        r#"{"path":"Cargo.toml","offset":1,"limit":5}"#,
    );
    let second = test_support::text_response("file contents here");
    let provider = MockProvider::new("mock", vec![first, second]);

    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run_json("Read Cargo.toml").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    let lines = parse_ndjson(&result.stdout);
    // Should have tool execution events
    let tool_events: Vec<_> = lines[1..]
        .iter()
        .filter(|v| {
            let evt = &v["event"];
            evt.get("type")
                .map(|t| t.as_str().unwrap_or("").starts_with("ToolExecution"))
                .unwrap_or(false)
        })
        .collect();
    assert!(!tool_events.is_empty(), "should have tool execution events");
}

// ---------------------------------------------------------------------------
// run_json does not duplicate stdout text (no plain text mixed in)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_mode_stdout_is_only_ndjson() {
    let response = test_support::text_response("plain text response");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
    );

    let result = runner.run_json("test").await;

    // Every line should be valid JSON (not raw text)
    for (i, line) in result.stdout.lines().enumerate() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
        assert!(parsed.is_ok(), "line {i} is not valid JSON: {line}");
    }
}
