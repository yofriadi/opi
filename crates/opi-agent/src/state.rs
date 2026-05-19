//! Agent state management.

#[derive(Debug, Default)]
pub struct AgentState {
    pub messages: Vec<serde_json::Value>,
    pub tool_results: Vec<serde_json::Value>,
}
