//! Streaming conversation display component.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Widget},
};

use crate::{Message, Role, theme::Theme};

/// Displays a scrollable list of conversation messages.
pub struct MessageList {
    messages: Vec<Message>,
    theme: Theme,
}

impl MessageList {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            theme: Theme::default(),
        }
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl MessageList {
    fn role_label(&self, role: &Role) -> (&'static str, Style) {
        let t = &self.theme;
        match role {
            Role::User => ("You", Style::default().fg(t.role_user)),
            Role::Assistant => (
                "Asst",
                Style::default()
                    .fg(t.role_assistant)
                    .add_modifier(Modifier::BOLD),
            ),
            Role::System => ("Sys", Style::default().fg(t.role_system)),
            Role::Tool => ("Tool", Style::default().fg(t.role_tool)),
        }
    }
}

impl Widget for MessageList {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title(" Messages ");
        let inner = block.inner(area);
        block.render(area, buf);

        for (i, msg) in self.messages.iter().enumerate() {
            let y = i as u16;
            if y >= inner.height {
                break;
            }
            let (label, style) = self.role_label(&msg.role);
            let line = Line::from(vec![
                Span::styled(format!("{label}: "), style),
                Span::raw(&msg.content),
            ]);
            line.render(
                Rect {
                    y: inner.y + y,
                    ..inner
                },
                buf,
            );
        }
    }
}
