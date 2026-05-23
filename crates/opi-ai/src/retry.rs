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
/// 1. `retry-after` — seconds value or HTTP-date
/// 2. `x-ratelimit-reset` — Unix timestamp
///
/// Returns delay in milliseconds, or `None` if no usable header is found.
pub fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    // Priority 1: Retry-After header
    if let Some(val) = headers.get("retry-after") && let Ok(s) = val.to_str() {
        // Try seconds first
        if let Ok(secs) = s.parse::<f64>() {
            return Some((secs * 1000.0) as u64);
        }
        // Try HTTP-date
        if let Some(delay_ms) = parse_http_date_delay(s) {
            return Some(delay_ms);
        }
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

/// Parse an HTTP-date string and return the delay in milliseconds from now.
///
/// Supports IMF-fixdate format: `Day, DD Mon YYYY HH:MM:SS GMT`
/// Example: `Fri, 23 May 2026 12:00:00 GMT`
///
/// Returns `None` if the date cannot be parsed or is in the past.
pub fn parse_http_date_delay(s: &str) -> Option<u64> {
    let ts = parse_imf_fixdate(s)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    if ts > now {
        Some((ts - now) * 1000)
    } else {
        None
    }
}

/// Parse IMF-fixdate format: `Day, DD Mon YYYY HH:MM:SS GMT`
fn parse_imf_fixdate(s: &str) -> Option<u64> {
    // Expected format: "Fri, 23 May 2026 12:00:00 GMT"
    let s = s.trim();
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    let rest = parts[1].trim();
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    // Expected: ["DD", "Mon", "YYYY", "HH:MM:SS", "GMT"]
    if tokens.len() != 5 {
        return None;
    }
    let day: u32 = tokens[0].parse().ok()?;
    let month = parse_month(tokens[1])?;
    let year: u32 = tokens[2].parse().ok()?;
    let time_parts: Vec<&str> = tokens[3].split(':').collect();
    if time_parts.len() != 3 {
        return None;
    }
    let hour: u32 = time_parts[0].parse().ok()?;
    let minute: u32 = time_parts[1].parse().ok()?;
    let second: u32 = time_parts[2].parse().ok()?;
    if tokens[4] != "GMT" {
        return None;
    }

    // Convert to Unix timestamp using civil date calculation
    Some(date_to_unix(year, month, day, hour, minute, second))
}

/// Map three-letter month abbreviation to month number (1-12).
fn parse_month(s: &str) -> Option<u32> {
    match s {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
}

/// Convert a civil date/time to a Unix timestamp (seconds since epoch, UTC).
fn date_to_unix(year: u32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> u64 {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let m = if month <= 2 { month + 9 } else { month - 3 } as i64;
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let secs = days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64;
    secs as u64
}
