//! Terminal User Interface library with differential rendering.
//!
//! Phase 1 components: [`MessageList`], [`InputEditor`], [`StatusBar`],
//! [`ToolCallView`], composed by [`Shell`].

pub mod diff_view;
pub mod editor;
pub mod keybindings;
pub mod markdown;
pub mod message_list;
pub mod render;
pub mod status_bar;
pub mod theme;
pub mod tool_call;

pub use diff_view::DiffView;
pub use editor::InputEditor;
pub use keybindings::{Key, KeyCombo, KeyComboParseError, Keybindings, Modifiers};
pub use markdown::{CodeBlock, MarkdownView};
pub use message_list::MessageList;
pub use render::Shell;
pub use status_bar::StatusBar;
pub use theme::{Theme, resolve_theme};
pub use tool_call::ToolCallView;

use std::fmt;

/// Error type for TUI operations.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal error: {0}")]
    Terminal(String),
    #[error("render error: {0}")]
    Render(String),
}

/// Role of a conversation message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

/// A single conversation message.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Optional structured diff payload. When present, the message is
    /// rendered with the `DiffView` widget instead of plain text.
    pub diff: Option<DiffPayload>,
}

/// Structured before/after content for diff rendering.
#[derive(Debug, Clone)]
pub struct DiffPayload {
    pub path: String,
    pub before: String,
    pub after: String,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            diff: None,
        }
    }

    /// Build a tool-role message that renders the supplied before/after as a
    /// unified diff via `DiffView`.
    pub fn diff(
        path: impl Into<String>,
        before: impl Into<String>,
        after: impl Into<String>,
    ) -> Self {
        let path = path.into();
        let before = before.into();
        let after = after.into();
        let content = format!("diff: {path}");
        Self {
            role: Role::Tool,
            content,
            diff: Some(DiffPayload {
                path,
                before,
                after,
            }),
        }
    }
}

/// Application state shown in the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle,
    Thinking,
    Streaming,
    ToolExecuting,
}

impl fmt::Display for AppState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Thinking => write!(f, "thinking..."),
            Self::Streaming => write!(f, "streaming..."),
            Self::ToolExecuting => write!(f, "executing tool..."),
        }
    }
}

/// Status of a tool call being displayed.
#[derive(Debug, Clone)]
pub enum ToolCallStatus {
    Running,
    Success,
    Error(String),
}

impl fmt::Display for ToolCallStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Success => write!(f, "success"),
            Self::Error(e) => write!(f, "error: {e}"),
        }
    }
}
