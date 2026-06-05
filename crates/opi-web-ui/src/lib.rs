//! Reusable web UI components for AI chat interfaces.
//!
//! Provides an embeddable component layer that consumes RPC/SDK events from
//! the opi agent toolkit and renders them as typed Rust state and HTML
//! components.
//!
//! # Architecture
//!
//! - **[`event`]** — Parses raw JSON values from the RPC JSONL protocol into
//!   typed [`WebUiEvent`] variants.
//! - **[`state`]** — Conversation state machine that processes events and
//!   maintains message history, tool call state, and session metadata.
//! - **[`components`]** — Typed UI component models (chat messages, tool call
//!   views, thinking blocks, status bars, conversation containers).
//! - **[`render`]** — HTML rendering trait and escape utilities.
//!
//! # Unstable 0.x API
//!
//! This crate is `publish = false` and all types are subject to change between
//! versions. Pin an exact version and test against upgrades.

pub mod components;
pub mod event;
pub mod render;
pub mod state;

pub use components::{ChatMessage, ConversationView, StatusBar, ToolCallView};
pub use event::WebUiEvent;
pub use render::Render;
pub use state::ConversationState;
