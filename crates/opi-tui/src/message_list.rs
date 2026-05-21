//! Streaming conversation display component.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Widget},
};

use crate::{Message, Role};

/// Displays a scrollable list of conversation messages.
pub struct MessageList {
    messages: Vec<Message>,
}

impl MessageList {
    pub fn new(messages: Vec<Message>) -> Self {
        Self { messages }
    }
}

fn role_label(role: &Role) -> (&'static str, Style) {
    match role {
        Role::User => ("You", Style::default().fg(Color::Green)),
        Role::Assistant => (
            "Asst",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Role::System => ("Sys", Style::default().fg(Color::Yellow)),
        Role::Tool => ("Tool", Style::default().fg(Color::Magenta)),
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
            let (label, style) = role_label(&msg.role);
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
