//! Multi-line prompt input component.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::theme::Theme;

/// Multi-line text input for user prompts.
pub struct InputEditor {
    text: String,
    theme: Theme,
}

impl InputEditor {
    pub fn new(text: String) -> Self {
        Self {
            text,
            theme: Theme::default(),
        }
    }

    pub fn empty() -> Self {
        Self {
            text: String::new(),
            theme: Theme::default(),
        }
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Default for InputEditor {
    fn default() -> Self {
        Self::empty()
    }
}

impl Widget for InputEditor {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = &self.theme;
        let block =
            Block::bordered().title(Span::styled(" Input ", Style::default().fg(t.editor_title)));
        let inner = block.inner(area);
        block.render(area, buf);

        let display_text = if self.text.is_empty() {
            Line::from(Span::styled(
                "Type a message...",
                Style::default().fg(t.editor_placeholder),
            ))
        } else {
            Line::from(self.text.as_str())
        };
        Paragraph::new(display_text).render(inner, buf);
    }
}
