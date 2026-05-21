//! Tool call arguments and status display component.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

use crate::ToolCallStatus;

/// Displays a tool call with name, arguments, and status.
pub struct ToolCallView {
    name: String,
    arguments: String,
    status: ToolCallStatus,
}

impl ToolCallView {
    pub fn new(name: String, arguments: String, status: ToolCallStatus) -> Self {
        Self {
            name,
            arguments,
            status,
        }
    }
}

impl Widget for ToolCallView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let status_style = match &self.status {
            ToolCallStatus::Running => Style::default().fg(Color::Yellow),
            ToolCallStatus::Success => Style::default().fg(Color::Green),
            ToolCallStatus::Error(_) => Style::default().fg(Color::Red),
        };

        let block = Block::bordered().title(format!(" tool: {} ", self.name));
        let inner = block.inner(area);
        block.render(area, buf);

        // Render status with color
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
