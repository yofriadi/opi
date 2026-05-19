//! General-purpose agent runtime with tool calling and transport abstraction.
//!
//! Provides the foundation for building specialized agents with pluggable
//! tool systems and communication transports.

pub mod state;
pub mod tool;
pub mod transport;

pub use state::AgentState;
pub use tool::{Error, Tool};
pub use transport::Transport;
