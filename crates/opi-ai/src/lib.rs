//! Unified multi-provider LLM API with streaming support.
//!
//! Provides a standardized interface for interacting with multiple LLM providers
//! including OpenAI, Anthropic, Google Gemini, Mistral, AWS Bedrock, and Azure OpenAI.

pub mod config;
pub mod model;
pub mod provider;
pub mod stream;

pub use config::{Config, Error};
pub use model::Model;
pub use provider::Provider;
pub use stream::StreamEvent;
