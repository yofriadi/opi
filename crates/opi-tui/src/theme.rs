//! Theme struct and palette model for TUI rendering.
//!
//! Provides a [`Theme`] with semantic color fields covering all widget roles,
//! built-in themes ("default" and "monokai"), and a [`resolve_theme`] function
//! for name-based lookup.

use ratatui::style::Color;

/// Semantic color palette for all TUI widgets.
///
/// Each field maps to a specific visual role in the interface. Widgets read
/// their colors from a `Theme` instance rather than hardcoding `Color` values.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Theme identifier (e.g. "default", "monokai").
    pub name: &'static str,
    // -- Role colors (MessageList) --
    /// User message label color.
    pub role_user: Color,
    /// Assistant message label color.
    pub role_assistant: Color,
    /// System message label color.
    pub role_system: Color,
    /// Tool message label color.
    pub role_tool: Color,
    // -- Status bar --
    /// Status bar background.
    pub status_bg: Color,
    /// Status text when idle.
    pub status_idle: Color,
    /// Status text when thinking.
    pub status_thinking: Color,
    /// Status text when streaming.
    pub status_streaming: Color,
    /// Status text when executing a tool.
    pub status_tool: Color,
    /// Token count text color.
    pub status_tokens: Color,
    // -- Editor --
    /// Input editor border/title color.
    pub editor_title: Color,
    /// Placeholder text color in empty input.
    pub editor_placeholder: Color,
    // -- Markdown / code --
    /// Code block border/title color.
    pub code_title: Color,
    /// Code block content color.
    pub code_content: Color,
    /// Heading level 1 color.
    pub heading_h1: Color,
    /// Heading level 2 color.
    pub heading_h2: Color,
    /// Heading level 3+ color.
    pub heading_h3: Color,
    /// Italic text color.
    pub italic: Color,
    // -- Diff view --
    /// DiffView border color.
    pub diff_border: Color,
    /// Diff hunk header color.
    pub diff_header: Color,
    /// Diff context line color.
    pub diff_context: Color,
    /// Diff added line color.
    pub diff_added: Color,
    /// Diff removed line color.
    pub diff_removed: Color,
    /// "(no changes)" text color.
    pub diff_no_changes: Color,
    // -- Tool call view --
    /// Running tool call status color.
    pub tool_running: Color,
    /// Successful tool call status color.
    pub tool_success: Color,
    /// Failed tool call status color.
    pub tool_error: Color,
    // -- SelectList --
    /// SelectList border/title color.
    pub picker_title: Color,
    /// SelectList selected row background.
    pub picker_selected_bg: Color,
    /// SelectList selected row text color.
    pub picker_selected_fg: Color,
    /// SelectList filter input prompt color.
    pub picker_filter: Color,
    /// SelectList item metadata color.
    pub picker_metadata: Color,
    /// SelectList empty-state text color.
    pub picker_empty: Color,
}

impl Default for Theme {
    /// The "default" theme matching the original hardcoded colors.
    fn default() -> Self {
        Self {
            name: "default",
            // Role colors
            role_user: Color::Green,
            role_assistant: Color::Cyan,
            role_system: Color::Yellow,
            role_tool: Color::Magenta,
            // Status bar
            status_bg: Color::DarkGray,
            status_idle: Color::White,
            status_thinking: Color::Yellow,
            status_streaming: Color::Green,
            status_tool: Color::Magenta,
            status_tokens: Color::DarkGray,
            // Editor
            editor_title: Color::Yellow,
            editor_placeholder: Color::DarkGray,
            // Markdown / code
            code_title: Color::Yellow,
            code_content: Color::Gray,
            heading_h1: Color::Cyan,
            heading_h2: Color::Yellow,
            heading_h3: Color::White,
            italic: Color::Cyan,
            // Diff view
            diff_border: Color::Cyan,
            diff_header: Color::Blue,
            diff_context: Color::Gray,
            diff_added: Color::Green,
            diff_removed: Color::Red,
            diff_no_changes: Color::DarkGray,
            // Tool call view
            tool_running: Color::Yellow,
            tool_success: Color::Green,
            tool_error: Color::Red,
            // SelectList
            picker_title: Color::Cyan,
            picker_selected_bg: Color::DarkGray,
            picker_selected_fg: Color::White,
            picker_filter: Color::Yellow,
            picker_metadata: Color::DarkGray,
            picker_empty: Color::DarkGray,
        }
    }
}

impl Theme {
    /// Monokai-inspired dark theme with warm accent colors.
    pub fn monokai() -> Self {
        Self {
            name: "monokai",
            // Role colors
            role_user: Color::Rgb(166, 226, 46),       // green
            role_assistant: Color::Rgb(102, 217, 239), // cyan-blue
            role_system: Color::Rgb(230, 219, 116),    // yellow
            role_tool: Color::Rgb(249, 38, 114),       // pink-red
            // Status bar
            status_bg: Color::Rgb(39, 40, 34),          // dark bg
            status_idle: Color::Rgb(248, 248, 242),     // near-white
            status_thinking: Color::Rgb(230, 219, 116), // yellow
            status_streaming: Color::Rgb(166, 226, 46), // green
            status_tool: Color::Rgb(249, 38, 114),      // pink-red
            status_tokens: Color::Rgb(117, 113, 94),    // dim
            // Editor
            editor_title: Color::Rgb(230, 219, 116), // yellow
            editor_placeholder: Color::Rgb(117, 113, 94), // dim
            // Markdown / code
            code_title: Color::Rgb(230, 219, 116),   // yellow
            code_content: Color::Rgb(248, 248, 242), // near-white
            heading_h1: Color::Rgb(166, 226, 46),    // green
            heading_h2: Color::Rgb(253, 151, 31),    // orange
            heading_h3: Color::Rgb(248, 248, 242),   // near-white
            italic: Color::Rgb(102, 217, 239),       // cyan-blue
            // Diff view
            diff_border: Color::Rgb(102, 217, 239), // cyan-blue
            diff_header: Color::Rgb(102, 217, 239), // cyan-blue
            diff_context: Color::Rgb(117, 113, 94), // dim
            diff_added: Color::Rgb(166, 226, 46),   // green
            diff_removed: Color::Rgb(249, 38, 114), // pink-red
            diff_no_changes: Color::Rgb(117, 113, 94), // dim
            // Tool call view
            tool_running: Color::Rgb(230, 219, 116), // yellow
            tool_success: Color::Rgb(166, 226, 46),  // green
            tool_error: Color::Rgb(249, 38, 114),    // pink-red
            // SelectList
            picker_title: Color::Rgb(102, 217, 239), // cyan-blue
            picker_selected_bg: Color::Rgb(73, 72, 62), // dim highlight
            picker_selected_fg: Color::Rgb(248, 248, 242), // near-white
            picker_filter: Color::Rgb(230, 219, 116), // yellow
            picker_metadata: Color::Rgb(117, 113, 94), // dim
            picker_empty: Color::Rgb(117, 113, 94),  // dim
        }
    }
}

/// Resolve a theme by name. Returns the default theme for unknown names.
pub fn resolve_theme(name: &str) -> Theme {
    match name {
        "default" => Theme::default(),
        "monokai" => Theme::monokai(),
        _ => Theme::default(),
    }
}
