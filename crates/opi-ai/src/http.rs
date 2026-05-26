//! Shared HTTP client with connection pooling (task 3.13).
//!
//! Provides [`HttpClient`] wrapping `reqwest::Client` with tuned pool
//! defaults, and [`HttpClientBuilder`] for custom configuration. All providers
//! should store `Arc<HttpClient>` to avoid per-request client allocation.

use std::time::Duration;

/// Default maximum idle connections per host in the connection pool.
const DEFAULT_POOL_MAX_IDLE_PER_HOST: usize = 10;

/// Default idle timeout for pooled connections.
const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Shared HTTP client with tuned connection-pool settings.
///
/// Wraps a `reqwest::Client` with sensible defaults for LLM provider use:
/// connection pooling enabled, limited idle connections per host, and a
/// reasonable idle timeout. Designed to be held as `Arc<HttpClient>` per
/// provider or shared across providers.
#[derive(Debug)]
pub struct HttpClient {
    inner: reqwest::Client,
    max_idle_per_host: usize,
    idle_timeout: Duration,
}

impl HttpClient {
    /// Create a new client with default pool settings.
    ///
    /// Defaults:
    /// - `pool_max_idle_per_host`: 10
    /// - `pool_idle_timeout`: 90 seconds
    pub fn new() -> Self {
        HttpClientBuilder::new()
            .build()
            .expect("HttpClient construction should not fail with valid defaults")
    }

    /// Access the underlying `reqwest::Client`.
    pub fn client(&self) -> &reqwest::Client {
        &self.inner
    }

    /// Return the pool configuration as `(max_idle_per_host, idle_timeout)`.
    pub fn pool_config(&self) -> (usize, Duration) {
        (self.max_idle_per_host, self.idle_timeout)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for custom `HttpClient` instances.
pub struct HttpClientBuilder {
    max_idle_per_host: usize,
    idle_timeout: Duration,
}

impl HttpClientBuilder {
    /// Create a builder with default settings.
    pub fn new() -> Self {
        Self {
            max_idle_per_host: DEFAULT_POOL_MAX_IDLE_PER_HOST,
            idle_timeout: DEFAULT_POOL_IDLE_TIMEOUT,
        }
    }

    /// Set the maximum number of idle connections per host.
    pub fn max_idle_per_host(mut self, n: usize) -> Self {
        self.max_idle_per_host = n;
        self
    }

    /// Set the idle timeout for pooled connections.
    pub fn idle_timeout(mut self, d: Duration) -> Self {
        self.idle_timeout = d;
        self
    }

    /// Build the `HttpClient`.
    ///
    /// Returns an error if the underlying `reqwest::Client` fails to
    /// construct (e.g. invalid TLS configuration).
    pub fn build(self) -> Result<HttpClient, reqwest::Error> {
        let inner = reqwest::Client::builder()
            .pool_max_idle_per_host(self.max_idle_per_host)
            .pool_idle_timeout(Some(self.idle_timeout))
            .build()?;
        Ok(HttpClient {
            inner,
            max_idle_per_host: self.max_idle_per_host,
            idle_timeout: self.idle_timeout,
        })
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
