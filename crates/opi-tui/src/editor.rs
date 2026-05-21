//! Multi-line prompt input component.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

/// Multi-line text input for user prompts.
pub struct InputEditor {
    text: String,
}

impl InputEditor {
    pub fn new(text: String) -> Self {
        Self { text }
    }

    pub fn empty() -> Self {
        Self {
            text: String::new(),
        }
    }
}

impl Default for InputEditor {
    fn default() -> Self {
        Self::empty()
    }
}

impl Widget for InputEditor {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block =
            Block::bordered().title(Span::styled(" Input ", Style::default().fg(Color::Yellow)));
        let inner = block.inner(area);
        block.render(area, buf);

        let display_text = if self.text.is_empty() {
            Line::from(Span::styled(
                "Type a message...",
                Style::default().fg(Color::DarkGray),
            ))
        } else {
            Line::from(self.text.as_str())
        };
        Paragraph::new(display_text).render(inner, buf);
    }
}
