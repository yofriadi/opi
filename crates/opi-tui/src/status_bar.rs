//! Status bar component showing model, state, and token info.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Widget},
};

use crate::AppState;

/// Single-line status bar.
pub struct StatusBar {
    model: String,
    state: AppState,
    token_count: Option<u64>,
}

impl StatusBar {
    pub fn new(model: String, state: AppState, token_count: Option<u64>) -> Self {
        Self {
            model,
            state,
            token_count,
        }
    }
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(Color::DarkGray))
            .render(area, buf);

        let state_style = match self.state {
            AppState::Idle => Style::default().fg(Color::White),
            AppState::Thinking => Style::default().fg(Color::Yellow),
            AppState::Streaming => Style::default().fg(Color::Green),
            AppState::ToolExecuting => Style::default().fg(Color::Magenta),
        };

        let mut spans = vec![
            Span::styled(
                format!(" {} ", self.model),
                Style::default().fg(Color::White),
            ),
            Span::styled(format!("[{}]", self.state), state_style),
        ];

        if let Some(count) = self.token_count {
            spans.push(Span::styled(
                format!(" | {count} tokens"),
                Style::default().fg(Color::DarkGray),
            ));
        }

        Line::from(spans).render(area, buf);
    }
}
