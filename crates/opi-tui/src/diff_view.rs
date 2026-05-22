//! Diff view widget for edit/patch visualization.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Widget},
};

use crate::theme::Theme;

/// A single line in a unified diff.
#[derive(Debug, Clone)]
enum DiffLine {
    Context {
        old_num: usize,
        new_num: usize,
        text: String,
    },
    Removed {
        old_num: usize,
        text: String,
    },
    Added {
        new_num: usize,
        text: String,
    },
}

/// A contiguous group of diff lines with shared context.
#[derive(Debug, Clone)]
struct Hunk {
    old_start: usize,
    new_start: usize,
    lines: Vec<DiffLine>,
}

/// Displays a unified diff between old and new file content.
pub struct DiffView {
    path: String,
    old: String,
    new: String,
    theme: Theme,
}

impl DiffView {
    pub fn new(path: impl Into<String>, old: impl Into<String>, new: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            old: old.into(),
            new: new.into(),
            theme: Theme::default(),
        }
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Compute a unified diff between old and new text lines.
/// Returns a list of hunks with 3 lines of context.
fn compute_diff(old_lines: &[&str], new_lines: &[&str]) -> Vec<Hunk> {
    // LCS-based diff using a DP table
    let m = old_lines.len();
    let n = new_lines.len();

    // Build LCS length table.
    // NOTE: O(m*n) space — fine for typical file edits visible in a terminal.
    // For large-file diffs, switch to a two-row rolling DP.
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            if old_lines[i] == new_lines[j] {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    // Backtrack to produce edit script
    let mut ops: Vec<DiffLine> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    let mut old_num = 1usize;
    let mut new_num = 1usize;

    while i < m || j < n {
        if i < m && j < n && old_lines[i] == new_lines[j] {
            ops.push(DiffLine::Context {
                old_num,
                new_num,
                text: old_lines[i].to_owned(),
            });
            old_num += 1;
            new_num += 1;
            i += 1;
            j += 1;
        } else if i < m && (j == n || dp[i + 1][j] >= dp[i][j + 1]) {
            ops.push(DiffLine::Removed {
                old_num,
                text: old_lines[i].to_owned(),
            });
            old_num += 1;
            i += 1;
        } else {
            ops.push(DiffLine::Added {
                new_num,
                text: new_lines[j].to_owned(),
            });
            new_num += 1;
            j += 1;
        }
    }

    if ops.is_empty() {
        return Vec::new();
    }

    // Split into hunks with 3 lines of context
    let context = 3;
    let mut hunks: Vec<Hunk> = Vec::new();

    // Find first non-context line to identify change regions
    let is_change = |idx: usize| -> bool { !matches!(ops[idx], DiffLine::Context { .. }) };

    // Find all change regions and group with context
    let mut change_ranges: Vec<(usize, usize)> = Vec::new();
    let mut in_change = false;
    let mut change_start = 0;

    for idx in 0..ops.len() {
        if is_change(idx) {
            if !in_change {
                change_start = idx;
                in_change = true;
            }
        } else if in_change {
            change_ranges.push((change_start, idx));
            in_change = false;
        }
    }
    if in_change {
        change_ranges.push((change_start, ops.len()));
    }

    if change_ranges.is_empty() {
        // No changes at all
        return Vec::new();
    }

    // Merge overlapping/adjacent ranges with context
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in &change_ranges {
        let ctx_start = start.saturating_sub(context);
        let ctx_end = (end + context).min(ops.len());
        if let Some(last) = merged.last_mut()
            && ctx_start <= last.1
        {
            last.1 = ctx_end;
            continue;
        }
        merged.push((ctx_start, ctx_end));
    }

    for (start, end) in merged {
        let hunk_lines: Vec<DiffLine> = ops[start..end].to_vec();
        // Derive hunk start line numbers from the first line.
        // Non-context starts only happen at file beginning where the
        // opposite side's start is 1 (no prior context consumed).
        let (os, ns) = match hunk_lines.first() {
            Some(DiffLine::Context {
                old_num: o,
                new_num: n,
                ..
            }) => (*o, *n),
            Some(DiffLine::Removed { old_num: o, .. }) => (*o, 1),
            Some(DiffLine::Added { new_num: n, .. }) => (1, *n),
            None => continue,
        };
        hunks.push(Hunk {
            old_start: os,
            new_start: ns,
            lines: hunk_lines,
        });
    }

    hunks
}

impl Widget for DiffView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = &self.theme;
        let old_lines: Vec<&str> = if self.old.is_empty() {
            Vec::new()
        } else {
            self.old.lines().collect()
        };
        let new_lines: Vec<&str> = if self.new.is_empty() {
            Vec::new()
        } else {
            self.new.lines().collect()
        };

        let hunks = compute_diff(&old_lines, &new_lines);

        let block = Block::bordered()
            .title(format!(" diff: {} ", self.path))
            .style(Style::default().fg(t.diff_border));
        let inner = block.inner(area);
        block.render(area, buf);

        let mut y = inner.y;
        let max_y = inner.y + inner.height;

        if hunks.is_empty() {
            // No changes
            if y < max_y {
                Line::from(Span::styled(
                    "(no changes)",
                    Style::default().fg(t.diff_no_changes),
                ))
                .render(Rect { y, ..inner }, buf);
            }
            return;
        }

        for hunk in &hunks {
            if y >= max_y {
                break;
            }

            // Hunk header: @@ -old_start,count +new_start,count @@
            let old_count = hunk
                .lines
                .iter()
                .filter(|l| matches!(l, DiffLine::Context { .. } | DiffLine::Removed { .. }))
                .count();
            let new_count = hunk
                .lines
                .iter()
                .filter(|l| matches!(l, DiffLine::Context { .. } | DiffLine::Added { .. }))
                .count();
            let header = format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, old_count, hunk.new_start, new_count
            );
            Line::from(Span::styled(
                header,
                Style::default()
                    .fg(t.diff_header)
                    .add_modifier(Modifier::BOLD),
            ))
            .render(Rect { y, ..inner }, buf);
            y += 1;

            let num_width = {
                let max_old =
                    hunk.lines
                        .iter()
                        .filter_map(|l| match l {
                            DiffLine::Context { old_num, .. }
                            | DiffLine::Removed { old_num, .. } => Some(*old_num),
                            _ => None,
                        })
                        .max()
                        .unwrap_or(0);
                let max_new = hunk
                    .lines
                    .iter()
                    .filter_map(|l| match l {
                        DiffLine::Context { new_num, .. } | DiffLine::Added { new_num, .. } => {
                            Some(*new_num)
                        }
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                max_old.max(max_new).to_string().len().max(1)
            };

            for line in &hunk.lines {
                if y >= max_y {
                    break;
                }
                match line {
                    DiffLine::Context {
                        old_num,
                        new_num,
                        text,
                    } => {
                        let prefix = format!(
                            " {:>width$} {:>width$} │ ",
                            old_num,
                            new_num,
                            width = num_width
                        );
                        Line::from(Span::styled(
                            format!("{prefix}{text}"),
                            Style::default().fg(t.diff_context),
                        ))
                        .render(Rect { y, ..inner }, buf);
                    }
                    DiffLine::Removed { old_num, text } => {
                        let prefix =
                            format!(" {:>width$} {:>width$} │ ", old_num, "", width = num_width);
                        Line::from(vec![
                            Span::styled(prefix, Style::default().fg(t.diff_removed)),
                            Span::styled(format!("-{text}"), Style::default().fg(t.diff_removed)),
                        ])
                        .render(Rect { y, ..inner }, buf);
                    }
                    DiffLine::Added { new_num, text } => {
                        let prefix =
                            format!(" {:>width$} {:>width$} │ ", "", new_num, width = num_width);
                        Line::from(vec![
                            Span::styled(prefix, Style::default().fg(t.diff_added)),
                            Span::styled(format!("+{text}"), Style::default().fg(t.diff_added)),
                        ])
                        .render(Rect { y, ..inner }, buf);
                    }
                }
                y += 1;
            }

            // Blank line between hunks
            if y < max_y {
                y += 1;
            }
        }
    }
}
