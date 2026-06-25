//! Provider collection/auth seam (Workstream 10.1).
//!
//! [`ProviderCollection`] is the higher-level facade above [`ProviderRegistry`]
//! that owns provider and model lookup, the provider-side auth contract,
//! OpenAI-compatible compatibility metadata, stream/complete dispatch, and
//! redacted missing/invalid auth diagnostics. It wraps a registry (D1) rather
//! than replacing it, so existing provider paths and the documented unstable
//! registration API keep working.
//!
//! # Auth resolution timing (D2)
//!
//! Auth descriptors are resolved at dispatch time and the resulting status is
//! snapshot for that dispatch. The collection deliberately has no notion of a
//! run or a turn — those concepts belong to the generic harness (Workstream
//! 10.2) and must not leak into `opi-ai`.
//!
//! # Complete-dispatch decision
//!
//! The current [`Provider`] trait is streaming-only.
//! Rather than adding a second trait method (which would touch every provider
//! adapter), complete dispatch is implemented by draining the stream returned
//! by [`Provider::stream`] to its terminal event. This keeps the decision
//! compatible with the existing streaming contract.
//!
//! # Future OAuth (not implemented)
//!
//! [`AuthDescriptor`] is `#[non_exhaustive]` so a future OAuth variant can be
//! added without redesigning provider construction (Workstream 10.1 SC4).
//! Phase 10 implements no OAuth login and no subscription auth.
//!
//! # Unstable
//!
//! This surface is part of the **unstable 0.x extension substrate**. Breaking
//! changes may occur between minor versions without a major version bump.

use std::collections::HashMap;

use futures_util::StreamExt;

use crate::message::AssistantMessage;
use crate::provider::{EventStream, ModelInfo, Provider, ProviderError, Request};
use crate::registry::{ModelCapabilities, ProviderRegistry, RegistrationError, RegistryError};
use crate::stream::{AssistantStreamEvent, StopReason};

// ---------------------------------------------------------------------------
// SecretKey — redacted credential value
// ---------------------------------------------------------------------------

/// An API key value that never reveals itself in debug/display output.
///
/// The collection stores credentials so it can report auth status and feed
/// diagnostics, but the value is always rendered as `<redacted>` when
/// formatted. Callers that need the raw value use [`SecretKey::as_str`].
#[derive(Clone)]
pub struct SecretKey(String);

impl SecretKey {
    /// Wrap a raw credential value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Access the raw value programmatically.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the key is non-empty.
    pub fn is_present(&self) -> bool {
        !self.0.is_empty()
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

impl std::fmt::Display for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

// ---------------------------------------------------------------------------
// Auth contract
// ---------------------------------------------------------------------------

/// Provider-owned auth contract.
///
/// Describes how a provider's credential is sourced without leaking the secret
/// itself. Two concrete variants cover the current built-in providers (static
/// API keys and env-described API keys). OAuth is an explicit future extension
/// point and is not implemented in Phase 10.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AuthDescriptor {
    /// A static API key value held by the collection. The key itself is
    /// redacted in all diagnostics; only its presence or absence is reported.
    StaticApiKey {
        /// Redacted at debug/display time; never surfaced in diagnostics.
        value: SecretKey,
    },
    /// An API key resolved from an environment variable at dispatch time.
    EnvApiKey {
        /// Name of the environment variable (e.g. `ANTHROPIC_API_KEY`).
        env_var: String,
    },
}

impl AuthDescriptor {
    /// Resolve the descriptor to a redacted [`AuthStatus`] at dispatch time.
    ///
    /// The returned `source` text names the reason (for example the env var
    /// name) but never contains a credential value.
    pub fn resolve(&self) -> AuthStatus {
        match self {
            AuthDescriptor::StaticApiKey { value } => {
                if value.is_present() {
                    AuthStatus::Configured
                } else {
                    AuthStatus::Missing {
                        source: "static api key is empty".to_owned(),
                    }
                }
            }
            AuthDescriptor::EnvApiKey { env_var } => match std::env::var(env_var) {
                Ok(value) if !value.is_empty() => AuthStatus::Configured,
                _ => AuthStatus::Missing {
                    source: format!("env var {env_var} is not set"),
                },
            },
        }
    }
}

/// Resolution of an [`AuthDescriptor`] at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStatus {
    /// Credential is present and non-empty.
    Configured,
    /// No credential found. `source` names the origin (e.g. env var name)
    /// without leaking any value.
    Missing { source: String },
}

// ---------------------------------------------------------------------------
// OpenAI-compatible compatibility metadata
// ---------------------------------------------------------------------------

/// Collection-level home for OpenAI-compatible profile flags.
///
/// Workstream 10.1 decision: profile compatibility flags live alongside model
/// metadata in the collection instead of being scattered across factory call
/// sites (SC3).
#[derive(Debug, Clone, Default)]
pub struct CompatMetadata {
    /// Whether the provider speaks an OpenAI-compatible Chat Completions API.
    pub openai_compatible: bool,
    /// Free-form profile label (e.g. `"openrouter"`, `"mistral"`) for
    /// diagnostics.
    pub profile: Option<String>,
}

// ---------------------------------------------------------------------------
// Completion result
// ---------------------------------------------------------------------------

/// Result of draining a provider stream to completion.
///
/// See the [module docs](self) for the complete-dispatch decision.
#[derive(Debug, Clone)]
pub enum CompletedRequest {
    /// Stream terminated with [`AssistantStreamEvent::Done`].
    Done {
        reason: StopReason,
        message: AssistantMessage,
    },
    /// Stream terminated with [`AssistantStreamEvent::Error`].
    Error {
        reason: StopReason,
        message: AssistantMessage,
    },
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error type for provider collection operations.
#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    /// A registry lookup failed.
    #[error(transparent)]
    Registry(#[from] RegistryError),
    /// Dispatch was rejected because auth is not configured for the provider.
    ///
    /// `source` is redacted and never carries a credential value.
    #[error("auth not configured for provider '{provider}': {detail}")]
    AuthNotConfigured {
        /// Provider id whose auth is missing.
        provider: String,
        /// Redacted description of the missing auth source.
        detail: String,
    },
    /// A provider stream failed while draining to completion.
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

// ---------------------------------------------------------------------------
// ProviderCollection
// ---------------------------------------------------------------------------

/// A collection of providers/models that owns provider+model lookup, auth
/// resolution, compatibility metadata, and stream/complete dispatch.
///
/// Wraps a [`ProviderRegistry`] (D1) and layers the auth/collection contract
/// on top so existing provider paths keep working.
pub struct ProviderCollection {
    registry: ProviderRegistry,
    auth: HashMap<String, AuthDescriptor>,
    compat: HashMap<String, CompatMetadata>,
}

impl ProviderCollection {
    /// Construct an empty collection.
    pub fn new() -> Self {
        Self {
            registry: ProviderRegistry::new(),
            auth: HashMap::new(),
            compat: HashMap::new(),
        }
    }

    /// Wrap an existing registry. Used by the coding-agent provider factory
    /// (Workstream 10.2) to layer collection semantics onto providers it
    /// constructs from config/env/package inputs.
    ///
    /// Pre-registered providers have no auth descriptor until one is attached,
    /// so [`ProviderCollection::auth_status`] returns `None` for them and
    /// dispatch is not auth-gated.
    pub fn from_registry(registry: ProviderRegistry) -> Self {
        Self {
            registry,
            auth: HashMap::new(),
            compat: HashMap::new(),
        }
    }

    /// Register a provider with its auth descriptor and compatibility metadata.
    ///
    /// Replaces any existing entry with the same provider id.
    ///
    /// # Errors
    ///
    /// Propagates [`RegistrationError::EmptyProviderId`] from the registry.
    pub fn register(
        &mut self,
        provider: Box<dyn Provider>,
        auth: AuthDescriptor,
        compat: CompatMetadata,
    ) -> Result<(), RegistrationError> {
        let id = provider.id().to_owned();
        self.registry.register_provider(provider)?;
        self.auth.insert(id.clone(), auth);
        self.compat.insert(id, compat);
        Ok(())
    }

    /// Access the underlying registry (for `--list-models`, overrides, etc.).
    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    /// Return sorted registered provider ids.
    pub fn provider_ids(&self) -> Vec<&str> {
        self.registry.provider_ids()
    }

    /// Resolve a `provider:model` spec into provider reference + model info.
    pub fn resolve(&self, spec: &str) -> Result<(&dyn Provider, &ModelInfo), RegistryError> {
        self.registry.resolve(spec)
    }

    /// Query capabilities for a `provider:model` spec.
    pub fn capabilities(&self, spec: &str) -> Result<ModelCapabilities, RegistryError> {
        self.registry.capabilities(spec)
    }

    /// The auth descriptor associated with a provider, if any.
    pub fn auth_descriptor(&self, provider_id: &str) -> Option<&AuthDescriptor> {
        self.auth.get(provider_id)
    }

    /// Resolve the current redacted auth status for a provider, if the
    /// collection owns an auth descriptor for it.
    pub fn auth_status(&self, provider_id: &str) -> Option<AuthStatus> {
        self.auth.get(provider_id).map(AuthDescriptor::resolve)
    }

    /// The compatibility metadata associated with a provider, if any.
    pub fn compat(&self, provider_id: &str) -> Option<&CompatMetadata> {
        self.compat.get(provider_id)
    }

    /// Resolve a spec, validate its auth, and return a provider stream.
    ///
    /// Dispatch is auth-gated only for providers the collection owns an auth
    /// descriptor for. A [`AuthStatus::Missing`] descriptor yields a redacted
    /// [`CollectionError::AuthNotConfigured`] before the provider is touched.
    pub fn dispatch_stream(
        &self,
        spec: &str,
        request: Request,
    ) -> Result<EventStream, CollectionError> {
        let (provider, _) = self.registry.resolve(spec)?;
        if let Some(AuthStatus::Missing { source }) = self.auth_status(provider.id()) {
            return Err(CollectionError::AuthNotConfigured {
                provider: provider.id().to_owned(),
                detail: source,
            });
        }
        Ok(provider.stream(request))
    }

    /// Drain a provider stream to its terminal event.
    ///
    /// This is the explicit complete-dispatch decision: complete dispatch is
    /// built on top of the streaming [`Provider`] trait rather than a separate
    /// trait method. See the [module docs](self).
    ///
    /// Auth gating is identical to [`ProviderCollection::dispatch_stream`].
    pub async fn dispatch_complete(
        &self,
        spec: &str,
        request: Request,
    ) -> Result<CompletedRequest, CollectionError> {
        let stream = self.dispatch_stream(spec, request)?;
        Ok(drain_to_completion(stream).await?)
    }

    /// Refresh provider-side state (model catalogs, rotated tokens).
    ///
    /// Phase 10 implements no refresh behavior; this is the documented
    /// extension point so providers that can refresh model lists or rotate
    /// credentials at run time can be added later without redesigning the
    /// collection contract.
    pub async fn refresh(&self) -> Result<(), CollectionError> {
        Ok(())
    }
}

impl Default for ProviderCollection {
    fn default() -> Self {
        Self::new()
    }
}

/// Drain an event stream until it yields a terminal event or errors.
///
/// A stream that ends without a terminal event is treated as a stream error.
async fn drain_to_completion(mut stream: EventStream) -> Result<CompletedRequest, ProviderError> {
    while let Some(item) = stream.next().await {
        match item {
            Ok(AssistantStreamEvent::Done { reason, message }) => {
                return Ok(CompletedRequest::Done { reason, message });
            }
            Ok(AssistantStreamEvent::Error { reason, message }) => {
                return Ok(CompletedRequest::Error { reason, message });
            }
            Ok(_) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(ProviderError::StreamError(
        "stream ended without a terminal event".to_owned(),
    ))
}
