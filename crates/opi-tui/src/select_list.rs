//! SelectList widget with fuzzy filtering (task 3.11).
//!
//! Provides a [`SelectListState`] for managing filter/navigation state and a
//! [`fuzzy_match`] function for ranked fuzzy matching. The [`SelectList`]
//! widget renders the state using ratatui.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::Theme;

/// A single selectable item with an ID, display label, and optional metadata.
#[derive(Debug, Clone)]
pub struct SelectItem {
    pub id: String,
    pub display: String,
    pub metadata: String,
}

/// Result of confirming a selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectOutcome {
    Selected(String),
    Cancelled,
}

// ---------------------------------------------------------------------------
// Fuzzy matching
// ---------------------------------------------------------------------------

/// Fuzzy-match `pattern` against `text` (case-insensitive, in-order).
///
/// Returns `Some((score, indices))` on match, `None` otherwise. Higher score
/// means a better match. `indices` marks which character positions in `text`
/// were matched.
pub fn fuzzy_match(pattern: &str, text: &str) -> Option<(i64, Vec<usize>)> {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();

    if p.is_empty() {
        return Some((0, vec![]));
    }
    if p.len() > t.len() {
        return None;
    }

    // Greedy left-to-right match: find each pattern char in text in order.
    let mut indices = Vec::with_capacity(p.len());
    let mut ti = 0;
    for &pc in &p {
        let plc = pc.to_ascii_lowercase();
        let mut found = false;
        while ti < t.len() {
            if t[ti].to_ascii_lowercase() == plc {
                indices.push(ti);
                ti += 1;
                found = true;
                break;
            }
            ti += 1;
        }
        if !found {
            return None;
        }
    }

    let score = compute_score(&indices, t.len());
    Some((score, indices))
}

/// Score a match: higher for prefix matches, consecutive chars, and shorter
/// text. Negative penalties for gaps between matched characters.
fn compute_score(indices: &[usize], text_len: usize) -> i64 {
    let mut score: i64 = 0;

    // Bonus for matching at the start.
    if indices[0] == 0 {
        score += 100;
    }

    // Bonus/penalty for consecutive vs spread matches.
    let mut consecutive = 0i64;
    for window in indices.windows(2) {
        if window[1] == window[0] + 1 {
            consecutive += 1;
            score += 10 + consecutive * 5;
        } else {
            consecutive = 0;
            let gap = (window[1] - window[0]) as i64;
            score -= gap;
        }
    }

    // Small bonus for shorter text (prefer shorter candidates).
    score -= (text_len as i64).min(50);

    score
}

// ---------------------------------------------------------------------------
// Internal: scored item for filtered results
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ScoredItem {
    original_index: usize,
    score: i64,
}

// ---------------------------------------------------------------------------
// SelectListState
// ---------------------------------------------------------------------------

/// State for a fuzzy-filtered selection list.
///
/// Owns the items, tracks the current filter text, the filtered/sorted result
/// set, the selected index within the filtered set, and the scroll offset for
/// rendering.
pub struct SelectListState {
    items: Vec<SelectItem>,
    filter: String,
    /// Filtered results: (original index, score, display, id, metadata).
    filtered: Vec<(usize, i64, String, String, String)>,
    selected: usize,
    scroll_offset: usize,
}

impl SelectListState {
    /// Create a new state with the given items. All items are visible
    /// initially (empty filter).
    pub fn new(items: Vec<SelectItem>) -> Self {
        let filtered = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                (
                    i,
                    0i64,
                    item.display.clone(),
                    item.id.clone(),
                    item.metadata.clone(),
                )
            })
            .collect();
        Self {
            items,
            filter: String::new(),
            filtered,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Number of items currently visible (after filtering).
    pub fn visible_count(&self) -> usize {
        self.filtered.len()
    }

    /// Currently visible items in display order.
    pub fn visible(&self) -> Vec<&SelectItem> {
        self.filtered
            .iter()
            .map(|(idx, _, _, _, _)| &self.items[*idx])
            .collect()
    }

    /// Index of the selected item within the visible list.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Current scroll offset (first visible row index in the filtered list).
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Current filter text.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Update the filter text and recompute the filtered/sorted results.
    /// Resets the selected index to 0 if the current selection is no longer
    /// visible.
    pub fn set_filter(&mut self, filter: impl Into<String>) {
        let filter = filter.into();
        self.filter = filter;

        if self.filter.is_empty() {
            // Show all items in original order.
            self.filtered = self
                .items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    (
                        i,
                        0,
                        item.display.clone(),
                        item.id.clone(),
                        item.metadata.clone(),
                    )
                })
                .collect();
        } else {
            let mut scored: Vec<_> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    // Match against display name and metadata.
                    let d_score = fuzzy_match(&self.filter, &item.display);
                    let m_score = fuzzy_match(&self.filter, &item.metadata);
                    let best = match (d_score, m_score) {
                        (Some(d), Some(m)) => Some(d.max(m)),
                        (Some(d), None) => Some(d),
                        (None, Some(m)) => Some(m),
                        (None, None) => None,
                    };
                    best.map(|(score, _)| ScoredItem {
                        original_index: i,
                        score,
                    })
                })
                .collect();

            // Sort by score descending, then by original index for stability.
            scored.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then_with(|| a.original_index.cmp(&b.original_index))
            });

            self.filtered = scored
                .into_iter()
                .map(|s| {
                    let item = &self.items[s.original_index];
                    (
                        s.original_index,
                        s.score,
                        item.display.clone(),
                        item.id.clone(),
                        item.metadata.clone(),
                    )
                })
                .collect();
        }

        // Clamp selection.
        if self.filtered.is_empty() || self.selected >= self.filtered.len() {
            self.selected = 0;
        }
        self.scroll_offset = 0;
    }

    // -- Navigation --

    /// Move selection down by one. Clamps at the last visible item.
    pub fn move_down(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        if self.selected < self.filtered.len() - 1 {
            self.selected += 1;
        }
    }

    /// Move selection up by one. Stays at 0 if already there.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down by `page_size` items. Clamps at the end.
    pub fn page_down(&mut self, page_size: usize) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = (self.selected + page_size).min(self.filtered.len() - 1);
        self.adjust_scroll(page_size);
    }

    /// Move selection up by `page_size` items. Clamps at 0.
    pub fn page_up(&mut self, page_size: usize) {
        self.selected = self.selected.saturating_sub(page_size);
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    // -- Selection --

    /// Confirm the current selection. Returns the selected item, or `None` if
    /// the visible list is empty.
    pub fn confirm(&self) -> Option<&SelectItem> {
        if self.filtered.is_empty() {
            return None;
        }
        let idx = self.filtered.get(self.selected)?.0;
        Some(&self.items[idx])
    }

    // -- Internal --

    fn adjust_scroll(&mut self, page_size: usize) {
        // Ensure selected is visible within a viewport of `page_size` rows.
        if self.selected >= self.scroll_offset + page_size {
            self.scroll_offset = self.selected - page_size + 1;
        }
    }
}

// ---------------------------------------------------------------------------
// SelectList widget (rendering)
// ---------------------------------------------------------------------------

/// Ratatui widget that renders a [`SelectListState`] as a bordered list with a
/// filter prompt, selection highlight, and metadata column.
pub struct SelectList<'a> {
    state: &'a SelectListState,
    theme: Theme,
    title: &'a str,
}

impl<'a> SelectList<'a> {
    pub fn new(state: &'a SelectListState, title: &'a str) -> Self {
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

impl Widget for SelectList<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let t = &self.theme;

        // Layout: top line = filter prompt, remaining = items.
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            format!(" {} ", self.title),
            Style::default().fg(t.picker_title),
        ));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        // Filter prompt line (first row).
        let filter_style = Style::default().fg(t.picker_filter);
        let filter_line = if self.state.filter().is_empty() {
            Line::from(Span::styled("Type to filter...", filter_style))
        } else {
            Line::from(vec![
                Span::styled("> ", filter_style),
                Span::styled(self.state.filter().to_string(), Style::default()),
            ])
        };

        let viewport_height = inner.height.saturating_sub(1) as usize;

        let mut lines: Vec<Line> = vec![filter_line];

        if self.state.visible_count() == 0 {
            lines.push(Line::from(Span::styled(
                "No matches",
                Style::default().fg(t.picker_empty),
            )));
        } else {
            let visible = &self.state.filtered;
            let start = self.state.scroll_offset();
            let end = (start + viewport_height).min(visible.len());

            for (vi, entry) in visible.iter().enumerate().skip(start).take(end - start) {
                let (_, _, display, _, metadata) = entry;
                let is_selected = vi == self.state.selected_index();

                let style = if is_selected {
                    Style::default()
                        .fg(t.picker_selected_fg)
                        .bg(t.picker_selected_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let meta_style = Style::default().fg(t.picker_metadata);

                let display_text = truncate_str(display, inner.width as usize);
                let meta_text = truncate_str(metadata, inner.width as usize);
                let display_width = unicode_display_width(&display_text);
                let meta_width = unicode_display_width(&meta_text);
                let padding = inner.width as usize;
                let gap = padding.saturating_sub(display_width + 1 + meta_width);

                let mut spans = vec![
                    Span::styled(display_text, style),
                    Span::raw(" ".repeat(gap)),
                    Span::styled(meta_text, meta_style),
                ];

                if is_selected {
                    // Prefix with pointer.
                    spans.insert(0, Span::styled("> ", style));
                }

                lines.push(Line::from(spans));
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}

/// Truncate a string to fit within `max_width` display columns.
fn truncate_str(s: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = unicode_display_width_char(ch);
        if width + cw > max_width {
            break;
        }
        result.push(ch);
        width += cw;
    }
    result
}

fn unicode_display_width(s: &str) -> usize {
    s.chars().map(unicode_display_width_char).sum()
}

fn unicode_display_width_char(ch: char) -> usize {
    match ch {
        '\t' => 4,
        c if c.is_control() => 0,
        _ => 1,
    }
}
