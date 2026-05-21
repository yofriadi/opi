//! General-purpose agent runtime with tool calling and transport abstraction.
//!
//! Provides the foundation for building specialized agents with pluggable
//! tool systems and communication transports.

pub mod state;
pub mod tool;
pub mod transport;
pub mod validation;

pub use state::AgentState;
pub use tool::{ExecutionMode, Tool, ToolError, ToolResult};
pub use transport::Transport;

// Re-export provider-facing types needed at the agent boundary.
pub use opi_ai::message::ToolDef;
