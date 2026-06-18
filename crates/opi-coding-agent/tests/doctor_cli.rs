//! Behavioral tests for the top-level `opi doctor` command (Phase 7 task 7.4).
//!
//! Two layers:
//! - **Library API** tests exercise the pure `doctor` module (`DoctorScope`,
//!   `run_doctor`, `DoctorReport`, formatters) directly. These pin scope
//!   parsing, per-scope diagnostics, the exit-code policy, the NDJSON shape,
//!   and the credential-value non-leak guarantee without spawning anything.
//! - **Binary** tests spawn the real `opi` binary to prove the top-level CLI
//!   dispatch, exit codes, scope selection, network-free behavior, and that
//!   `opi package doctor` remains a distinct, intact subcommand.
//!
//! No test makes a network call or requires real credentials. Provider scope
//! checks credential *presence* only; the credential *value* is never emitted.

use std::collections::HashMap;
use std::path::Path;

use opi_agent::Severity;
use opi_coding_agent::config::{ConfigError, OpiConfig};
use opi_coding_agent::diagnostic_bridge::diagnostic_from_config;
use opi_coding_agent::doctor::{
    DoctorContext, DoctorReport, DoctorScope, format_json, format_text, run_doctor,
};

const ANTHROPIC_ENV: &str = "ANTHROPIC_API_KEY";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_config(model: &str) -> OpiConfig {
    let mut config = OpiConfig::default();
    config.defaults.model = model.to_string();
    config
}

#[allow(clippy::type_complexity)]
fn ctx<'a>(
    config: &'a OpiConfig,
    sessions_dir: &'a Path,
    env_var: &'a dyn Fn(&str) -> Option<String>,
) -> DoctorContext<'a> {
    DoctorContext {
        config,
        config_error: None,
        workspace_root: Path::new("."),
        user_config_dir: Path::new("."),
        sessions_dir,
        term: None,
        term_program: None,
        term_features: None,
        no_color: false,
        colorterm: None,
        env_var,
    }
}

fn no_env(_: &str) -> Option<String> {
    None
}

/// Collect the distinct scope strings present in a report's NDJSON output.
fn scope_strings(report: &DoctorReport) -> Vec<String> {
    let mut scopes: Vec<String> = report
        .entries
        .iter()
        .map(|e| DoctorScope::as_str(&e.scope).to_string())
        .collect();
    scopes.sort();
    scopes.dedup();
    scopes
}

// ---------------------------------------------------------------------------
// Scope parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_list_empty_is_ok_empty() {
    // Empty/blank input means "all scopes" at the call site (caller treats
    // empty as ALL), so parsing itself succeeds with an empty selection.
    assert!(DoctorScope::parse_list("").unwrap().is_empty());
    assert!(DoctorScope::parse_list("   ").unwrap().is_empty());
}

#[test]
fn parse_list_subset() {
    let scopes = DoctorScope::parse_list("config,tui").unwrap();
    assert_eq!(scopes, vec![DoctorScope::Config, DoctorScope::Tui]);
}

#[test]
fn parse_list_trims_whitespace() {
    let scopes = DoctorScope::parse_list(" config , rpc ,tui").unwrap();
    assert_eq!(
        scopes,
        vec![DoctorScope::Config, DoctorScope::Rpc, DoctorScope::Tui]
    );
}

#[test]
fn parse_list_unknown_token_errors() {
    assert!(DoctorScope::parse_list("bogus").is_err());
    assert!(DoctorScope::parse_list("config,notascope").is_err());
}

#[test]
fn all_six_scopes_listed() {
    // The doctor surface must cover exactly the six design scopes.
    assert_eq!(
        DoctorScope::ALL.len(),
        6,
        "expected exactly six doctor scopes"
    );
}

// ---------------------------------------------------------------------------
// Config scope
// ---------------------------------------------------------------------------

#[test]
fn config_scope_reports_resolved_model() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let report = run_doctor(&[DoctorScope::Config], &ctx(&config, dir.path(), &no_env));
    assert!(
        !report.entries.is_empty(),
        "config scope must emit >=1 entry"
    );
    let has_model = format_text(&report).contains("claude-test-model");
    assert!(
        has_model,
        "config scope should mention the resolved model, got: {}",
        format_text(&report)
    );
    assert_eq!(report.entries[0].diagnostic.source, "config");
}

#[test]
fn config_scope_surfaces_config_error_as_error_severity() {
    // A config read failure must surface as an Error-severity shared diagnostic
    // (exit code 2), not an internal doctor failure (exit code 1).
    let config = test_config("anthropic:claude-test-model");
    let err = ConfigError::Read {
        path: std::path::PathBuf::from("/nonexistent/config.toml"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "config file not found"),
    };
    let expected = diagnostic_from_config(&err);
    assert_eq!(expected.severity, Severity::Error);

    let dir = tempfile::tempdir().unwrap();
    let ctx = DoctorContext {
        config_error: Some(&err),
        ..ctx(&config, dir.path(), &no_env)
    };
    let report = run_doctor(&[DoctorScope::Config], &ctx);
    assert!(
        report.has_errors(),
        "config error should produce an error-severity diagnostic"
    );
    assert_eq!(report.exit_code(), 2);
    assert!(
        report
            .entries
            .iter()
            .any(|e| e.diagnostic.severity == Severity::Error)
    );
}

#[test]
fn doctor_json_redacts_absolute_path_in_config_details() {
    // A config error carries the config file path in `details`; the public
    // --json boundary must redact it (Phase 7 design: details are redacted
    // structured metadata, absolute paths are not emitted by default).
    let config = test_config("anthropic:claude-test-model");
    let leak_path = "/tmp/opi-secret-leak/config.toml";
    let err = ConfigError::Read {
        path: std::path::PathBuf::from(leak_path),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "config file not found"),
    };
    let dir = tempfile::tempdir().unwrap();
    let ctx = DoctorContext {
        config_error: Some(&err),
        ..ctx(&config, dir.path(), &no_env)
    };
    let report = run_doctor(&[DoctorScope::Config], &ctx);
    let json = format_json(&report);
    assert!(
        !json.contains(leak_path),
        "absolute config path leaked into doctor --json details: {json}",
    );
    assert!(
        json.contains("[REDACTED]"),
        "expected a redaction marker in details, got: {json}",
    );
}

// ---------------------------------------------------------------------------
// Provider scope (network-free; credential presence only)
// ---------------------------------------------------------------------------

#[test]
fn provider_scope_credential_present_is_info() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let map: HashMap<&str, String> = [(ANTHROPIC_ENV, "sk-present".into())].into_iter().collect();
    let env = |n: &str| map.get(n).cloned();
    let report = run_doctor(&[DoctorScope::Provider], &ctx(&config, dir.path(), &env));
    assert!(
        report
            .entries
            .iter()
            .any(|e| e.diagnostic.severity == Severity::Info),
        "present credentials should be Info, got: {:?}",
        report.entries
    );
    assert!(!report.has_errors());
}

#[test]
fn provider_scope_credential_absent_is_warning() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let report = run_doctor(&[DoctorScope::Provider], &ctx(&config, dir.path(), &no_env));
    assert!(
        report
            .entries
            .iter()
            .any(|e| e.diagnostic.severity == Severity::Warning),
        "absent credentials should be Warning, got: {:?}",
        report.entries
    );
    // Missing credentials is a warning, not an error -> still exit 0.
    assert!(!report.has_errors());
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn provider_scope_never_emits_credential_value() {
    // The credential *value* must never appear in any diagnostic field, even
    // though doctor inspects credential presence for the selected provider.
    let sentinel = "sk-test-DO-NOT-LEAK-1234567890";
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let map: HashMap<&str, String> = [(ANTHROPIC_ENV, sentinel.into())].into_iter().collect();
    let env = |n: &str| map.get(n).cloned();
    let report = run_doctor(&[DoctorScope::Provider], &ctx(&config, dir.path(), &env));
    let json = format_json(&report);
    let text = format_text(&report);
    assert!(
        !json.contains(sentinel),
        "credential value leaked into JSON output: {json}"
    );
    assert!(
        !text.contains(sentinel),
        "credential value leaked into text output: {text}"
    );
}

// ---------------------------------------------------------------------------
// Session scope
// ---------------------------------------------------------------------------

#[test]
fn session_scope_reports_session_count() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    // Drop two fake session JSONL files into the sessions dir.
    std::fs::write(dir.path().join("aaa.jsonl"), "{}\n").unwrap();
    std::fs::write(dir.path().join("bbb.jsonl"), "{}\n").unwrap();
    let report = run_doctor(&[DoctorScope::Session], &ctx(&config, dir.path(), &no_env));
    let text = format_text(&report);
    assert!(
        text.contains('2') || text.contains("two"),
        "session scope should report the session count, got: {text}"
    );
    assert_eq!(report.entries[0].diagnostic.source, "session");
    assert!(!report.has_errors());
}

#[test]
fn session_scope_missing_createable_dir_is_info() {
    // A not-yet-created sessions dir under an existing parent is normal on a
    // fresh install and must not be an error.
    let config = test_config("anthropic:claude-test-model");
    let parent = tempfile::tempdir().unwrap();
    let missing = parent.path().join("sessions");
    let report = run_doctor(&[DoctorScope::Session], &ctx(&config, &missing, &no_env));
    assert!(
        !report.has_errors(),
        "missing-but-createable sessions dir should not error, got: {:?}",
        report.entries
    );
}

// ---------------------------------------------------------------------------
// TUI scope
// ---------------------------------------------------------------------------

#[test]
fn tui_scope_detects_iterm_protocol_and_no_color() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let ctx = DoctorContext {
        term: Some("xterm-256color"),
        term_program: Some("iTerm.app"),
        term_features: None,
        no_color: true,
        colorterm: None,
        ..ctx(&config, dir.path(), &no_env)
    };
    let report = run_doctor(&[DoctorScope::Tui], &ctx);
    let text = format_text(&report).to_lowercase();
    assert!(
        text.contains("iterm"),
        "tui scope should report the iTerm2 protocol, got: {text}"
    );
    assert!(
        text.contains("no color") || text.contains("no_color"),
        "tui scope should report no-color state, got: {text}"
    );
    assert_eq!(report.entries[0].diagnostic.source, "tui");
}

#[test]
fn tui_scope_fallback_when_no_graphics_protocol() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let report = run_doctor(&[DoctorScope::Tui], &ctx(&config, dir.path(), &no_env));
    assert!(!report.has_errors());
    assert_eq!(report.entries[0].diagnostic.source, "tui");
}

// ---------------------------------------------------------------------------
// RPC scope
// ---------------------------------------------------------------------------

#[test]
fn rpc_scope_reports_schema_version() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let report = run_doctor(&[DoctorScope::Rpc], &ctx(&config, dir.path(), &no_env));
    let text = format_text(&report);
    let version = opi_coding_agent::rpc::RPC_SCHEMA_VERSION;
    assert!(
        text.contains(&version.to_string()),
        "rpc scope should report the schema version {version}, got: {text}"
    );
    assert_eq!(report.entries[0].diagnostic.source, "rpc");
    assert!(!report.has_errors());
}

// ---------------------------------------------------------------------------
// Package scope (delegates to the installed-package resolver)
// ---------------------------------------------------------------------------

#[test]
fn package_scope_empty_workspace_is_info_no_errors() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let report = run_doctor(&[DoctorScope::Package], &ctx(&config, dir.path(), &no_env));
    assert!(
        !report.entries.is_empty(),
        "package scope must emit >=1 entry even with no packages"
    );
    assert!(
        !report.has_errors(),
        "empty workspace should not produce package errors, got: {:?}",
        report.entries
    );
    assert!(
        report
            .entries
            .iter()
            .all(|e| e.diagnostic.source == "package")
    );
}

// ---------------------------------------------------------------------------
// Whole-report behavior
// ---------------------------------------------------------------------------

#[test]
fn run_doctor_all_scopes_covers_every_scope() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let report = run_doctor(DoctorScope::ALL, &ctx(&config, dir.path(), &no_env));
    let scopes = scope_strings(&report);
    assert_eq!(
        scopes.len(),
        6,
        "default doctor run must cover all six scopes, got: {scopes:?}"
    );
}

#[test]
fn exit_code_no_errors_is_zero() {
    let report = DoctorReport::default();
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn exit_code_with_error_is_two() {
    let config = test_config("anthropic:claude-test-model");
    let err = ConfigError::Read {
        path: std::path::PathBuf::from("/nonexistent/config.toml"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
    };
    let dir = tempfile::tempdir().unwrap();
    let ctx = DoctorContext {
        config_error: Some(&err),
        ..ctx(&config, dir.path(), &no_env)
    };
    let report = run_doctor(DoctorScope::ALL, &ctx);
    assert_eq!(report.exit_code(), 2);
}

#[test]
fn format_json_is_ndjson_with_required_fields() {
    let config = test_config("anthropic:claude-test-model");
    let dir = tempfile::tempdir().unwrap();
    let report = run_doctor(DoctorScope::ALL, &ctx(&config, dir.path(), &no_env));
    let json = format_json(&report);
    assert!(!json.trim().is_empty(), "json output must not be empty");
    for line in json.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("line is not valid JSON: {line}\nerror: {e}"));
        assert!(value.get("scope").is_some(), "missing scope: {line}");
        assert!(value.get("severity").is_some(), "missing severity: {line}");
        assert!(value.get("code").is_some(), "missing code: {line}");
        assert!(value.get("source").is_some(), "missing source: {line}");
        assert!(value.get("message").is_some(), "missing message: {line}");
    }
}

// ===========================================================================
// Binary (integration) tests — spawn the real `opi` binary.
// ===========================================================================

fn opi_bin() -> String {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();
    for profile in &["debug", "release"] {
        let mut path = workspace_root.join("target").join(profile).join("opi");
        if cfg!(windows) {
            path.set_extension("exe");
        }
        if path.exists() {
            return path.to_string_lossy().into_owned();
        }
    }
    let mut path = workspace_root.join("target/debug/opi");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path.to_string_lossy().into_owned()
}

fn run_opi(args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let bin = opi_bin();
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(args).current_dir(tmp.path()).env_clear();
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to run {bin}: {e}"))
}

#[test]
fn doctor_clean_env_exits_zero() {
    // No credentials, no config files, from a clean tempdir: doctor should run
    // with only warnings/info (missing provider credentials is a warning) and
    // exit 0.
    let output = run_opi(&["doctor"], &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0 for clean doctor run\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        !stdout.trim().is_empty(),
        "doctor should print a report to stdout, got empty stdout\nstderr: {stderr}"
    );
}

#[test]
fn doctor_json_reports_all_scopes_without_network() {
    // Acceptance scenario `phase7-doctor-all-scopes`: every scope is reported
    // as NDJSON with no network and no credentials.
    let output = run_opi(&["doctor", "--json"], &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0\nstdout: {stdout}\nstderr: {stderr}",
    );

    let mut scopes: Vec<String> = Vec::new();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("doctor --json line not valid JSON: {line}\n{e}"));
        if let Some(scope) = value.get("scope").and_then(|v| v.as_str()) {
            scopes.push(scope.to_string());
        }
    }
    scopes.sort();
    scopes.dedup();
    assert_eq!(
        scopes,
        vec!["config", "package", "provider", "rpc", "session", "tui"],
        "doctor --json must report all six scopes, got: {scopes:?}\nstdout: {stdout}",
    );
}

#[test]
fn doctor_scope_subset_reports_only_requested_scopes() {
    let output = run_opi(&["doctor", "--json", "--scope", "config,rpc"], &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0; stdout: {stdout}",
    );
    let mut scopes: Vec<String> = Vec::new();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).expect("doctor --json line must be valid JSON");
        if let Some(scope) = value.get("scope").and_then(|v| v.as_str()) {
            scopes.push(scope.to_string());
        }
    }
    scopes.sort();
    scopes.dedup();
    assert_eq!(
        scopes,
        vec!["config", "rpc"],
        "only requested scopes, got: {scopes:?}"
    );
}

#[test]
fn doctor_unknown_scope_exits_one() {
    // An unknown scope token is an internal doctor command failure -> exit 1.
    let output = run_opi(&["doctor", "--scope", "bogus"], &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1 for unknown scope\nstderr: {stderr}",
    );
}

#[test]
fn doctor_config_error_exits_two() {
    // A malformed config must surface as an error-severity diagnostic (exit 2),
    // reported through the shared diagnostic shape, not exit 1.
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("broken.toml");
    std::fs::write(&config_path, "this is = = not valid toml [[[\n").unwrap();
    let bin = opi_bin();
    let output = std::process::Command::new(&bin)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "doctor",
            "--json",
        ])
        .current_dir(tmp.path())
        .env_clear()
        .output()
        .unwrap_or_else(|e| panic!("failed to run {bin}: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for config error\nstdout: {stdout}\nstderr: {stderr}",
    );
    // The error must be carried as a structured diagnostic in --json output.
    let saw_config_error = stdout.lines().any(|line| {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return false;
        };
        value.get("source").and_then(|v| v.as_str()) == Some("config")
            && value.get("severity").and_then(|v| v.as_str()) == Some("error")
    });
    assert!(
        saw_config_error,
        "expected a config error diagnostic in --json, got stdout: {stdout}",
    );
}

#[test]
fn doctor_does_not_leak_credential_value_end_to_end() {
    // Set a real-looking credential; doctor --json --scope provider must report
    // presence but never the value itself.
    let sentinel = "sk-test-SECRET-VALUE-xyz";
    let output = run_opi(
        &["doctor", "--json", "--scope", "provider"],
        &[(ANTHROPIC_ENV, sentinel)],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        !stdout.contains(sentinel),
        "credential value leaked into doctor output: {stdout}",
    );
}

#[test]
fn package_doctor_remains_a_distinct_intact_subcommand() {
    // `opi package doctor` is a separate subcommand from the top-level
    // `opi doctor` and must keep working unchanged.
    let output = run_opi(&["package", "doctor"], &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let _ = String::from_utf8_lossy(&output.stderr); // package doctor prints to stdout/stderr
    // A clean environment (no installed packages, env_clear) yields no package
    // diagnostics, so package doctor must exit 0. Pin the exact code so an
    // exit-semantic regression cannot hide behind a 0-or-2 disjunction.
    assert!(
        output.status.code() == Some(0),
        "package doctor should exit 0 in a clean environment\nstdout: {stdout}",
    );
}
