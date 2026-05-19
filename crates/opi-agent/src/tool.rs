//! Tool calling abstraction.

use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, input: Value) -> Result<Value, crate::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}
