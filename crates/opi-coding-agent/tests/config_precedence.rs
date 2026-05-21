//! Tests for config precedence (task 1.16).
//!
//! Verifies: CLI > env > project config > user config > built-in defaults.

use std::fs;

use opi_coding_agent::config::{ConfigSource, OpiConfig, load_config_file, resolve_config};

fn write_config(dir: &std::path::Path, subpath: &str, contents: &str) -> std::path::PathBuf {
    if let Some(parent) = std::path::Path::new(subpath).parent() {
        let parent_dir = dir.join(parent);
        fs::create_dir_all(&parent_dir).unwrap();
    }
    let path = dir.join(subpath);
    fs::write(&path, contents).unwrap();
    path
}

fn user_config_path(temp: &std::path::Path) -> std::path::PathBuf {
    temp.join("user_config").join("config.toml")
}

fn project_dir(temp: &std::path::Path) -> std::path::PathBuf {
    temp.join("project")
}

// ---------------------------------------------------------------------------
// CLI overrides everything
// ---------------------------------------------------------------------------

#[test]
fn cli_model_overrides_user_config() {
    let temp = tempfile::tempdir().unwrap();

    write_config(
        temp.path(),
        "user_config/config.toml",
        r#"
[defaults]
model = "anthropic:claude-opus-4"
"#,
    );

    write_config(
        temp.path(),
        "project/.opi/config.toml",
        r#"
[defaults]
model = "anthropic:claude-haiku-4"
"#,
    );

    let config = resolve_config(ConfigSource {
        cli_model: Some("anthropic:claude-sonnet-4".into()),
        config_path: None,
        env_model: None,
        project_dir: Some(project_dir(temp.path())),
        user_config_path: Some(user_config_path(temp.path())),
    })
    .unwrap();

    assert_eq!(config.defaults.model, "anthropic:claude-sonnet-4");
}

// ---------------------------------------------------------------------------
// Env overrides user and project config
// ---------------------------------------------------------------------------

#[test]
fn env_model_overrides_user_config() {
    let temp = tempfile::tempdir().unwrap();

    write_config(
        temp.path(),
        "user_config/config.toml",
        r#"
[defaults]
model = "anthropic:claude-opus-4"
"#,
    );

    let config = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: Some("anthropic:claude-haiku-4".into()),
        project_dir: None,
        user_config_path: Some(user_config_path(temp.path())),
    })
    .unwrap();

    assert_eq!(config.defaults.model, "anthropic:claude-haiku-4");
}

// ---------------------------------------------------------------------------
// Project config overrides user config
// ---------------------------------------------------------------------------

#[test]
fn project_config_overrides_user_config() {
    let temp = tempfile::tempdir().unwrap();

    write_config(
        temp.path(),
        "user_config/config.toml",
        r#"
[defaults]
model = "anthropic:claude-opus-4"
max_iterations = 200
"#,
    );

    write_config(
        temp.path(),
        "project/.opi/config.toml",
        r#"
[defaults]
model = "anthropic:claude-sonnet-4"
"#,
    );

    let config = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: None,
        project_dir: Some(project_dir(temp.path())),
        user_config_path: Some(user_config_path(temp.path())),
    })
    .unwrap();

    // Project model wins
    assert_eq!(config.defaults.model, "anthropic:claude-sonnet-4");
    // User's max_iterations still applies (project didn't override it)
    assert_eq!(config.defaults.max_iterations, 200);
}

// ---------------------------------------------------------------------------
// User config overrides defaults
// ---------------------------------------------------------------------------

#[test]
fn user_config_overrides_defaults() {
    let temp = tempfile::tempdir().unwrap();

    write_config(
        temp.path(),
        "user_config/config.toml",
        r#"
[defaults]
model = "anthropic:claude-opus-4"
max_iterations = 100
"#,
    );

    let config = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: None,
        project_dir: None,
        user_config_path: Some(user_config_path(temp.path())),
    })
    .unwrap();

    assert_eq!(config.defaults.model, "anthropic:claude-opus-4");
    assert_eq!(config.defaults.max_iterations, 100);
}

// ---------------------------------------------------------------------------
// Built-in defaults when nothing is configured
// ---------------------------------------------------------------------------

#[test]
fn defaults_when_nothing_configured() {
    let temp = tempfile::tempdir().unwrap();

    let config = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: None,
        project_dir: Some(temp.path().join("no_project")),
        user_config_path: Some(temp.path().join("no_user").join("config.toml")),
    })
    .unwrap();

    let defaults = OpiConfig::default();
    assert_eq!(config.defaults.model, defaults.defaults.model);
    assert_eq!(
        config.defaults.max_iterations,
        defaults.defaults.max_iterations
    );
    assert_eq!(
        config.defaults.tool_timeout_ms,
        defaults.defaults.tool_timeout_ms
    );
}

// ---------------------------------------------------------------------------
// Full precedence chain: CLI > env > project > user > defaults
// ---------------------------------------------------------------------------

#[test]
fn full_precedence_chain() {
    let temp = tempfile::tempdir().unwrap();

    write_config(
        temp.path(),
        "user_config/config.toml",
        r#"
[defaults]
model = "user-model"
"#,
    );

    write_config(
        temp.path(),
        "project/.opi/config.toml",
        r#"
[defaults]
model = "project-model"
"#,
    );

    // CLI wins over env, project, user
    let config = resolve_config(ConfigSource {
        cli_model: Some("cli-model".into()),
        config_path: None,
        env_model: Some("env-model".into()),
        project_dir: Some(project_dir(temp.path())),
        user_config_path: Some(user_config_path(temp.path())),
    })
    .unwrap();
    assert_eq!(config.defaults.model, "cli-model");

    // Env wins over project, user (no CLI)
    let config = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: Some("env-model".into()),
        project_dir: Some(project_dir(temp.path())),
        user_config_path: Some(user_config_path(temp.path())),
    })
    .unwrap();
    assert_eq!(config.defaults.model, "env-model");

    // Project wins over user (no CLI, no env)
    let config = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: None,
        project_dir: Some(project_dir(temp.path())),
        user_config_path: Some(user_config_path(temp.path())),
    })
    .unwrap();
    assert_eq!(config.defaults.model, "project-model");

    // User wins over defaults (no CLI, no env, no project)
    let config = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: None,
        project_dir: None,
        user_config_path: Some(user_config_path(temp.path())),
    })
    .unwrap();
    assert_eq!(config.defaults.model, "user-model");
}

// ---------------------------------------------------------------------------
// Malformed user config errors out
// ---------------------------------------------------------------------------

#[test]
fn malformed_user_config_is_error() {
    let temp = tempfile::tempdir().unwrap();

    write_config(
        temp.path(),
        "user_config/config.toml",
        r#"
[invalid toml !!!
"#,
    );

    let result = resolve_config(ConfigSource {
        cli_model: None,
        config_path: None,
        env_model: None,
        project_dir: None,
        user_config_path: Some(user_config_path(temp.path())),
    });

    assert!(result.is_err(), "malformed user config should be an error");
}

// ---------------------------------------------------------------------------
// Malformed project config errors out
// ---------------------------------------------------------------------------

#[test]
fn malformed_project_config_is_error() {
    let temp = tempfile::tempdir().unwrap();

    write_config(
        temp.path(),
        "project/.opi/config.toml",
        r#"
[broken [[[
"#,
    );

    let result = load_config_file(&temp.path().join("project").join(".opi").join("config.toml"));
    assert!(
        result.is_err(),
        "malformed project config should be an error"
    );
}
