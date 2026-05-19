//! LLM provider abstraction.

use async_trait::async_trait;

use crate::stream::StreamEvent;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn complete(
        &self,
        messages: &[serde_json::Value],
    ) -> Result<Vec<StreamEvent>, crate::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Google,
    Mistral,
    Bedrock,
    Azure,
}
