//! Proxy config parsing tests (task 3.12).
//!
//! Verifies `[providers.*.proxy]` TOML config parsing and proxy config
//! flowing through config resolution. No live network dependency.

use std::fs;

use opi_coding_agent::config::{OpiConfig, load_config_file};

fn write_temp_config(dir: &std::path::Path, contents: &str) -> std::path::PathBuf {
    let path = dir.join("config.toml");
    fs::write(&path, contents).unwrap();
    path
}

// ---------------------------------------------------------------------------
// No proxy config → None
// ---------------------------------------------------------------------------

#[test]
fn no_proxy_config_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
"#,
    );
    let config = load_config_file(&path).unwrap();
    assert!(
        config.providers.anthropic.proxy.is_none(),
        "no proxy config should be None"
    );
}

#[test]
fn empty_config_has_no_proxy() {
    let config = OpiConfig::default();
    assert!(config.providers.anthropic.proxy.is_none());
    assert!(config.providers.openai.proxy.is_none());
    assert!(config.providers.openrouter.proxy.is_none());
    assert!(config.providers.gemini.proxy.is_none());
}

// ---------------------------------------------------------------------------
// Parse proxy config for Anthropic provider
// ---------------------------------------------------------------------------

#[test]
fn parse_anthropic_proxy_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

[providers.anthropic.proxy]
url = "http://proxy.example.com:8080"
"#,
    );
    let config = load_config_file(&path).unwrap();
    let proxy = config
        .providers
        .anthropic
        .proxy
        .as_ref()
        .expect("proxy should be set");
    assert_eq!(proxy.url, "http://proxy.example.com:8080");
    assert!(proxy.no_proxy.is_none());
}

#[test]
fn parse_anthropic_proxy_with_no_proxy() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

[providers.anthropic.proxy]
url = "http://proxy.example.com:8080"
no_proxy = "localhost,*.internal"
"#,
    );
    let config = load_config_file(&path).unwrap();
    let proxy = config
        .providers
        .anthropic
        .proxy
        .as_ref()
        .expect("proxy should be set");
    assert_eq!(proxy.url, "http://proxy.example.com:8080");
    assert_eq!(proxy.no_proxy.as_deref(), Some("localhost,*.internal"));
}

// ---------------------------------------------------------------------------
// Parse proxy config for OpenAI provider
// ---------------------------------------------------------------------------

#[test]
fn parse_openai_proxy_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.openai]
api_key_env = "OPENAI_API_KEY"

[providers.openai.proxy]
url = "http://proxy.example.com:8080"
"#,
    );
    let config = load_config_file(&path).unwrap();
    let proxy = config
        .providers
        .openai
        .proxy
        .as_ref()
        .expect("proxy should be set");
    assert_eq!(proxy.url, "http://proxy.example.com:8080");
}

// ---------------------------------------------------------------------------
// Parse proxy config for Gemini provider
// ---------------------------------------------------------------------------

#[test]
fn parse_gemini_proxy_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.gemini]
api_key_env = "GEMINI_API_KEY"

[providers.gemini.proxy]
url = "http://proxy.example.com:8080"
no_proxy = "localhost"
"#,
    );
    let config = load_config_file(&path).unwrap();
    let proxy = config
        .providers
        .gemini
        .proxy
        .as_ref()
        .expect("proxy should be set");
    assert_eq!(proxy.url, "http://proxy.example.com:8080");
    assert_eq!(proxy.no_proxy.as_deref(), Some("localhost"));
}

// ---------------------------------------------------------------------------
// Parse proxy config for OpenRouter provider
// ---------------------------------------------------------------------------

#[test]
fn parse_openrouter_proxy_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.openrouter]
api_key_env = "OPENROUTER_API_KEY"

[providers.openrouter.proxy]
url = "http://proxy.example.com:8080"
"#,
    );
    let config = load_config_file(&path).unwrap();
    let proxy = config
        .providers
        .openrouter
        .proxy
        .as_ref()
        .expect("proxy should be set");
    assert_eq!(proxy.url, "http://proxy.example.com:8080");
}

// ---------------------------------------------------------------------------
// Parse proxy config for Mistral provider
// ---------------------------------------------------------------------------

#[test]
fn parse_mistral_proxy_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.mistral]
api_key_env = "MISTRAL_API_KEY"

[providers.mistral.proxy]
url = "http://proxy.example.com:8080"
"#,
    );
    let config = load_config_file(&path).unwrap();
    let proxy = config
        .providers
        .mistral
        .proxy
        .as_ref()
        .expect("proxy should be set");
    assert_eq!(proxy.url, "http://proxy.example.com:8080");
}

// ---------------------------------------------------------------------------
// Parse proxy config for OpenAI Responses provider
// ---------------------------------------------------------------------------

#[test]
fn parse_openai_responses_proxy_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.openai_responses]
api_key_env = "OPENAI_API_KEY"

[providers.openai_responses.proxy]
url = "http://proxy.example.com:8080"
"#,
    );
    let config = load_config_file(&path).unwrap();
    let proxy = config
        .providers
        .openai_responses
        .proxy
        .as_ref()
        .expect("proxy should be set");
    assert_eq!(proxy.url, "http://proxy.example.com:8080");
}

// ---------------------------------------------------------------------------
// Multiple providers with different proxy configs
// ---------------------------------------------------------------------------

#[test]
fn different_proxy_per_provider() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

[providers.anthropic.proxy]
url = "http://anthropic-proxy:8080"

[providers.openai]
api_key_env = "OPENAI_API_KEY"

[providers.openai.proxy]
url = "http://openai-proxy:9090"
no_proxy = "localhost"
"#,
    );
    let config = load_config_file(&path).unwrap();
    let ap = config
        .providers
        .anthropic
        .proxy
        .as_ref()
        .expect("anthropic proxy");
    assert_eq!(ap.url, "http://anthropic-proxy:8080");
    assert!(ap.no_proxy.is_none());

    let op = config
        .providers
        .openai
        .proxy
        .as_ref()
        .expect("openai proxy");
    assert_eq!(op.url, "http://openai-proxy:9090");
    assert_eq!(op.no_proxy.as_deref(), Some("localhost"));

    // Gemini has no proxy configured
    assert!(config.providers.gemini.proxy.is_none());
}

// ---------------------------------------------------------------------------
// Empty proxy section without url is ignored
// ---------------------------------------------------------------------------

#[test]
fn empty_proxy_section_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_temp_config(
        dir.path(),
        r#"
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

[providers.anthropic.proxy]
no_proxy = "localhost"
"#,
    );
    let config = load_config_file(&path).unwrap();
    assert!(
        config.providers.anthropic.proxy.is_none(),
        "proxy section without url should be ignored"
    );
}
