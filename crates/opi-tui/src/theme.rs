//! Theme struct, palette model, and color parsing for TUI rendering.
//!
//! Provides a [`Theme`] with semantic color fields covering all widget roles,
//! built-in themes ("default" and "monokai"), a [`resolve_theme`] function
//! for name-based lookup, and [`parse_color`] / [`THEME_TOKENS`] for
//! progressive theme discovery.
//!
//! # Theme Token Schema
//!
//! The [`THEME_TOKENS`] constant lists all valid color token names that can
//! appear in a theme definition file. Each token maps to a field on [`Theme`].
//!
//! # Color Format
//!
//! Color values are parsed by [`parse_color`] and support:
//!
//! - **Named colors**: `"Red"`, `"DarkGray"`, `"LightCyan"`, etc.
//! - **Hex RGB**: `"#rrggbb"` (e.g. `"#ff6600"`)
//!
//! # Unstable
//!
//! Theme discovery types are part of the **unstable 0.x extension API**.
//! Breaking changes may occur between minor versions without a major version
//! bump.

use ratatui::style::Color;

// ---------------------------------------------------------------------------
// Color parsing
// ---------------------------------------------------------------------------

/// Errors from color string parsing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ColorParseError {
    /// The color string is not a recognized named color or valid hex RGB.
    #[error("invalid color: {0:?}")]
    InvalidColor(String),
}

/// Parse a color string into a ratatui [`Color`].
///
/// Supports:
/// - Named colors matching ratatui `Color` enum variants (case-sensitive):
///   `"Black"`, `"Red"`, `"Green"`, `"Yellow"`, `"Blue"`, `"Magenta"`,
///   `"Cyan"`, `"Gray"`, `"DarkGray"`, `"LightRed"`, `"LightGreen"`,
///   `"LightYellow"`, `"LightBlue"`, `"LightMagenta"`, `"LightCyan"`,
///   `"White"`, `"Reset"`.
/// - Hex RGB: `"#rrggbb"` (case-insensitive hex digits).
pub fn parse_color(s: &str) -> Result<Color, ColorParseError> {
    // Named color lookup
    match s {
        "Black" => return Ok(Color::Black),
        "Red" => return Ok(Color::Red),
        "Green" => return Ok(Color::Green),
        "Yellow" => return Ok(Color::Yellow),
        "Blue" => return Ok(Color::Blue),
        "Magenta" => return Ok(Color::Magenta),
        "Cyan" => return Ok(Color::Cyan),
        "Gray" => return Ok(Color::Gray),
        "DarkGray" => return Ok(Color::DarkGray),
        "LightRed" => return Ok(Color::LightRed),
        "LightGreen" => return Ok(Color::LightGreen),
        "LightYellow" => return Ok(Color::LightYellow),
        "LightBlue" => return Ok(Color::LightBlue),
        "LightMagenta" => return Ok(Color::LightMagenta),
        "LightCyan" => return Ok(Color::LightCyan),
        "White" => return Ok(Color::White),
        "Reset" => return Ok(Color::Reset),
        _ => {}
    }

    // Hex RGB: #rrggbb
    if let Some(hex) = s.strip_prefix('#')
        && hex.len() == 6
    {
        let r = u8::from_str_radix(&hex[0..2], 16)
            .map_err(|_| ColorParseError::InvalidColor(s.to_string()))?;
        let g = u8::from_str_radix(&hex[2..4], 16)
            .map_err(|_| ColorParseError::InvalidColor(s.to_string()))?;
        let b = u8::from_str_radix(&hex[4..6], 16)
            .map_err(|_| ColorParseError::InvalidColor(s.to_string()))?;
        return Ok(Color::Rgb(r, g, b));
    }

    Err(ColorParseError::InvalidColor(s.to_string()))
}

// ---------------------------------------------------------------------------
// Theme token schema
// ---------------------------------------------------------------------------

/// All valid color token names for theme definitions.
///
/// Each token corresponds to a field on [`Theme`]. Theme files may specify
/// any subset; unspecified tokens inherit from the default theme.
pub static THEME_TOKENS: &[&str] = &[
    "role_user",
    "role_assistant",
    "role_system",
    "role_tool",
    "status_bg",
    "status_idle",
    "status_thinking",
    "status_streaming",
    "status_tool",
    "status_tokens",
    "editor_title",
    "editor_placeholder",
    "code_title",
    "code_content",
    "heading_h1",
    "heading_h2",
    "heading_h3",
    "italic",
    "diff_border",
    "diff_header",
    "diff_context",
    "diff_added",
    "diff_removed",
    "diff_no_changes",
    "tool_running",
    "tool_success",
    "tool_error",
    "picker_title",
    "picker_selected_bg",
    "picker_selected_fg",
    "picker_filter",
    "picker_metadata",
    "picker_empty",
];

/// Check whether a token name is a valid theme color token.
pub fn is_valid_token(token: &str) -> bool {
    THEME_TOKENS.contains(&token)
}

// ---------------------------------------------------------------------------
// Theme struct
// ---------------------------------------------------------------------------

/// Semantic color palette for all TUI widgets.
///
/// Each field maps to a specific visual role in the interface. Widgets read
/// their colors from a `Theme` instance rather than hardcoding `Color` values.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Theme identifier (e.g. "default", "monokai").
    pub name: String,
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
            name: String::from("default"),
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
            name: String::from("monokai"),
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

    /// Build a theme from a name and a partial color map, filling missing
    /// tokens from the default theme.
    ///
    /// The `colors` map keys must be valid theme token names (see
    /// [`THEME_TOKENS`]). Invalid tokens produce an error. Valid tokens not
    /// present in the map inherit their values from [`Theme::default`].
    pub fn from_color_map(
        name: String,
        colors: &std::collections::HashMap<String, Color>,
    ) -> Result<Self, ColorParseError> {
        // Validate all keys are recognized tokens
        for key in colors.keys() {
            if !is_valid_token(key) {
                return Err(ColorParseError::InvalidColor(format!(
                    "unknown theme token: {key}"
                )));
            }
        }

        let defaults = Theme::default();
        let get = |token: &str| -> Color {
            colors
                .get(token)
                .copied()
                .unwrap_or_else(|| Self::get_field(&defaults, token))
        };

        Ok(Self {
            name,
            role_user: get("role_user"),
            role_assistant: get("role_assistant"),
            role_system: get("role_system"),
            role_tool: get("role_tool"),
            status_bg: get("status_bg"),
            status_idle: get("status_idle"),
            status_thinking: get("status_thinking"),
            status_streaming: get("status_streaming"),
            status_tool: get("status_tool"),
            status_tokens: get("status_tokens"),
            editor_title: get("editor_title"),
            editor_placeholder: get("editor_placeholder"),
            code_title: get("code_title"),
            code_content: get("code_content"),
            heading_h1: get("heading_h1"),
            heading_h2: get("heading_h2"),
            heading_h3: get("heading_h3"),
            italic: get("italic"),
            diff_border: get("diff_border"),
            diff_header: get("diff_header"),
            diff_context: get("diff_context"),
            diff_added: get("diff_added"),
            diff_removed: get("diff_removed"),
            diff_no_changes: get("diff_no_changes"),
            tool_running: get("tool_running"),
            tool_success: get("tool_success"),
            tool_error: get("tool_error"),
            picker_title: get("picker_title"),
            picker_selected_bg: get("picker_selected_bg"),
            picker_selected_fg: get("picker_selected_fg"),
            picker_filter: get("picker_filter"),
            picker_metadata: get("picker_metadata"),
            picker_empty: get("picker_empty"),
        })
    }

    /// Get a single color field by token name. Panics if the token is unknown.
    fn get_field(theme: &Theme, token: &str) -> Color {
        match token {
            "role_user" => theme.role_user,
            "role_assistant" => theme.role_assistant,
            "role_system" => theme.role_system,
            "role_tool" => theme.role_tool,
            "status_bg" => theme.status_bg,
            "status_idle" => theme.status_idle,
            "status_thinking" => theme.status_thinking,
            "status_streaming" => theme.status_streaming,
            "status_tool" => theme.status_tool,
            "status_tokens" => theme.status_tokens,
            "editor_title" => theme.editor_title,
            "editor_placeholder" => theme.editor_placeholder,
            "code_title" => theme.code_title,
            "code_content" => theme.code_content,
            "heading_h1" => theme.heading_h1,
            "heading_h2" => theme.heading_h2,
            "heading_h3" => theme.heading_h3,
            "italic" => theme.italic,
            "diff_border" => theme.diff_border,
            "diff_header" => theme.diff_header,
            "diff_context" => theme.diff_context,
            "diff_added" => theme.diff_added,
            "diff_removed" => theme.diff_removed,
            "diff_no_changes" => theme.diff_no_changes,
            "tool_running" => theme.tool_running,
            "tool_success" => theme.tool_success,
            "tool_error" => theme.tool_error,
            "picker_title" => theme.picker_title,
            "picker_selected_bg" => theme.picker_selected_bg,
            "picker_selected_fg" => theme.picker_selected_fg,
            "picker_filter" => theme.picker_filter,
            "picker_metadata" => theme.picker_metadata,
            "picker_empty" => theme.picker_empty,
            _ => unreachable!("invalid token already validated: {token}"),
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
