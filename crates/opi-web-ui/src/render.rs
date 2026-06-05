//! HTML rendering trait and utilities.
//!
//! Provides the [`Render`] trait for HTML output and helper functions
//! for safe HTML construction.
//!
//! **Unstable 0.x API** — these types may change between minor versions.

/// Escape special HTML characters to prevent XSS in rendered output.
pub fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Trait for rendering a component to an HTML string.
pub trait Render {
    /// Render this component to an HTML string.
    fn render_html(&self) -> String;
}
