//! Transport abstraction for agent communication.

use async_trait::async_trait;

#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, message: &str) -> Result<(), TransportError>;
    async fn receive(&self) -> Result<String, TransportError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("send failed: {0}")]
    SendFailed(String),
}
