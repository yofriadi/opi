//! Behavioral tests for the provider collection/auth seam (task 10.1).
//!
//! DoD: opi-ai exposes a provider collection/auth contract that owns provider
//! and model lookup, static API-key and env-auth descriptors, optional refresh
//! capability, OpenAI-compatible compatibility metadata, stream dispatch, an
//! explicit complete-dispatch decision compatible with the current streaming
//! Provider trait, redacted missing/invalid auth diagnostics, and a registry
//! regression asserting all built-in providers still resolve.

use opi_ai::message::{AssistantContent, AssistantMessage};
use opi_ai::provider::{Provider, Request, ThinkingConfig};
use opi_ai::provider_collection::{
    AuthDescriptor, AuthStatus, CollectionError, CompatMetadata, CompletedRequest,
    ProviderCollection, SecretKey,
};
use opi_ai::registry::ProviderRegistry;
use opi_ai::test_support::{MockProvider, text_response};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Request/message helpers
// ---------------------------------------------------------------------------

fn minimal_request(model: &str) -> Request {
    Request {
        model: model.into(),
        system: None,
        messages: vec![],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: ThinkingConfig::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: CancellationToken::new(),
    }
}

/// Concatenate all text content carried by an assistant message.
fn assistant_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|content| match content {
            AssistantContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Build a mock provider that streams a single text response.
fn text_mock(id: &str, text: &str) -> Box<dyn Provider> {
    Box::new(MockProvider::new(id, vec![text_response(text)]))
}

/// Build a mock provider with `count` identical text response batches, for
/// tests that dispatch more than once.
fn text_mock_repeated(id: &str, text: &str, count: usize) -> Box<dyn Provider> {
    let responses = (0..count).map(|_| text_response(text)).collect();
    Box::new(MockProvider::new(id, responses))
}

const SECRET_VALUE: &str = "sk-super-secret-value-DO-NOT-LEAK";

// ---------------------------------------------------------------------------
// SecretKey redaction
// ---------------------------------------------------------------------------

#[test]
fn secret_key_redacts_in_debug_and_display() {
    let key = SecretKey::new(SECRET_VALUE);
    let debug = format!("{key:?}");
    let display = format!("{key}");
    assert_eq!(debug, "<redacted>");
    assert_eq!(display, "<redacted>");
    assert!(!debug.contains(SECRET_VALUE));
    assert!(!display.contains(SECRET_VALUE));
    // The value is still accessible programmatically by callers that need it.
    assert_eq!(key.as_str(), SECRET_VALUE);
    assert!(key.is_present());
}

#[test]
fn secret_key_empty_is_not_present() {
    let key = SecretKey::new("");
    assert!(!key.is_present());
}

// ---------------------------------------------------------------------------
// AuthDescriptor resolution (redacted, no provider needed)
// ---------------------------------------------------------------------------

#[test]
fn auth_descriptor_static_key_resolves_configured_and_missing() {
    let configured = AuthDescriptor::StaticApiKey {
        value: SecretKey::new(SECRET_VALUE),
    };
    assert_eq!(configured.resolve(), AuthStatus::Configured);

    let missing = AuthDescriptor::StaticApiKey {
        value: SecretKey::new(""),
    };
    match missing.resolve() {
        AuthStatus::Missing { source } => {
            // Source names the reason but never leaks a value.
            assert!(!source.contains(SECRET_VALUE));
        }
        other => panic!("expected Missing, got {other:?}"),
    }
}

#[test]
fn auth_descriptor_env_key_missing_when_var_unset() {
    // Read-only: relies on the var being unset. Unique name avoids collisions.
    let descriptor = AuthDescriptor::EnvApiKey {
        env_var: "OPI_TEST_PROV_COLL_DEFINITELY_UNSET_9F2A7C".into(),
    };
    match descriptor.resolve() {
        AuthStatus::Missing { source } => {
            assert!(source.contains("OPI_TEST_PROV_COLL_DEFINITELY_UNSET_9F2A7C"));
            assert!(!source.contains(SECRET_VALUE));
        }
        AuthStatus::Configured => panic!("expected Missing for unset env var"),
    }
}

// ---------------------------------------------------------------------------
// Acceptance scenario: provider_collection_dispatches_with_redacted_auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provider_collection_dispatches_with_redacted_auth() {
    let mut collection = ProviderCollection::new();
    collection
        .register(
            text_mock_repeated("mock", "hello from mock", 2),
            AuthDescriptor::StaticApiKey {
                value: SecretKey::new(SECRET_VALUE),
            },
            CompatMetadata::default(),
        )
        .unwrap();

    // Auth status is Configured and the descriptor never leaks the secret.
    assert_eq!(collection.auth_status("mock"), Some(AuthStatus::Configured));
    let descriptor_debug = format!("{:?}", collection.auth_descriptor("mock").unwrap());
    assert!(!descriptor_debug.contains(SECRET_VALUE));

    // Stream dispatch flows through the collection and reaches Done.
    let stream = collection
        .dispatch_stream("mock:mock-model", minimal_request("mock:mock-model"))
        .unwrap();
    use futures_util::StreamExt;
    let events: Vec<_> = stream.collect::<Vec<_>>().await;
    let done_message = events
        .into_iter()
        .filter_map(|event| match event.unwrap() {
            opi_ai::AssistantStreamEvent::Done { message, .. } => Some(message),
            _ => None,
        })
        .next()
        .expect("stream produced a Done event");
    assert_eq!(assistant_text(&done_message), "hello from mock");

    // Complete-dispatch decision: drain the streaming trait to a terminal.
    let completed = collection
        .dispatch_complete("mock:mock-model", minimal_request("mock:mock-model"))
        .await
        .unwrap();
    match completed {
        CompletedRequest::Done { message, .. } => {
            assert_eq!(assistant_text(&message), "hello from mock");
        }
        other => panic!("expected CompletedRequest::Done, got {other:?}"),
    }
}

#[tokio::test]
async fn provider_collection_dispatch_rejects_missing_auth_with_redacted_diagnostic() {
    let mut collection = ProviderCollection::new();
    collection
        .register(
            text_mock("noauth", "should not stream"),
            AuthDescriptor::StaticApiKey {
                value: SecretKey::new(""),
            },
            CompatMetadata::default(),
        )
        .unwrap();

    assert!(matches!(
        collection.auth_status("noauth"),
        Some(AuthStatus::Missing { .. })
    ));

    let err = match collection
        .dispatch_stream("noauth:mock-model", minimal_request("noauth:mock-model"))
    {
        Err(error) => error,
        Ok(_) => panic!("expected AuthNotConfigured error, got a stream"),
    };
    match err {
        CollectionError::AuthNotConfigured {
            ref provider,
            ref detail,
        } => {
            assert_eq!(provider.as_str(), "noauth");
            // Diagnostic is redacted: it never carries the secret value.
            assert!(!detail.contains(SECRET_VALUE));
            assert!(!format!("{err}").contains(SECRET_VALUE));
        }
        other => panic!("expected AuthNotConfigured, got {other:?}"),
    }

    // Complete-dispatch also rejects before touching the provider.
    let complete_err = collection
        .dispatch_complete("noauth:mock-model", minimal_request("noauth:mock-model"))
        .await
        .unwrap_err();
    assert!(matches!(
        complete_err,
        CollectionError::AuthNotConfigured { .. }
    ));
}

// ---------------------------------------------------------------------------
// Acceptance scenario: collection_supports_provider_correctness_fixtures
// ---------------------------------------------------------------------------

#[tokio::test]
async fn collection_supports_provider_correctness_fixtures() {
    use opi_ai::provider::ModelInfo;

    // An OpenAI-compatible profile provider, as Phase 12 fixtures will exercise.
    let profile_model = ModelInfo {
        id: "profile-model".into(),
        display_name: "Profile Model".into(),
        context_window: 128_000,
        max_output_tokens: 4_096,
        supports_images: true,
        supports_streaming: true,
        supports_thinking: false,
    };
    let profile_provider = Box::new(MockProvider::new_with_models(
        "openrouter-profile",
        vec![profile_model],
        vec![text_response("profile response")],
    ));

    let mut collection = ProviderCollection::new();
    collection
        .register(
            profile_provider,
            AuthDescriptor::StaticApiKey {
                value: SecretKey::new(SECRET_VALUE),
            },
            CompatMetadata {
                openai_compatible: true,
                profile: Some("openrouter".into()),
            },
        )
        .unwrap();

    // Model lookup through the collection (no CLI harness constructed).
    let (resolved, model) = collection
        .resolve("openrouter-profile:profile-model")
        .unwrap();
    assert_eq!(resolved.id(), "openrouter-profile");
    assert_eq!(model.id, "profile-model");

    let caps = collection
        .capabilities("openrouter-profile:profile-model")
        .unwrap();
    assert_eq!(caps.context_window, 128_000);
    assert!(caps.supports_images);

    // Compatibility metadata has a home on the collection.
    let compat = collection.compat("openrouter-profile").unwrap();
    assert!(compat.openai_compatible);
    assert_eq!(compat.profile.as_deref(), Some("openrouter"));

    // Auth diagnostics resolve through the collection.
    let status = collection.auth_status("openrouter-profile").unwrap();
    assert_eq!(status, AuthStatus::Configured);
    let descriptor_debug = format!(
        "{:?}",
        collection.auth_descriptor("openrouter-profile").unwrap()
    );
    assert!(!descriptor_debug.contains(SECRET_VALUE));

    // Stream dispatch works without the CLI product harness.
    let completed = collection
        .dispatch_complete(
            "openrouter-profile:profile-model",
            minimal_request("openrouter-profile:profile-model"),
        )
        .await
        .unwrap();
    match completed {
        CompletedRequest::Done { message, .. } => {
            assert_eq!(assistant_text(&message), "profile response");
        }
        other => panic!("expected CompletedRequest::Done, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Optional refresh extension point (documented no-op in Phase 10)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn collection_refresh_is_a_documented_noop_extension_point() {
    let collection = ProviderCollection::new();
    let result = collection.refresh().await;
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// from_registry wrapping (for the coding-agent provider factory in 10.2)
// ---------------------------------------------------------------------------

#[test]
fn collection_wraps_existing_registry_via_from_registry() {
    let mut registry = ProviderRegistry::new();
    registry.register(text_mock("wrapped", "wrapped response"));

    let collection = ProviderCollection::from_registry(registry);
    // Underlying registry is accessible for list-models / overrides.
    assert_eq!(collection.registry().provider_ids(), vec!["wrapped"]);
    // Model lookup flows through the wrapped registry.
    let (provider, _) = collection.resolve("wrapped:mock-model").unwrap();
    assert_eq!(provider.id(), "wrapped");
    // Auth descriptor defaults to absent for pre-registered providers.
    assert!(collection.auth_descriptor("wrapped").is_none());
    assert!(collection.auth_status("wrapped").is_none());
}
