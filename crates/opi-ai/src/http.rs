//! Shared HTTP client with connection pooling and proxy support (tasks 3.13, 3.12).
//!
//! Provides [`HttpClient`] wrapping `reqwest::Client` with tuned pool
//! defaults, proxy configuration, and [`HttpClientBuilder`] for custom
//! configuration. All providers should store `Arc<HttpClient>` to avoid
//! per-request client allocation.

use std::time::Duration;

/// Default maximum idle connections per host in the connection pool.
const DEFAULT_POOL_MAX_IDLE_PER_HOST: usize = 10;

/// Default idle timeout for pooled connections.
const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Proxy configuration for an [`HttpClient`].
///
/// When `url` is `Some`, the client routes requests through the proxy.
/// `no_proxy` is a comma-separated list of host patterns that bypass the
/// proxy (e.g. `"localhost,*.internal"`).
#[derive(Debug, Clone, Default)]
pub struct ProxyConfig {
    /// Proxy URL (e.g. `http://proxy.example.com:8080`).
    pub url: Option<String>,
    /// Comma-separated host patterns to exclude from proxying.
    pub no_proxy: Option<String>,
}

impl ProxyConfig {
    fn normalize(&mut self) {
        if self.url.as_ref().is_some_and(|s| s.trim().is_empty()) {
            self.url = None;
        }
        if self.no_proxy.as_ref().is_some_and(|s| s.trim().is_empty()) {
            self.no_proxy = None;
        }
    }
}

/// Shared HTTP client with tuned connection-pool and proxy settings.
///
/// Wraps a `reqwest::Client` with sensible defaults for LLM provider use:
/// connection pooling enabled, limited idle connections per host, a
/// reasonable idle timeout, and optional proxy configuration. Designed to be
/// held as `Arc<HttpClient>` per provider or shared across providers.
#[derive(Debug)]
pub struct HttpClient {
    inner: reqwest::Client,
    max_idle_per_host: usize,
    idle_timeout: Duration,
    proxy_config: ProxyConfig,
}

impl HttpClient {
    /// Create a new client with default pool settings and no proxy.
    ///
    /// Defaults:
    /// - `pool_max_idle_per_host`: 10
    /// - `pool_idle_timeout`: 90 seconds
    /// - proxy: none
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

    /// Return the resolved proxy configuration.
    pub fn proxy_config(&self) -> &ProxyConfig {
        &self.proxy_config
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
    proxy_config: ProxyConfig,
}

impl HttpClientBuilder {
    /// Create a builder with default settings.
    pub fn new() -> Self {
        Self {
            max_idle_per_host: DEFAULT_POOL_MAX_IDLE_PER_HOST,
            idle_timeout: DEFAULT_POOL_IDLE_TIMEOUT,
            proxy_config: ProxyConfig::default(),
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

    /// Set explicit proxy configuration.
    ///
    /// When set, this takes precedence over environment variable detection.
    /// An empty `url` is normalized to `None` (no proxy).
    pub fn proxy(mut self, config: ProxyConfig) -> Self {
        self.proxy_config = config;
        self.proxy_config.normalize();
        self
    }

    /// Build the `HttpClient`.
    ///
    /// Returns an error if the underlying `reqwest::Client` fails to
    /// construct (e.g. invalid TLS or proxy URL).
    pub fn build(self) -> Result<HttpClient, reqwest::Error> {
        let mut builder = reqwest::Client::builder()
            .pool_max_idle_per_host(self.max_idle_per_host)
            .pool_idle_timeout(Some(self.idle_timeout));

        if let Some(ref url) = self.proxy_config.url {
            let mut proxy = reqwest::Proxy::all(url)?;
            if let Some(ref np) = self.proxy_config.no_proxy {
                proxy = proxy.no_proxy(reqwest::NoProxy::from_string(np));
            }
            builder = builder.proxy(proxy);
        }

        let inner = builder.build()?;
        Ok(HttpClient {
            inner,
            max_idle_per_host: self.max_idle_per_host,
            idle_timeout: self.idle_timeout,
            proxy_config: self.proxy_config,
        })
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve proxy configuration from explicit values.
///
/// `https_proxy` takes precedence over `http_proxy` when both are set.
/// Empty strings are treated as `None`. This is the pure-logic core used by
/// [`proxy_from_env`] and config resolution.
pub fn resolve_proxy(
    http_proxy: Option<&str>,
    https_proxy: Option<&str>,
    no_proxy: Option<&str>,
) -> ProxyConfig {
    let url = https_proxy
        .and_then(|s| {
            if s.trim().is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        })
        .or_else(|| {
            http_proxy.and_then(|s| {
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            })
        });
    let np = no_proxy.and_then(|s| {
        if s.trim().is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });
    ProxyConfig { url, no_proxy: np }
}

/// Read an environment variable, preferring uppercase over lowercase.
fn env_var_case_insensitive(upper: &str, lower: &str) -> Option<String> {
    std::env::var(upper)
        .ok()
        .or_else(|| std::env::var(lower).ok())
}

/// Resolve proxy configuration from standard environment variables.
///
/// Checks both uppercase and lowercase variants of `HTTP_PROXY`,
/// `HTTPS_PROXY`, and `NO_PROXY`. Uppercase takes precedence when both
/// cases exist. `HTTPS_PROXY` takes precedence over `HTTP_PROXY`.
pub fn proxy_from_env() -> ProxyConfig {
    let https_proxy = env_var_case_insensitive("HTTPS_PROXY", "https_proxy");
    let http_proxy = env_var_case_insensitive("HTTP_PROXY", "http_proxy");
    let no_proxy = env_var_case_insensitive("NO_PROXY", "no_proxy");
    resolve_proxy(
        http_proxy.as_deref(),
        https_proxy.as_deref(),
        no_proxy.as_deref(),
    )
}

/// Redact credentials embedded in a proxy URL for safe display.
///
/// Converts `http://user:pass@host:port` to `http://***:***@host:port`.
/// URLs without credentials are returned unchanged.
pub fn redact_proxy_credentials(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(at_pos) = after_scheme.find('@') {
            let credentials = &after_scheme[..at_pos];
            let host_part = &after_scheme[at_pos + 1..];
            if credentials.contains(':') {
                return format!("{}***:***@{}", &url[..scheme_end + 3], host_part);
            }
            // User without password
            return format!("{}***@{}", &url[..scheme_end + 3], host_part);
        }
    }
    url.to_string()
}
