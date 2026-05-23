//! Tests for provider factory construction across all 6 providers.
//!
//! Each test constructs a provider with a dummy API key and verifies the
//! provider reports the correct ID. Config integration tests verify that
//! TOML-deserialized provider configs resolve to the right env var names.

use opi_ai::provider::Provider;
use opi_coding_agent::config::{
    GenericProviderConfig, OpenRouterProviderConfig, OpiConfig, load_config_file,
};

// ---------------------------------------------------------------------------
// Provider construction: correct id() per provider
// ---------------------------------------------------------------------------

#[test]
fn anthropic_provider_construction() {
    let provider = opi_ai::anthropic::AnthropicProvider::new("test-key".into(), None);
    assert_eq!(provider.id(), "anthropic");
}

#[test]
fn openai_provider_construction() {
    let provider = opi_ai::openai_chat::OpenAiChatProvider::new("test-key".into(), None);
    assert_eq!(provider.id(), "openai");
}

#[test]
fn openrouter_provider_construction() {
    let provider = opi_ai::openrouter::openrouter_provider("test-key".into(), None);
    assert_eq!(provider.id(), "openrouter");
}

#[test]
fn mistral_provider_construction() {
    let provider = opi_ai::mistral::mistral_provider("test-key".into(), None);
    assert_eq!(provider.id(), "mistral");
}

#[test]
fn openai_responses_provider_construction() {
    let provider = opi_ai::openai_responses::OpenAiResponsesProvider::new("test-key".into(), None);
    assert_eq!(provider.id(), "openai-responses");
}

#[test]
fn gemini_provider_construction() {
    let provider = opi_ai::gemini::GeminiProvider::new("test-key".into(), None);
    assert_eq!(provider.id(), "gemini");
}

// ---------------------------------------------------------------------------
// OpenRouter with custom referer header
// ---------------------------------------------------------------------------

#[test]
fn openrouter_with_custom_referer() {
    let compat = opi_ai::openai_chat::CompatConfig::default();
    // Get the default model list from the convenience function.
    let temp = opi_ai::openrouter::openrouter_provider(String::new(), None);
    let models = temp.models().to_vec();
    let provider = opi_ai::openai_chat::OpenAiChatProvider::new_for_profile(
        "test-key".into(),
        "https://openrouter.ai/api".into(),
        "openrouter".into(),
        compat,
        vec![
            ("HTTP-Referer".into(), "https://custom.example.com".into()),
            ("X-Title".into(), "opi".into()),
        ],
        models,
    );
    assert_eq!(provider.id(), "openrouter");
}

// ---------------------------------------------------------------------------
// Defaults config: provider structs
// ---------------------------------------------------------------------------

#[test]
fn generic_provider_default_has_empty_env() {
    let cfg = GenericProviderConfig::default();
    assert!(cfg.api_key_env.is_empty());
    assert!(cfg.base_url.is_none());
}

#[test]
fn openrouter_provider_default_has_empty_env() {
    let cfg = OpenRouterProviderConfig::default();
    assert!(cfg.api_key_env.is_empty());
    assert!(cfg.base_url.is_none());
    assert!(cfg.referer.is_none());
}

#[test]
fn opi_config_default_anthropic_env() {
    let config = OpiConfig::default();
    assert_eq!(config.providers.anthropic.api_key_env, "ANTHROPIC_API_KEY");
}

// ---------------------------------------------------------------------------
// TOML deserialization: all provider sections
// ---------------------------------------------------------------------------

#[test]
fn toml_parses_openai_provider() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[providers.openai]
api_key_env = "MY_OPENAI_KEY"
base_url = "https://custom.openai.example.com"
"#,
    )
    .unwrap();
    let config = load_config_file(&path).unwrap();
    assert_eq!(config.providers.openai.api_key_env, "MY_OPENAI_KEY");
    assert_eq!(
        config.providers.openai.base_url.as_deref(),
        Some("https://custom.openai.example.com")
    );
}

#[test]
fn toml_parses_openrouter_provider() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[providers.openrouter]
api_key_env = "MY_OPENROUTER_KEY"
referer = "https://myapp.example.com"
"#,
    )
    .unwrap();
    let config = load_config_file(&path).unwrap();
    assert_eq!(config.providers.openrouter.api_key_env, "MY_OPENROUTER_KEY");
    assert_eq!(
        config.providers.openrouter.referer.as_deref(),
        Some("https://myapp.example.com")
    );
}

#[test]
fn toml_parses_mistral_provider() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[providers.mistral]
api_key_env = "MY_MISTRAL_KEY"
"#,
    )
    .unwrap();
    let config = load_config_file(&path).unwrap();
    assert_eq!(config.providers.mistral.api_key_env, "MY_MISTRAL_KEY");
}

#[test]
fn toml_parses_openai_responses_provider() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[providers.openai_responses]
api_key_env = "MY_OPENAI_KEY"
"#,
    )
    .unwrap();
    let config = load_config_file(&path).unwrap();
    assert_eq!(
        config.providers.openai_responses.api_key_env,
        "MY_OPENAI_KEY"
    );
}

#[test]
fn toml_parses_gemini_provider() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[providers.gemini]
api_key_env = "MY_GEMINI_KEY"
base_url = "https://custom-gemini.example.com"
"#,
    )
    .unwrap();
    let config = load_config_file(&path).unwrap();
    assert_eq!(config.providers.gemini.api_key_env, "MY_GEMINI_KEY");
    assert_eq!(
        config.providers.gemini.base_url.as_deref(),
        Some("https://custom-gemini.example.com")
    );
}

#[test]
fn toml_multiple_providers_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[providers.anthropic]
api_key_env = "KEY_A"

[providers.openai]
api_key_env = "KEY_O"

[providers.gemini]
api_key_env = "KEY_G"

[providers.mistral]
api_key_env = "KEY_M"

[providers.openrouter]
api_key_env = "KEY_OR"

[providers.openai_responses]
api_key_env = "KEY_OAR"
"#,
    )
    .unwrap();
    let config = load_config_file(&path).unwrap();
    assert_eq!(config.providers.anthropic.api_key_env, "KEY_A");
    assert_eq!(config.providers.openai.api_key_env, "KEY_O");
    assert_eq!(config.providers.gemini.api_key_env, "KEY_G");
    assert_eq!(config.providers.mistral.api_key_env, "KEY_M");
    assert_eq!(config.providers.openrouter.api_key_env, "KEY_OR");
    assert_eq!(config.providers.openai_responses.api_key_env, "KEY_OAR");
}
