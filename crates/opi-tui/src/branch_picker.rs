//! Branch picker widget for session branch selection (task 4.9).
//!
//! Renders a session's branches as a selectable list with navigation support.
//! The [`BranchPickerState`] tracks the selected branch index, and the
//! [`BranchPicker`] widget renders the list using ratatui.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::Theme;

/// A single branch item for display in the picker.
#[derive(Debug, Clone)]
pub struct BranchItem {
    /// Entry ID at the tip of this branch.
    pub tip_id: String,
    /// Display label (e.g. "Branch 1" or summary text).
    pub label: String,
    /// Metadata string (e.g. "3 messages" or timestamp).
    pub metadata: String,
    /// Whether this is the currently active branch.
    pub is_active: bool,
}

/// Result of confirming a branch selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchPickerOutcome {
    /// User selected a branch by tip entry ID.
    Selected(String),
    /// User cancelled the picker.
    Cancelled,
}

/// State for the branch picker overlay.
#[derive(Debug, Clone)]
pub struct BranchPickerState {
    items: Vec<BranchItem>,
    selected: usize,
    scroll_offset: usize,
}

impl BranchPickerState {
    /// Create a new picker state with the given branch items.
    pub fn new(items: Vec<BranchItem>) -> Self {
        // Start with the active branch selected, if any.
        let selected = items.iter().position(|i| i.is_active).unwrap_or(0);
        Self {
            items,
            selected,
            scroll_offset: 0,
        }
    }

    /// Number of branch items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Current selected index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Current scroll offset.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Move selection up by one.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.adjust_scroll_up();
        }
    }

    /// Move selection down by one.
    pub fn move_down(&mut self) {
        if !self.items.is_empty() && self.selected < self.items.len() - 1 {
            self.selected += 1;
            // Adjust scroll for a reasonable default viewport (20 rows).
            self.adjust_scroll_down(20);
        }
    }

    /// Confirm the current selection.
    pub fn confirm(&self) -> Option<&BranchItem> {
        self.items.get(self.selected)
    }

    fn adjust_scroll_up(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
    }

    fn adjust_scroll_down(&mut self, viewport_height: usize) {
        if self.selected >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected - viewport_height + 1;
        }
    }
}

/// Ratatui widget that renders a [`BranchPickerState`].
pub struct BranchPicker<'a> {
    state: &'a BranchPickerState,
    theme: Theme,
    title: &'a str,
}

impl<'a> BranchPicker<'a> {
    pub fn new(state: &'a BranchPickerState, title: &'a str) -> Self {
        Self {
            state,
            theme: Theme::default(),
            title,
        }
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for BranchPicker<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let t = &self.theme;

        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            format!(" {} ", self.title),
            Style::default().fg(t.picker_title),
        ));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || self.state.is_empty() {
            if inner.height > 0 {
                let line = Line::from(Span::styled(
                    "No branches",
                    Style::default().fg(t.picker_empty),
                ));
                let p = Paragraph::new(line);
                p.render(inner, buf);
            }
            return;
        }

        let viewport_height = inner.height as usize;
        // Adjust scroll to keep selected visible
        let state_scroll = self.state.scroll_offset;
        let selected = self.state.selected_index();

        let start = if selected >= state_scroll + viewport_height {
            selected - viewport_height + 1
        } else {
            state_scroll
        };
        let end = (start + viewport_height).min(self.state.len());

        let mut lines: Vec<Line> = Vec::new();

        for (vi, item) in self
            .state
            .items
            .iter()
            .enumerate()
            .skip(start)
            .take(end - start)
        {
            let is_selected = vi == selected;

            let style = if is_selected {
                Style::default()
                    .fg(t.picker_selected_fg)
                    .bg(t.picker_selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let active_marker = if item.is_active { " * " } else { "   " };
            let meta_style = Style::default().fg(t.picker_metadata);

            let label_text = truncate_to_width(&item.label, inner.width as usize);
            let meta_text = truncate_to_width(&item.metadata, inner.width as usize);
            let label_w = unicode_display_width(&label_text);
            let meta_w = unicode_display_width(&meta_text);
            let padding = inner.width as usize;
            let gap = padding.saturating_sub(label_w + 3 + meta_w);

            let mut spans = vec![
                Span::styled(active_marker.to_string(), meta_style),
                Span::styled(label_text, style),
                Span::raw(" ".repeat(gap)),
                Span::styled(meta_text, meta_style),
            ];

            if is_selected {
                spans.insert(0, Span::styled("> ", style));
            }

            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}

fn truncate_to_width(s: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if width + cw > max_width {
            break;
        }
        result.push(ch);
        width += cw;
    }
    result
}

fn unicode_display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_width_counts_cjk_as_double_width() {
        assert_eq!(unicode_display_width("分支"), 4);
        assert_eq!(truncate_to_width("分支A", 4), "分支");
    }
}
