//! Provider registry — resolves `provider:model` specs to provider + model info.

use crate::provider::{ModelInfo, Provider};

/// Error type for registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("invalid model spec: {0}")]
    InvalidSpec(String),
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("unknown model '{model}' for provider '{provider}'")]
    UnknownModel { provider: String, model: String },
}

/// Capabilities of a resolved model.
#[derive(Debug, Clone, Copy)]
pub struct ModelCapabilities {
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub supports_streaming: bool,
    pub supports_thinking: bool,
}

/// Registry of available providers, keyed by provider id.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider. Replaces any existing provider with the same id.
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let id = provider.id().to_owned();
        self.providers.retain(|p| p.id() != id);
        self.providers.push(provider);
    }

    /// Return sorted list of registered provider ids.
    pub fn provider_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.providers.iter().map(|p| p.id()).collect();
        ids.sort();
        ids
    }

    /// Resolve a `provider:model` spec into provider reference + model info.
    pub fn resolve(&self, spec: &str) -> Result<(&dyn Provider, &ModelInfo), RegistryError> {
        let (provider_id, model_id) = split_spec(spec)?;
        let provider = self
            .providers
            .iter()
            .find(|p| p.id() == provider_id)
            .ok_or_else(|| RegistryError::UnknownProvider(provider_id.to_owned()))?;
        let model = provider
            .models()
            .iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| RegistryError::UnknownModel {
                provider: provider_id.to_owned(),
                model: model_id.to_owned(),
            })?;
        Ok((provider.as_ref(), model))
    }

    /// Query capabilities for a `provider:model` spec.
    pub fn capabilities(&self, spec: &str) -> Result<ModelCapabilities, RegistryError> {
        let (_, model) = self.resolve(spec)?;
        Ok(ModelCapabilities {
            context_window: model.context_window,
            max_output_tokens: model.max_output_tokens,
            supports_streaming: model.supports_streaming,
            supports_thinking: model.supports_thinking,
        })
    }

    /// Get a provider by id.
    pub fn get_provider(&self, id: &str) -> Option<&dyn Provider> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .map(|p| p.as_ref())
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Split a `provider:model` spec. Returns `InvalidSpec` if no colon or empty parts.
fn split_spec(spec: &str) -> Result<(&str, &str), RegistryError> {
    let Some((provider, model)) = spec.split_once(':') else {
        return Err(RegistryError::InvalidSpec(format!(
            "spec must be 'provider:model', got: {spec:?}"
        )));
    };
    if provider.is_empty() || model.is_empty() {
        return Err(RegistryError::InvalidSpec(format!(
            "spec must be 'provider:model', got: {spec:?}"
        )));
    }
    Ok((provider, model))
}
