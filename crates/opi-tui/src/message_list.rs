//! Streaming conversation display component.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Widget},
};

use crate::{DiffView, Message, Role, theme::Theme};

/// Number of terminal rows a single diff message reserves when rendered as a
/// `DiffView`. Chosen to comfortably fit a small hunk plus header.
const DIFF_ROWS: u16 = 10;

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

        let mut y = 0u16;
        for msg in self.messages.iter() {
            if y >= inner.height {
                break;
            }
            if let Some(diff) = &msg.diff {
                let rows = DIFF_ROWS.min(inner.height.saturating_sub(y));
                if rows == 0 {
                    break;
                }
                let rect = Rect {
                    x: inner.x,
                    y: inner.y + y,
                    width: inner.width,
                    height: rows,
                };
                DiffView::new(diff.path.clone(), diff.before.clone(), diff.after.clone())
                    .theme(self.theme.clone())
                    .render(rect, buf);
                y += rows;
                continue;
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
            y += 1;
        }
    }
}
