//! Model discovery and configuration.

use crate::provider::ProviderKind;

#[derive(Debug, Clone)]
pub struct Model {
    pub id: String,
    pub provider: ProviderKind,
    pub context_window: u32,
}
