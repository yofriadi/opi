//! Proxy support tests (task 3.12).
//!
//! Verifies proxy configuration from env vars and config, precedence,
//! NO_PROXY exclusion, credential redaction, and provider wiring.
//! No live network dependency.

use std::sync::{Arc, Mutex};

use opi_ai::anthropic::AnthropicProvider;
use opi_ai::gemini::GeminiProvider;
use opi_ai::http::{
    HttpClient, HttpClientBuilder, ProxyConfig, proxy_from_env, redact_proxy_credentials,
    resolve_proxy,
};
use opi_ai::openai_chat::OpenAiChatProvider;
use opi_ai::openai_responses::OpenAiResponsesProvider;

// Serialize env-var tests to avoid parallel interference.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Run `f` with all proxy-related env vars cleared, restoring originals on
/// return. Acquires `ENV_MUTEX` so only one test touches these vars at a time.
fn with_clean_proxy_env<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _lock = ENV_MUTEX.lock().unwrap();
    let vars = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "no_proxy",
    ];
    let originals: Vec<(String, Option<String>)> = vars
        .iter()
        .map(|k| (k.to_string(), std::env::var(k).ok()))
        .collect();
    for k in &vars {
        unsafe { std::env::remove_var(k) };
    }
    let result = f();
    for (key, val) in &originals {
        match val {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
    }
    result
}

// ---------------------------------------------------------------------------
// ProxyConfig defaults
// ---------------------------------------------------------------------------

#[test]
fn proxy_config_default_is_none() {
    let config = ProxyConfig::default();
    assert!(config.url.is_none(), "default url should be None");
    assert!(config.no_proxy.is_none(), "default no_proxy should be None");
}

// ---------------------------------------------------------------------------
// HttpClientBuilder proxy configuration
// ---------------------------------------------------------------------------

#[test]
fn builder_without_proxy_has_none() {
    let client = HttpClient::new();
    let proxy = client.proxy_config();
    assert!(proxy.url.is_none(), "no proxy set by default");
    assert!(proxy.no_proxy.is_none());
}

#[test]
fn builder_with_proxy_url() {
    let client = HttpClientBuilder::new()
        .proxy(ProxyConfig {
            url: Some("http://proxy.example.com:8080".into()),
            no_proxy: None,
        })
        .build()
        .expect("build should succeed");
    assert_eq!(
        client.proxy_config().url.as_deref(),
        Some("http://proxy.example.com:8080")
    );
}

#[test]
fn builder_with_proxy_and_no_proxy() {
    let client = HttpClientBuilder::new()
        .proxy(ProxyConfig {
            url: Some("http://proxy.example.com:8080".into()),
            no_proxy: Some("localhost,*.internal".into()),
        })
        .build()
        .expect("build should succeed");
    assert_eq!(
        client.proxy_config().no_proxy.as_deref(),
        Some("localhost,*.internal")
    );
}

#[test]
fn builder_normalizes_empty_url_to_none() {
    let client = HttpClientBuilder::new()
        .proxy(ProxyConfig {
            url: Some(String::new()),
            no_proxy: None,
        })
        .build()
        .expect("build should succeed");
    assert!(
        client.proxy_config().url.is_none(),
        "empty URL should normalize to None"
    );
}

// ---------------------------------------------------------------------------
// resolve_proxy — pure function tests
// ---------------------------------------------------------------------------

#[test]
fn resolve_prefers_https_proxy_over_http_proxy() {
    let config = resolve_proxy(
        Some("http://http-proxy:8080"),
        Some("http://https-proxy:8080"),
        None,
    );
    assert_eq!(config.url.as_deref(), Some("http://https-proxy:8080"));
}

#[test]
fn resolve_falls_back_to_http_proxy() {
    let config = resolve_proxy(Some("http://http-proxy:8080"), None, None);
    assert_eq!(config.url.as_deref(), Some("http://http-proxy:8080"));
}

#[test]
fn resolve_reads_no_proxy() {
    let config = resolve_proxy(
        Some("http://proxy:8080"),
        None,
        Some("localhost,*.internal"),
    );
    assert_eq!(config.no_proxy.as_deref(), Some("localhost,*.internal"));
}

#[test]
fn resolve_none_when_all_empty() {
    let config = resolve_proxy(None, None, None);
    assert!(config.url.is_none());
    assert!(config.no_proxy.is_none());
}

#[test]
fn resolve_ignores_empty_strings() {
    let config = resolve_proxy(Some(""), Some(""), Some(""));
    assert!(
        config.url.is_none(),
        "empty strings should be treated as None"
    );
    assert!(config.no_proxy.is_none());
}

// ---------------------------------------------------------------------------
// proxy_from_env — env var reading (serialized)
// ---------------------------------------------------------------------------

#[test]
fn proxy_from_env_reads_https_proxy_uppercase() {
    let config = with_clean_proxy_env(|| {
        unsafe { std::env::set_var("HTTPS_PROXY", "http://secure-proxy:8080") };
        proxy_from_env()
    });
    assert_eq!(config.url.as_deref(), Some("http://secure-proxy:8080"));
}

#[test]
fn proxy_from_env_reads_http_proxy_lowercase() {
    let config = with_clean_proxy_env(|| {
        unsafe { std::env::set_var("http_proxy", "http://lower-proxy:8080") };
        proxy_from_env()
    });
    assert_eq!(config.url.as_deref(), Some("http://lower-proxy:8080"));
}

#[test]
#[cfg(unix)]
fn proxy_from_env_uppercase_takes_precedence() {
    let config = with_clean_proxy_env(|| {
        unsafe { std::env::set_var("HTTP_PROXY", "http://upper:8080") };
        unsafe { std::env::set_var("http_proxy", "http://lower:8080") };
        proxy_from_env()
    });
    assert_eq!(config.url.as_deref(), Some("http://upper:8080"));
}

#[test]
fn proxy_from_env_reads_no_proxy() {
    let config = with_clean_proxy_env(|| {
        unsafe { std::env::set_var("HTTP_PROXY", "http://proxy:8080") };
        unsafe { std::env::set_var("NO_PROXY", "localhost,*.corp") };
        proxy_from_env()
    });
    assert_eq!(config.no_proxy.as_deref(), Some("localhost,*.corp"));
}

#[test]
fn proxy_from_env_reads_no_proxy_lowercase() {
    let config = with_clean_proxy_env(|| {
        unsafe { std::env::set_var("HTTP_PROXY", "http://proxy:8080") };
        unsafe { std::env::set_var("no_proxy", "localhost") };
        proxy_from_env()
    });
    assert_eq!(config.no_proxy.as_deref(), Some("localhost"));
}

#[test]
fn proxy_from_env_none_when_unset() {
    let config = with_clean_proxy_env(proxy_from_env);
    assert!(config.url.is_none());
    assert!(config.no_proxy.is_none());
}

// ---------------------------------------------------------------------------
// Credential redaction
// ---------------------------------------------------------------------------

#[test]
fn redact_hides_user_and_password() {
    let redacted = redact_proxy_credentials("http://user:secret@proxy.example.com:8080");
    assert_eq!(redacted, "http://***:***@proxy.example.com:8080");
}

#[test]
fn redact_preserves_url_without_credentials() {
    let redacted = redact_proxy_credentials("http://proxy.example.com:8080");
    assert_eq!(redacted, "http://proxy.example.com:8080");
}

#[test]
fn redact_handles_empty_string() {
    let redacted = redact_proxy_credentials("");
    assert_eq!(redacted, "");
}

#[test]
fn redact_handles_https_url() {
    let redacted = redact_proxy_credentials("https://admin:pass123@secure-proxy.corp:3128");
    assert_eq!(redacted, "https://***:***@secure-proxy.corp:3128");
}

#[test]
fn redact_handles_user_only() {
    let redacted = redact_proxy_credentials("http://justuser@proxy:8080");
    assert_eq!(redacted, "http://***@proxy:8080");
}

// ---------------------------------------------------------------------------
// Provider wiring — proxy config flows through to provider
// ---------------------------------------------------------------------------

#[test]
fn anthropic_provider_with_proxy_client() {
    let client = Arc::new(
        HttpClientBuilder::new()
            .proxy(ProxyConfig {
                url: Some("http://proxy.example.com:8080".into()),
                no_proxy: Some("localhost".into()),
            })
            .build()
            .expect("build"),
    );
    let provider = AnthropicProvider::with_client("test-key".into(), None, client);
    let proxy = provider.http_client().proxy_config();
    assert_eq!(proxy.url.as_deref(), Some("http://proxy.example.com:8080"));
    assert_eq!(proxy.no_proxy.as_deref(), Some("localhost"));
}

#[test]
fn openai_chat_provider_with_proxy_client() {
    let client = Arc::new(
        HttpClientBuilder::new()
            .proxy(ProxyConfig {
                url: Some("http://proxy.example.com:8080".into()),
                no_proxy: None,
            })
            .build()
            .expect("build"),
    );
    let provider =
        OpenAiChatProvider::with_client("test-key".into(), None, "openai".into(), vec![], client);
    assert_eq!(
        provider.http_client().proxy_config().url.as_deref(),
        Some("http://proxy.example.com:8080")
    );
}

#[test]
fn openai_responses_provider_with_proxy_client() {
    let client = Arc::new(
        HttpClientBuilder::new()
            .proxy(ProxyConfig {
                url: Some("http://proxy.example.com:8080".into()),
                no_proxy: None,
            })
            .build()
            .expect("build"),
    );
    let provider = OpenAiResponsesProvider::with_client("test-key".into(), None, client);
    assert_eq!(
        provider.http_client().proxy_config().url.as_deref(),
        Some("http://proxy.example.com:8080")
    );
}

#[test]
fn gemini_provider_with_proxy_client() {
    let client = Arc::new(
        HttpClientBuilder::new()
            .proxy(ProxyConfig {
                url: Some("http://proxy.example.com:8080".into()),
                no_proxy: None,
            })
            .build()
            .expect("build"),
    );
    let provider = GeminiProvider::with_client("test-key".into(), None, client);
    assert_eq!(
        provider.http_client().proxy_config().url.as_deref(),
        Some("http://proxy.example.com:8080")
    );
}
