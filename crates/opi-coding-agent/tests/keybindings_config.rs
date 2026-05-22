//! Keybindings config TOML parsing tests (task 2.13).
//!
//! DoD: "KeybindingsConfig struct added to OpiConfig with [keybindings] TOML parsing,
//!       configurable submit/abort/new_line passed to opi-tui input handler,
//!       tested with non-default bindings"

use opi_coding_agent::config::load_config_file;
use tempfile::NamedTempFile;

fn write_toml(contents: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut f, contents.as_bytes()).unwrap();
    f
}

// ---------------------------------------------------------------------------
// [keybindings] TOML section parsing
// ---------------------------------------------------------------------------

#[test]
fn keybindings_section_parsed_into_config() {
    let toml = r#"
[keybindings]
submit = "ctrl+j"
abort = "ctrl+c"
new_line = "shift+enter"
"#;
    let f = write_toml(toml);
    let config = load_config_file(f.path()).unwrap();
    assert_eq!(config.keybindings.submit, "ctrl+j");
    assert_eq!(config.keybindings.abort, "ctrl+c");
    assert_eq!(config.keybindings.new_line, "shift+enter");
}

#[test]
fn keybindings_missing_section_uses_defaults() {
    let toml = r#"
[defaults]
model = "anthropic:claude-sonnet-4"
"#;
    let f = write_toml(toml);
    let config = load_config_file(f.path()).unwrap();
    assert_eq!(config.keybindings.submit, "enter");
    assert_eq!(config.keybindings.abort, "escape");
    assert_eq!(config.keybindings.new_line, "alt+enter");
}

#[test]
fn keybindings_partial_override() {
    let toml = r#"
[keybindings]
submit = "ctrl+s"
"#;
    let f = write_toml(toml);
    let config = load_config_file(f.path()).unwrap();
    assert_eq!(config.keybindings.submit, "ctrl+s");
    // Non-overridden fields keep defaults
    assert_eq!(config.keybindings.abort, "escape");
    assert_eq!(config.keybindings.new_line, "alt+enter");
}

#[test]
fn keybindings_case_insensitive() {
    let toml = r#"
[keybindings]
submit = "Enter"
abort = "ESCAPE"
"#;
    let f = write_toml(toml);
    let config = load_config_file(f.path()).unwrap();
    assert_eq!(config.keybindings.submit, "Enter");
    assert_eq!(config.keybindings.abort, "ESCAPE");
}

#[test]
fn nonexistent_file_gives_default_keybindings() {
    let config = load_config_file(std::path::Path::new("/nonexistent/opi/config.toml")).unwrap();
    assert_eq!(config.keybindings.submit, "enter");
    assert_eq!(config.keybindings.abort, "escape");
    assert_eq!(config.keybindings.new_line, "alt+enter");
}
