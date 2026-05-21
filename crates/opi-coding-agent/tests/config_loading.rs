//! Tests for TOML config loading (task 1.16).
//!
//! DoD: "missing defaults and malformed errors tested"

use std::fs;
use std::path::Path;

use opi_coding_agent::config::{ConfigSource, OpiConfig, load_config_file, resolve_config};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_temp_config(dir: &Path, contents: &str) -> std::path::PathBuf {
    let path = dir.join("config.toml");
    fs::write(&path, contents).unwrap();
    path
}

// ---------------------------------------------------------------------------
// Missing config → defaults (silent fallback)
// ---------------------------------------------------------------------------

#[test]
fn missing_file_returns_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nonexistent.toml");
    let config = load_config_file(&missing).unwrap();
    let defaults = OpiConfig::default();
    assert_eq!(
        config.defaults.model, defaults.defaults.model,
        "missing file should fall back to default model"
    );
}

#[test]
fn missing_file_does_not_error() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nonexistent.toml");
    let result = load_config_file(&missing);
    assert!(
        result.is_ok(),
        "missing optional config file should not error, got {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Valid TOML → correct parsed values
// ---------------------------------------------------------------------------

#[test]
fn valid_config_parses_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[defaults]
model = "anthropic:claude-sonnet-4"
max_iterations = 100
tool_timeout_ms = 60000
theme = "dark"
"#,
    );
    let config = load_config_file(&path).unwrap();
    assert_eq!(config.defaults.model, "anthropic:claude-sonnet-4");
    assert_eq!(config.defaults.max_iterations, 100);
    assert_eq!(config.defaults.tool_timeout_ms, 60000);
    assert_eq!(config.defaults.theme, "dark");
}

#[test]
fn valid_config_parses_thinking() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[thinking]
enabled = true
budget_tokens = 20000
"#,
    );
    let config = load_config_file(&path).unwrap();
    assert!(config.thinking.enabled);
    assert_eq!(config.thinking.budget_tokens, 20000);
}

#[test]
fn valid_config_parses_providers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.anthropic]
api_key_env = "MY_ANTHROPIC_KEY"
"#,
    );
    let config = load_config_file(&path).unwrap();
    assert_eq!(config.providers.anthropic.api_key_env, "MY_ANTHROPIC_KEY");
}

// ---------------------------------------------------------------------------
// Malformed TOML → clear error
// ---------------------------------------------------------------------------

#[test]
fn malformed_toml_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
this is not valid toml [[[

[defaults
model = broken
"#,
    );
    let result = load_config_file(&path);
    assert!(result.is_err(), "malformed TOML should return error");
}

#[test]
fn malformed_error_message_is_clear() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[invalid toml !!
"#,
    );
    let result = load_config_file(&path);
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("config") || msg.contains("parse") || msg.contains("TOML"),
        "error message should mention config/parse/TOML, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Partial config → defaults for missing fields
// ---------------------------------------------------------------------------

#[test]
fn partial_config_fills_missing_with_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[defaults]
model = "anthropic:claude-sonnet-4"
"#,
    );
    let config = load_config_file(&path).unwrap();
    assert_eq!(config.defaults.model, "anthropic:claude-sonnet-4");
    let defaults = OpiConfig::default();
    assert_eq!(
        config.defaults.max_iterations, defaults.defaults.max_iterations,
        "missing field should use default"
    );
    assert_eq!(
        config.defaults.tool_timeout_ms, defaults.defaults.tool_timeout_ms,
        "missing field should use default"
    );
}

#[test]
fn empty_config_uses_all_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(dir.path(), "");
    let config = load_config_file(&path).unwrap();
    let defaults = OpiConfig::default();
    assert_eq!(config.defaults.model, defaults.defaults.model);
    assert_eq!(
        config.defaults.max_iterations,
        defaults.defaults.max_iterations
    );
}

// ---------------------------------------------------------------------------
// resolve_config: defaults when no sources
// ---------------------------------------------------------------------------

#[test]
fn resolve_with_no_sources_returns_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let config = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: None,
        project_dir: Some(dir.path().to_path_buf()),
        user_config_path: None,
    })
    .unwrap();
    let defaults = OpiConfig::default();
    assert_eq!(config.defaults.model, defaults.defaults.model);
}

// ---------------------------------------------------------------------------
// Unknown fields ignored gracefully
// ---------------------------------------------------------------------------

#[test]
fn unknown_fields_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[defaults]
model = "anthropic:claude-sonnet-4"

[future_feature]
some_new_option = true
"#,
    );
    let result = load_config_file(&path);
    assert!(
        result.is_ok(),
        "unknown fields should be ignored, got {:?}",
        result
    );
}
