//! Shell layout composing all TUI components.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::Widget,
};

use crate::{
    AppState, InputEditor, Message, MessageList, StatusBar, ToolCallStatus, ToolCallView,
    theme::Theme,
};

/// Top-level TUI shell composing all Phase 1 components.
pub struct Shell {
    messages: Vec<Message>,
    input_text: String,
    model: String,
    state: AppState,
    token_count: Option<u64>,
    cost_usd: Option<f64>,
    active_tool: Option<ToolCallViewData>,
    theme: Theme,
}

struct ToolCallViewData {
    name: String,
    arguments: String,
    status: ToolCallStatus,
}

impl Shell {
    pub fn new(model: String) -> Self {
        Self {
            messages: Vec::new(),
            input_text: String::new(),
            model,
            state: AppState::Idle,
            token_count: None,
            cost_usd: None,
            active_tool: None,
            theme: Theme::default(),
        }
    }

    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    pub fn input_text(mut self, text: String) -> Self {
        self.input_text = text;
        self
    }

    pub fn state(mut self, state: AppState) -> Self {
        self.state = state;
        self
    }

    pub fn token_count(mut self, count: u64) -> Self {
        self.token_count = Some(count);
        self
    }

    pub fn cost_usd(mut self, cost: f64) -> Self {
        self.cost_usd = Some(cost);
        self
    }

    pub fn active_tool(mut self, name: String, arguments: String, status: ToolCallStatus) -> Self {
        self.active_tool = Some(ToolCallViewData {
            name,
            arguments,
            status,
        });
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for Shell {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let input_height: u16 = 3;
        let status_height: u16 = 1;
        let tool_height: u16 = if self.active_tool.is_some() { 5 } else { 0 };

        let mut constraints = vec![Constraint::Min(1)];
        if tool_height > 0 {
            constraints.push(Constraint::Length(tool_height));
        }
        constraints.push(Constraint::Length(status_height));
        constraints.push(Constraint::Length(input_height));

        let chunks = Layout::vertical(constraints).split(area);
        let mut ci = 0;
        let theme = &self.theme;

        MessageList::new(self.messages)
            .theme(theme.clone())
            .render(chunks[ci], buf);
        ci += 1;

        if let Some(tool) = self.active_tool {
            ToolCallView::new(tool.name, tool.arguments, tool.status)
                .theme(theme.clone())
                .render(chunks[ci], buf);
            ci += 1;
        }

        let mut status =
            StatusBar::new(self.model, self.state, self.token_count).theme(theme.clone());
        if let Some(cost) = self.cost_usd {
            status = status.cost_usd(cost);
        }
        status.render(chunks[ci], buf);
        ci += 1;

        InputEditor::new(self.input_text)
            .theme(theme.clone())
            .render(chunks[ci], buf);
    }
}
