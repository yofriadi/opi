//! Provider configuration and authentication.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("provider not configured: {0}")]
    ProviderNotConfigured(String),
    #[error("authentication failed: {0}")]
    AuthFailed(String),
    #[error("request failed: {0}")]
    RequestFailed(String),
}

pub type Result<T> = std::result::Result<T, Error>;
