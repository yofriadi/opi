//! Phase 7 task 7.2 — provider error diagnostic classification (opi-ai layer).
//!
//! `opi-ai` cannot depend on `opi-agent` (the dependency graph runs the other
//! way), so it cannot see the shared [`opi_agent::Diagnostic`] type. The
//! provider-side classification surface therefore lives here as a stable
//! taxonomy: [`ProviderError::category`] returns a [`ProviderErrorCategory`],
//! and `opi-agent` maps each category into the shared diagnostic vocabulary
//! (`code`/`severity`/`source`). These tests pin the taxonomy and its
//! consistency with retryability, with no network access and no provider
//! backend.

use opi_ai::provider::{ProviderError, ProviderErrorCategory};

// ---------------------------------------------------------------------------
// category(): each variant maps to a stable diagnostic category
// ---------------------------------------------------------------------------

#[test]
fn rate_limited_classifies_as_rate_limit() {
    assert_eq!(
        ProviderError::RateLimited {
            retry_after_ms: Some(5_000)
        }
        .category(),
        ProviderErrorCategory::RateLimit
    );
    assert_eq!(
        ProviderError::RateLimited {
            retry_after_ms: None
        }
        .category(),
        ProviderErrorCategory::RateLimit
    );
}

#[test]
fn timeout_classifies_as_timeout() {
    assert_eq!(
        ProviderError::Timeout.category(),
        ProviderErrorCategory::Timeout
    );
}

#[test]
fn request_failed_classifies_as_request() {
    assert_eq!(
        ProviderError::RequestFailed("internal server error".into()).category(),
        ProviderErrorCategory::Request
    );
}

#[test]
fn stream_error_classifies_as_stream() {
    assert_eq!(
        ProviderError::StreamError("connection reset".into()).category(),
        ProviderErrorCategory::Stream
    );
}

#[test]
fn auth_failed_classifies_as_auth() {
    assert_eq!(
        ProviderError::AuthFailed("invalid api key".into()).category(),
        ProviderErrorCategory::Auth
    );
}

// ---------------------------------------------------------------------------
// retry_after_ms(): only RateLimited carries a server-advised delay
// ---------------------------------------------------------------------------

#[test]
fn rate_limited_exposes_retry_after_ms() {
    assert_eq!(
        ProviderError::RateLimited {
            retry_after_ms: Some(7_500)
        }
        .retry_after_ms(),
        Some(7_500)
    );
    assert_eq!(
        ProviderError::RateLimited {
            retry_after_ms: None
        }
        .retry_after_ms(),
        None
    );
}

#[test]
fn non_rate_limit_errors_have_no_retry_after() {
    assert_eq!(ProviderError::Timeout.retry_after_ms(), None);
    assert_eq!(
        ProviderError::RequestFailed("boom".into()).retry_after_ms(),
        None
    );
    assert_eq!(
        ProviderError::StreamError("boom".into()).retry_after_ms(),
        None
    );
    assert_eq!(
        ProviderError::AuthFailed("boom".into()).retry_after_ms(),
        None
    );
}

// ---------------------------------------------------------------------------
// category() is consistent with is_retryable()
// ---------------------------------------------------------------------------

#[test]
fn only_rate_limit_and_timeout_categories_are_retryable() {
    for error in [
        ProviderError::RateLimited {
            retry_after_ms: None,
        },
        ProviderError::Timeout,
    ] {
        assert!(
            error.is_retryable(),
            "{:?} should be retryable",
            error.category()
        );
    }
    for error in [
        ProviderError::RequestFailed("boom".into()),
        ProviderError::StreamError("boom".into()),
        ProviderError::AuthFailed("boom".into()),
    ] {
        assert!(
            !error.is_retryable(),
            "{:?} should not be retryable",
            error.category()
        );
    }
}
