//! Terminal User Interface library with differential rendering.
//!
//! Synchronous ratatui widget library used by `opi-coding-agent`. Core widgets:
//! [`MessageList`], [`InputEditor`], [`StatusBar`], [`ToolCallView`],
//! [`MarkdownView`], [`CodeBlock`], [`DiffView`], composed by [`Shell`].
//! Themes via [`Theme`]/[`resolve_theme`] and configurable [`Keybindings`].

pub mod diff_view;
pub mod editor;
pub mod keybindings;
pub mod markdown;
pub mod message_list;
pub mod render;
pub mod status_bar;
pub mod terminal_image;
pub mod theme;
pub mod tool_call;

pub use diff_view::DiffView;
pub use editor::InputEditor;
pub use keybindings::{Key, KeyCombo, KeyComboParseError, Keybindings, Modifiers};
pub use markdown::{CodeBlock, MarkdownView};
pub use message_list::MessageList;
pub use render::Shell;
pub use status_bar::StatusBar;
pub use terminal_image::{
    CapabilitySource, ImageData, MediaType, TerminalGraphicsProtocol, detect_graphics_protocol,
    iterm_escape, kitty_escape, sixel_escape, text_fallback,
};
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
    /// Optional image payload. When present, the message renders an image
    /// using terminal graphics protocol escape sequences or text fallback.
    pub image: Option<ImagePayload>,
}

/// Image payload for terminal rendering.
#[derive(Debug, Clone)]
pub struct ImagePayload {
    pub data: ImageData,
    pub protocol: TerminalGraphicsProtocol,
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
            image: None,
        }
    }

    /// Build an image-only message for terminal rendering.
    pub fn image(role: Role, payload: ImagePayload) -> Self {
        let fallback = text_fallback(&payload.data);
        Self {
            role,
            content: fallback,
            diff: None,
            image: Some(payload),
        }
    }

    /// Attach an image payload to an existing text message.
    pub fn with_image(mut self, payload: ImagePayload) -> Self {
        self.image = Some(payload);
        self
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
            image: None,
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
