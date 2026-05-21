//! Behavioral tests for the provider registry (task 1.4).
//!
//! DoD: "resolves anthropic:model and capabilities"

use opi_ai::anthropic::AnthropicProvider;
use opi_ai::provider::{EventStream, ModelInfo, Provider, Request};
use opi_ai::registry::{ProviderRegistry, RegistryError};

/// Minimal stub provider for multi-provider tests.
struct StubProvider {
    id: String,
    models: Vec<ModelInfo>,
}

impl StubProvider {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            models: vec![ModelInfo {
                id: format!("{id}-model-1"),
                display_name: format!("{id} Model 1"),
                context_window: 128000,
                max_output_tokens: 4096,
                supports_streaming: true,
                supports_thinking: false,
            }],
        }
    }
}

impl Provider for StubProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn models(&self) -> &[ModelInfo] {
        &self.models
    }
    fn stream(&self, _request: Request) -> EventStream {
        Box::pin(futures_util::stream::empty())
    }
}

/// Helper: create a registry with the Anthropic provider registered.
fn anthropic_registry() -> ProviderRegistry {
    let mut reg = ProviderRegistry::new();
    reg.register(Box::new(AnthropicProvider::new("test-key".into(), None)));
    reg
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[test]
fn register_and_list_providers() {
    let reg = anthropic_registry();
    let ids = reg.provider_ids();
    assert_eq!(ids, vec!["anthropic"]);
}

#[test]
fn register_multiple_providers() {
    let mut reg = ProviderRegistry::new();
    reg.register(Box::new(StubProvider::new("alpha")));
    reg.register(Box::new(StubProvider::new("beta")));
    let mut ids = reg.provider_ids();
    ids.sort();
    assert_eq!(ids, vec!["alpha", "beta"]);
}

#[test]
fn duplicate_registration_replaces() {
    let mut reg = ProviderRegistry::new();
    reg.register(Box::new(StubProvider::new("p")));
    reg.register(Box::new(StubProvider::new("p")));
    assert_eq!(reg.provider_ids().len(), 1);
}

// ---------------------------------------------------------------------------
// Resolve: provider:model → (Provider, ModelInfo)
// ---------------------------------------------------------------------------

#[test]
fn resolve_anthropic_model() {
    let reg = anthropic_registry();
    let (provider, model) = reg.resolve("anthropic:claude-sonnet-4-5-20250514").unwrap();
    assert_eq!(provider.id(), "anthropic");
    assert_eq!(model.id, "claude-sonnet-4-5-20250514");
    assert!(model.supports_streaming);
}

#[test]
fn resolve_returns_capabilities() {
    let reg = anthropic_registry();
    let caps = reg
        .capabilities("anthropic:claude-sonnet-4-5-20250514")
        .unwrap();
    assert!(caps.supports_streaming);
    assert!(caps.supports_thinking);
}

#[test]
fn resolve_unknown_provider() {
    let reg = anthropic_registry();
    let result = reg.resolve("openai:gpt-4o");
    assert!(matches!(result, Err(RegistryError::UnknownProvider(ref s)) if s == "openai"));
}

#[test]
fn resolve_unknown_model() {
    let reg = anthropic_registry();
    let result = reg.resolve("anthropic:nonexistent-model");
    assert!(
        matches!(result, Err(RegistryError::UnknownModel { ref provider, .. }) if provider == "anthropic")
    );
}

#[test]
fn resolve_missing_colon() {
    let reg = anthropic_registry();
    let result = reg.resolve("just-a-string");
    assert!(matches!(result, Err(RegistryError::InvalidSpec(_))));
}

#[test]
fn resolve_empty_spec() {
    let reg = anthropic_registry();
    let result = reg.resolve("");
    assert!(matches!(result, Err(RegistryError::InvalidSpec(_))));
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

#[test]
fn capabilities_context_window() {
    let reg = anthropic_registry();
    let caps = reg
        .capabilities("anthropic:claude-sonnet-4-5-20250514")
        .unwrap();
    assert_eq!(caps.context_window, 200000);
    assert_eq!(caps.max_output_tokens, 8192);
}

#[test]
fn capabilities_unknown_provider() {
    let reg = anthropic_registry();
    let result = reg.capabilities("google:gemini-pro");
    assert!(matches!(result, Err(RegistryError::UnknownProvider(_))));
}

// ---------------------------------------------------------------------------
// Resolve partial model name (exact match required)
// ---------------------------------------------------------------------------

#[test]
fn resolve_requires_exact_model_id() {
    let reg = anthropic_registry();
    let result = reg.resolve("anthropic:claude-sonnet-4");
    assert!(matches!(result, Err(RegistryError::UnknownModel { .. })));
}

// ---------------------------------------------------------------------------
// Provider access
// ---------------------------------------------------------------------------

#[test]
fn get_provider_by_id() {
    let reg = anthropic_registry();
    let provider = reg.get_provider("anthropic").unwrap();
    assert_eq!(provider.id(), "anthropic");
    assert!(!provider.models().is_empty());
}

#[test]
fn get_provider_unknown() {
    let reg = anthropic_registry();
    assert!(reg.get_provider("nonexistent").is_none());
}
