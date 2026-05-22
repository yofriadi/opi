//! Retry/backoff/rate-limit tests (task 2.15).
//!
//! DoD: "exponential backoff, rate-limit header parsing (Retry-After,
//!       x-ratelimit-*), AutoRetry events, max attempts from config"
//!
//! This file tests: header parsing, backoff calculation, RetryConfig,
//! MockProvider error injection, ProviderError retryability.
//!
//! Agent-level retry integration tests are in opi-agent/tests/retry_agent.rs.

use opi_ai::Provider;
use opi_ai::provider::ProviderError;
use opi_ai::retry::{RetryConfig, calculate_backoff_delay, parse_retry_after};
use opi_ai::test_support::{self, MockProvider, MockResponse};

// ---------------------------------------------------------------------------
// Retry-After header parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_retry_after_seconds_value() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("retry-after", "30".parse().unwrap());
    let ms = parse_retry_after(&headers);
    assert_eq!(ms, Some(30_000));
}

#[test]
fn parse_retry_after_no_header_returns_none() {
    let headers = reqwest::header::HeaderMap::new();
    assert_eq!(parse_retry_after(&headers), None);
}

#[test]
fn parse_retry_after_x_ratelimit_reset() {
    let mut headers = reqwest::header::HeaderMap::new();
    let future_ts = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 60) as f64;
    headers.insert("x-ratelimit-reset", future_ts.to_string().parse().unwrap());
    let ms = parse_retry_after(&headers);
    let ms = ms.expect("should parse x-ratelimit-reset");
    assert!(
        (55_000..=65_000).contains(&ms),
        "expected ~60000ms, got {ms}"
    );
}

#[test]
fn parse_retry_after_prefers_retry_after_over_ratelimit() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("retry-after", "10".parse().unwrap());
    headers.insert("x-ratelimit-reset", "9999999999".parse().unwrap());
    assert_eq!(parse_retry_after(&headers), Some(10_000));
}

#[test]
fn parse_retry_after_invalid_value_returns_none() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("retry-after", "not-a-number".parse().unwrap());
    assert_eq!(parse_retry_after(&headers), None);
}

// ---------------------------------------------------------------------------
// Exponential backoff calculation
// ---------------------------------------------------------------------------

#[test]
fn backoff_doubles_each_attempt() {
    let base = 1000;
    let max = 60_000;
    assert_eq!(calculate_backoff_delay(0, base, max), 1000);
    assert_eq!(calculate_backoff_delay(1, base, max), 2000);
    assert_eq!(calculate_backoff_delay(2, base, max), 4000);
    assert_eq!(calculate_backoff_delay(3, base, max), 8000);
}

#[test]
fn backoff_capped_at_max_delay() {
    assert_eq!(calculate_backoff_delay(3, 1000, 5000), 5000);
}

#[test]
fn backoff_with_large_attempt_stays_at_max() {
    assert_eq!(calculate_backoff_delay(10, 1000, 60_000), 60_000);
}

#[test]
fn backoff_zero_base_stays_zero() {
    assert_eq!(calculate_backoff_delay(5, 0, 60_000), 0);
}

// ---------------------------------------------------------------------------
// RetryConfig
// ---------------------------------------------------------------------------

#[test]
fn retry_config_default_values() {
    let config = RetryConfig::default();
    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.initial_delay_ms, 1000);
    assert_eq!(config.max_delay_ms, 60_000);
}

#[test]
fn retry_config_delay_for_attempt_respects_retry_after() {
    let config = RetryConfig::default();
    let delay = config.delay_for_attempt(0, Some(5000));
    assert_eq!(delay, 5000);
}

#[test]
fn retry_config_delay_for_attempt_uses_backoff_when_no_retry_after() {
    let config = RetryConfig {
        max_attempts: 5,
        initial_delay_ms: 1000,
        max_delay_ms: 60_000,
    };
    assert_eq!(config.delay_for_attempt(0, None), 1000);
    assert_eq!(config.delay_for_attempt(1, None), 2000);
    assert_eq!(config.delay_for_attempt(2, None), 4000);
}

#[test]
fn retry_config_delay_respects_max_even_with_retry_after() {
    let config = RetryConfig {
        max_attempts: 3,
        initial_delay_ms: 1000,
        max_delay_ms: 5000,
    };
    let delay = config.delay_for_attempt(0, Some(10000));
    assert_eq!(delay, 5000);
}

#[test]
fn retry_config_should_retry_within_max_attempts() {
    let config = RetryConfig::default();
    assert!(config.should_retry(0));
    assert!(config.should_retry(1));
    assert!(config.should_retry(2));
    assert!(!config.should_retry(3));
}

// ---------------------------------------------------------------------------
// MockProvider error injection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mock_provider_returns_error_response() {
    let error = ProviderError::RateLimited {
        retry_after_ms: Some(5000),
    };
    let provider = MockProvider::new_with_errors(
        "mock",
        vec![
            MockResponse::Error(error),
            MockResponse::Events(test_support::text_response("success")),
        ],
    );
    let request = opi_ai::provider::Request {
        model: "mock-model".into(),
        system: None,
        messages: vec![],
        tools: vec![],
        max_tokens: None,
        temperature: None,
        thinking: Default::default(),
        stop_sequences: vec![],
        metadata: None,
        cancel: tokio_util::sync::CancellationToken::new(),
    };
    use futures_util::StreamExt;
    let mut stream = provider.stream(request);
    let first = stream.next().await.unwrap();
    assert!(first.is_err(), "first call should return error");
    match first.unwrap_err() {
        ProviderError::RateLimited { retry_after_ms } => {
            assert_eq!(retry_after_ms, Some(5000));
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// ProviderError retryability
// ---------------------------------------------------------------------------

#[test]
fn rate_limited_is_retryable() {
    assert!(
        ProviderError::RateLimited {
            retry_after_ms: None
        }
        .is_retryable()
    );
}

#[test]
fn timeout_is_retryable() {
    assert!(ProviderError::Timeout.is_retryable());
}

#[test]
fn auth_failed_is_not_retryable() {
    assert!(!ProviderError::AuthFailed("bad key".into()).is_retryable());
}

#[test]
fn request_failed_is_not_retryable() {
    assert!(!ProviderError::RequestFailed("500 error".into()).is_retryable());
}
