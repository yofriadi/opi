//! Tool call arguments and status display component.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

use crate::{ToolCallStatus, theme::Theme};

/// Displays a tool call with name, arguments, and status.
pub struct ToolCallView {
    name: String,
    arguments: String,
    status: ToolCallStatus,
    theme: Theme,
}

impl ToolCallView {
    pub fn new(name: String, arguments: String, status: ToolCallStatus) -> Self {
        Self {
            name,
            arguments,
            status,
            theme: Theme::default(),
        }
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for ToolCallView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = &self.theme;
        let status_style = match &self.status {
            ToolCallStatus::Running => Style::default().fg(t.tool_running),
            ToolCallStatus::Success => Style::default().fg(t.tool_success),
            ToolCallStatus::Error(_) => Style::default().fg(t.tool_error),
        };

        let block = Block::bordered().title(format!(" tool: {} ", self.name));
        let inner = block.inner(area);
        block.render(area, buf);

        let lines = vec![
            Line::from(format!("args: {}", self.arguments)),
            Line::from(vec![
                ratatui::text::Span::styled("status: ", Style::default()),
                ratatui::text::Span::styled(format!("{}", self.status), status_style),
            ]),
        ];

        Paragraph::new(lines).render(inner, buf);
    }
}
