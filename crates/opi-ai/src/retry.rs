//! Retry/backoff/rate-limit support (task 2.15).
//!
//! Provides header parsing, exponential backoff calculation, and retry config.

/// Configuration for automatic retry of retryable provider errors.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not counting the initial request).
    pub max_attempts: u32,
    /// Base delay in milliseconds for exponential backoff.
    pub initial_delay_ms: u64,
    /// Upper bound on delay in milliseconds.
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 60_000,
        }
    }
}

impl RetryConfig {
    /// Calculate the delay for a given retry attempt.
    ///
    /// If `retry_after_ms` is provided (from provider headers), it is used
    /// (capped to `max_delay_ms`). Otherwise, exponential backoff is applied:
    /// `initial_delay_ms * 2^attempt`, capped at `max_delay_ms`.
    pub fn delay_for_attempt(&self, attempt: u32, retry_after_ms: Option<u64>) -> u64 {
        let raw = match retry_after_ms {
            Some(ms) => ms,
            None => calculate_backoff_delay(attempt, self.initial_delay_ms, self.max_delay_ms),
        };
        raw.min(self.max_delay_ms)
    }

    /// Whether a retry should be attempted at the given attempt number.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

/// Calculate exponential backoff delay: `base_ms * 2^attempt`, capped at `max_delay_ms`.
pub fn calculate_backoff_delay(attempt: u32, base_ms: u64, max_delay_ms: u64) -> u64 {
    let multiplier = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
    base_ms.saturating_mul(multiplier).min(max_delay_ms)
}

/// Parse retry delay from HTTP response headers.
///
/// Checks headers in priority order:
/// 1. `retry-after` — seconds value (HTTP date not supported)
/// 2. `x-ratelimit-reset` — Unix timestamp
///
/// Returns delay in milliseconds, or `None` if no usable header is found.
pub fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    // Priority 1: Retry-After header (seconds)
    if let Some(val) = headers.get("retry-after")
        && let Ok(s) = val.to_str()
        && let Ok(secs) = s.parse::<f64>()
    {
        return Some((secs * 1000.0) as u64);
    }

    // Priority 2: x-ratelimit-reset (Unix timestamp)
    if let Some(val) = headers.get("x-ratelimit-reset")
        && let Ok(s) = val.to_str()
        && let Ok(timestamp) = s.parse::<f64>()
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let delay_secs = timestamp - now;
        if delay_secs > 0.0 {
            return Some((delay_secs * 1000.0) as u64);
        }
    }

    None
}
