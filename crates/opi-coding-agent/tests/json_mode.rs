//! JSON mode contract tests (task 2.14).
//!
//! DoD: "one AgentSessionEvent JSON object per line to stdout,
//!       schema version field, contract tests for framing"
//!
//! The `session_summary` line is the `AgentSessionEvent::SessionSummary`
//! variant (renamed for wire compatibility); all stdout lines after the
//! header round-trip through `AgentSessionEvent`.

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
        Vec::new(),
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
        Vec::new(),
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
        Vec::new(),
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
        Vec::new(),
    );

    let result = runner.run_json("test").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    // Every line after the schema header should deserialize as an
    // AgentSessionEvent. The wire contract is "one AgentSessionEvent per line",
    // so the session_summary line is now part of the union — no special-casing.
    for line in result.stdout.lines().skip(1) {
        if line.is_empty() {
            continue;
        }
        let parsed: Result<AgentSessionEvent, _> = serde_json::from_str(line);
        assert!(
            parsed.is_ok(),
            "failed to deserialize as AgentSessionEvent: {line}: {:?}",
            parsed.err(),
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
        Vec::new(),
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
        Vec::new(),
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
        Vec::new(),
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
        Vec::new(),
    );

    let result = runner.run_json("test").await;

    // Every line should be valid JSON (not raw text)
    for (i, line) in result.stdout.lines().enumerate() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
        assert!(parsed.is_ok(), "line {i} is not valid JSON: {line}");
    }
}

#[tokio::test]
async fn json_mode_emits_session_summary_with_token_totals() {
    let response = test_support::text_response("hi");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "anthropic:claude-sonnet-4".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
        Vec::new(),
    );

    let result = runner.run_json("test").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    let parsed = parse_ndjson(&result.stdout);
    let summary = parsed
        .iter()
        .find(|v| v.get("type").and_then(|t| t.as_str()) == Some("session_summary"))
        .expect("session_summary line should be emitted");

    assert!(
        summary.get("session_id").is_some(),
        "summary has session_id"
    );
    assert!(summary.get("turns").is_some(), "summary has turn count");
    assert!(summary.get("tokens").is_some(), "summary has token totals");
    assert_eq!(
        summary
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "anthropic:claude-sonnet-4"
    );
}

// ---------------------------------------------------------------------------
// Subprocess E2E: exercise the full CLI wiring for --json
// ---------------------------------------------------------------------------

fn opi_binary() -> std::path::PathBuf {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let workspace_root = std::path::PathBuf::from(&crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("crate should be in crates/opi-coding-agent")
        .to_path_buf();
    let bin_name = if cfg!(windows) { "opi.exe" } else { "opi" };
    let path = workspace_root.join("target").join("debug").join(bin_name);
    assert!(
        path.exists(),
        "opi binary must be built: run `cargo build -p opi-coding-agent`"
    );
    path
}

fn build_opi_if_needed() {
    let bin = opi_binary();
    if !bin.exists() {
        let status = std::process::Command::new("cargo")
            .args(["build", "-p", "opi-coding-agent"])
            .status()
            .expect("failed to run cargo build");
        assert!(status.success(), "cargo build failed");
    }
}

#[test]
fn e2e_json_mode_auth_failure_produces_ndjson_stderr() {
    // Without API keys, the binary should fail with an auth error.
    // The test validates CLI wiring: arg parsing → config → provider → runner → exit code.
    build_opi_if_needed();

    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(opi_binary())
        .env("OPI_SESSIONS_DIR", dir.path())
        .env("ANTHROPIC_API_KEY", "") // ensure no key
        .arg("--json")
        .arg("--model")
        .arg("anthropic:claude-sonnet-4")
        .arg("test prompt")
        .output()
        .expect("failed to run opi");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Auth failure should produce non-zero exit code
    assert!(
        !output.status.success(),
        "expected non-zero exit code without API key, got {}",
        output.status
    );

    // stderr should mention the auth problem (either missing key or auth failure)
    assert!(
        stderr.contains("API key")
            || stderr.contains("api key")
            || stderr.contains("missing")
            || stderr.contains("authentication")
            || stderr.contains("access denied"),
        "stderr should mention auth failure, got: {stderr}"
    );

    // stdout should not contain non-JSON text (CLI wiring must route all
    // diagnostics to stderr, keeping stdout reserved for NDJSON)
    if !stdout.is_empty() {
        for (i, line) in stdout.lines().enumerate() {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
            assert!(
                parsed.is_ok(),
                "stdout line {i} is not valid JSON (CLI should not write plain text to stdout in --json mode): {line}"
            );
        }
    }
}

#[test]
fn e2e_json_mode_schema_header_on_stdout() {
    // Even when the run fails (no API key), the first stdout line should be
    // the schema version header if any output was produced.
    build_opi_if_needed();

    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(opi_binary())
        .env("OPI_SESSIONS_DIR", dir.path())
        .env("ANTHROPIC_API_KEY", "")
        .arg("--json")
        .arg("--model")
        .arg("anthropic:claude-sonnet-4")
        .arg("test prompt")
        .output()
        .expect("failed to run opi");

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        let first_line = stdout.lines().next().unwrap_or("");
        let header: serde_json::Value = serde_json::from_str(first_line)
            .unwrap_or_else(|e| panic!("first stdout line must be JSON: {e}: {first_line}"));
        assert_eq!(
            header["type"], "session_header",
            "first line must be session_header"
        );
        assert_eq!(header["schema_version"], 1, "schema_version must be 1");
    }
}

#[tokio::test]
async fn json_mode_session_summary_roundtrips_through_agent_session_event() {
    // The session_summary line must be the AgentSessionEvent::SessionSummary
    // variant — not an ad-hoc JSON shape. Consumers parsing the NDJSON stream
    // as a sequence of AgentSessionEvent values rely on this.
    let response = test_support::text_response("hi");
    let provider = MockProvider::new("mock", vec![response]);
    let mut runner = NonInteractiveRunner::new(
        Box::new(provider),
        "anthropic:claude-sonnet-4".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
        false,
        None,
        Vec::new(),
    );

    let result = runner.run_json("test").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    let summary_line = result
        .stdout
        .lines()
        .find(|l| l.contains(r#""type":"session_summary""#))
        .expect("session_summary line emitted");

    let parsed: AgentSessionEvent = serde_json::from_str(summary_line)
        .unwrap_or_else(|e| panic!("session_summary line must round-trip: {e}: {summary_line}"));
    match parsed {
        AgentSessionEvent::SessionSummary {
            ref model, turns, ..
        } => {
            assert_eq!(model, "anthropic:claude-sonnet-4");
            assert!(turns >= 1, "turns should advance after a successful run");
        }
        other => panic!("expected SessionSummary, got {other:?}"),
    }
}

// ===========================================================================
// Phase 7 task 7.5 — JSON exposure: startup diagnostics, run-summary
// diagnostic counts, and the versioned redacted trace envelope.
// ===========================================================================

mod phase7 {
    use super::parse_ndjson;
    use opi_agent::TRACE_SCHEMA_VERSION;
    use opi_agent::extension::ExtensionRegistry;
    use opi_agent::session_event::AgentSessionEvent;
    use opi_ai::provider::{Provider, ProviderError};
    use opi_ai::test_support::{self, MockProvider, MockResponse};
    use opi_coding_agent::config::OpiConfig;
    use opi_coding_agent::policy::ToolSelection;
    use opi_coding_agent::runner::{ExitCode, NonInteractiveRunner};
    use opi_coding_agent::runtime_packages::RuntimePackageStartup;

    fn workspace_root() -> std::path::PathBuf {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    }

    fn runner_with_startup(
        provider: Box<dyn Provider>,
        diagnostics: Vec<String>,
        trace_path: Option<std::path::PathBuf>,
    ) -> NonInteractiveRunner {
        NonInteractiveRunner::new_with_resume_and_runtime_packages(
            provider,
            "mock-model".into(),
            OpiConfig::default(),
            workspace_root(),
            false,
            None,
            Vec::new(),
            None,
            ToolSelection::Default,
            Some(RuntimePackageStartup {
                extension_registry: ExtensionRegistry::new(),
                installed_packages: Vec::new(),
                diagnostics,
            }),
            trace_path,
        )
        .expect("non-interactive tool policy should be valid")
    }

    /// Startup diagnostics appear before accepted prompt output (clause 1).
    #[tokio::test]
    async fn phase7_startup_diagnostics_and_counts() {
        let provider = MockProvider::new("mock", vec![test_support::text_response("hi")]);
        let mut runner =
            runner_with_startup(Box::new(provider), vec!["phase7 startup warn".into()], None);
        let result = runner.run_json("hello").await;
        assert_eq!(result.exit_code, ExitCode::Success as i32);

        let lines = parse_ndjson(&result.stdout);
        // lines[0] is the session_header; the startup diagnostics line must
        // follow it and precede the first Agent event.
        assert_eq!(lines[0]["type"], "session_header", "first line is header");
        assert_eq!(
            lines[1]["type"], "StartupDiagnostics",
            "second line must be startup diagnostics"
        );
        assert_eq!(
            lines[1]["diagnostics"][0], "phase7 startup warn",
            "startup diagnostic carried verbatim"
        );
        let agent_idx = lines
            .iter()
            .position(|l| l["type"] == "Agent")
            .expect("at least one Agent event");
        assert!(
            agent_idx > 1,
            "startup diagnostics must precede the first Agent event"
        );
    }

    /// Run summary carries structured diagnostic counts (clause 2).
    #[tokio::test]
    async fn phase7_run_summary_carries_diagnostic_counts() {
        // One retryable error then success: emits a Warning retry-attempt and
        // an Info retry-succeeded diagnostic, which must aggregate into counts.
        let provider = MockProvider::new_with_errors(
            "mock",
            vec![
                MockResponse::Error(ProviderError::RateLimited {
                    retry_after_ms: Some(1),
                }),
                MockResponse::Events(test_support::text_response("ok")),
            ],
        );
        let mut runner = runner_with_startup(Box::new(provider), Vec::new(), None);
        let result = runner.run_json("hello").await;
        assert_eq!(result.exit_code, ExitCode::Success as i32);

        let summary_line = result
            .stdout
            .lines()
            .find(|l| l.contains(r#""type":"session_summary""#))
            .expect("session_summary line emitted");
        let parsed: AgentSessionEvent = serde_json::from_str(summary_line).unwrap();
        match parsed {
            AgentSessionEvent::SessionSummary {
                diagnostics: Some(counts),
                ..
            } => {
                assert!(
                    counts.warning >= 1,
                    "expected >=1 warning (retry attempt), got {}",
                    counts.warning
                );
                assert!(
                    counts.info >= 1,
                    "expected >=1 info (retry succeeded), got {}",
                    counts.info
                );
            }
            AgentSessionEvent::SessionSummary {
                diagnostics: None, ..
            } => {
                panic!("run summary must carry diagnostic counts")
            }
            other => panic!("expected SessionSummary, got {other:?}"),
        }
    }

    /// The requested trace envelope is versioned and does not leak the prompt
    /// (clause 6; redaction applied at the trace emit boundary).
    #[tokio::test]
    async fn phase7_trace_envelope_versioned_and_no_prompt_leak() {
        let dir = tempfile::tempdir().expect("tempdir");
        let trace_path = dir.path().join("trace.jsonl");
        let secret = "sk-ant-AAAAAAAAAAAAAAAAAAAAleak";
        let prompt = format!("my secret plan {secret}");

        let provider = MockProvider::new("mock", vec![test_support::text_response("hi")]);
        let mut runner =
            runner_with_startup(Box::new(provider), Vec::new(), Some(trace_path.clone()));
        let result = runner.run_json(&prompt).await;
        assert_eq!(result.exit_code, ExitCode::Success as i32);

        let contents =
            std::fs::read_to_string(&trace_path).expect("trace file written for the run");
        assert!(!contents.is_empty(), "trace envelope must be produced");
        let records: Vec<serde_json::Value> = contents
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).expect("each line is a JSON record"))
            .collect();
        assert!(!records.is_empty(), "at least one trace record");
        for record in &records {
            assert_eq!(
                record["schema_version"],
                serde_json::json!(TRACE_SCHEMA_VERSION),
                "every trace record carries the unstable schema version"
            );
        }
        // No prompt leak: the prompt text and the secret-like token must not
        // appear anywhere in the trace envelope.
        assert!(
            !contents.contains(&prompt),
            "trace must not leak the prompt text"
        );
        assert!(
            !contents.contains(secret),
            "trace must not leak secret-like content"
        );
    }
}
