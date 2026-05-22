//! Markdown and code rendering components.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Widget},
};

use crate::theme::Theme;

/// Renders a fenced code block with optional language label.
pub struct CodeBlock {
    language: String,
    code: String,
    theme: Theme,
}

impl CodeBlock {
    pub fn new(language: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            code: code.into(),
            theme: Theme::default(),
        }
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl Widget for CodeBlock {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = &self.theme;
        let title = if self.language.is_empty() {
            " code ".to_owned()
        } else {
            format!(" {} ", self.language)
        };
        let block = Block::bordered()
            .title(title)
            .style(Style::default().fg(t.code_title));
        let inner = block.inner(area);
        block.render(area, buf);

        for (i, line) in self.code.lines().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            Line::from(Span::styled(
                line.to_owned(),
                Style::default().fg(t.code_content),
            ))
            .render(Rect { y, ..inner }, buf);
        }
    }
}

// ---------------------------------------------------------------------------
// Inline span parsing
// ---------------------------------------------------------------------------

/// Parse a single line of markdown inline content into styled spans.
fn parse_inline(line: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = line.char_indices().peekable();
    let mut plain_start = 0;

    while let Some((i, ch)) = chars.next() {
        if ch == '*' {
            // peek ahead to see if this is ** or *
            let next = chars.peek().map(|&(j, c)| (j, c));

            if let Some((j, '*')) = next {
                // **bold** — consume the second *
                chars.next();
                let bold_start = j + 1;
                // find closing **
                let mut closed = false;
                while let Some((k, c)) = chars.next() {
                    if c == '*'
                        && let Some(&(_, '*')) = chars.peek()
                    {
                        chars.next();
                        // emit plain before bold
                        if plain_start < i {
                            spans.push(Span::raw(line[plain_start..i].to_owned()));
                        }
                        spans.push(Span::styled(
                            line[bold_start..k].to_owned(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ));
                        plain_start = k + 2;
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    // unmatched **, treat as literal
                }
            } else {
                // *italic*
                let italic_start = i + 1;
                let mut closed = false;
                for (k, c) in chars.by_ref() {
                    if c == '*' {
                        if plain_start < i {
                            spans.push(Span::raw(line[plain_start..i].to_owned()));
                        }
                        spans.push(Span::styled(
                            line[italic_start..k].to_owned(),
                            Style::default().fg(theme.italic),
                        ));
                        plain_start = k + 1;
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    // unmatched *, treat as literal
                }
            }
        }
    }

    if plain_start < line.len() {
        spans.push(Span::raw(line[plain_start..].to_owned()));
    }

    if spans.is_empty() {
        spans.push(Span::raw(line.to_owned()));
    }

    spans
}

// ---------------------------------------------------------------------------
// MarkdownView
// ---------------------------------------------------------------------------

/// Renders markdown text as a ratatui widget.
///
/// Supports headings (`#`, `##`, `###`), **bold**, *italic*, paragraphs,
/// and fenced code blocks (``` ``` ```). No syntax highlighting in Phase 1.
pub struct MarkdownView {
    markdown: String,
    theme: Theme,
}

impl MarkdownView {
    pub fn new(markdown: impl Into<String>) -> Self {
        Self {
            markdown: markdown.into(),
            theme: Theme::default(),
        }
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

/// Parsed element from markdown input.
enum MdElement {
    Heading { level: u8, text: String },
    Paragraph(String),
    CodeBlock { language: String, code: String },
}

fn parse_markdown(input: &str) -> Vec<MdElement> {
    let mut elements = Vec::new();
    let mut lines = input.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Fenced code block
        if trimmed.starts_with("```") {
            let language = trimmed.trim_start_matches('`').trim().to_owned();
            let mut code_lines = Vec::new();
            for code_line in lines.by_ref() {
                if code_line.trim().starts_with("```") {
                    break;
                }
                code_lines.push(code_line);
            }
            elements.push(MdElement::CodeBlock {
                language,
                code: code_lines.join("\n"),
            });
            continue;
        }

        // Heading
        let level = trimmed.bytes().take_while(|&b| b == b'#').count() as u8;
        if level > 0 {
            let text = trimmed[level as usize..].trim().to_owned();
            elements.push(MdElement::Heading { level, text });
            continue;
        }

        // Regular text line — accumulate consecutive non-blank lines as paragraph
        let mut para_lines = vec![trimmed.to_owned()];
        while let Some(peek) = lines.peek() {
            if peek.trim().is_empty()
                || peek.trim().starts_with('#')
                || peek.trim().starts_with("```")
            {
                break;
            }
            if let Some(next_line) = lines.next() {
                para_lines.push(next_line.trim().to_owned());
            }
        }
        elements.push(MdElement::Paragraph(para_lines.join(" ")));
    }

    elements
}

impl Widget for MarkdownView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = &self.theme;
        let elements = parse_markdown(&self.markdown);
        let mut y = area.y;

        for elem in &elements {
            match elem {
                MdElement::Heading { level, text } => {
                    if y >= area.y + area.height {
                        break;
                    }
                    let style = match level {
                        1 => Style::default()
                            .fg(theme.heading_h1)
                            .add_modifier(Modifier::BOLD),
                        2 => Style::default()
                            .fg(theme.heading_h2)
                            .add_modifier(Modifier::BOLD),
                        _ => Style::default()
                            .fg(theme.heading_h3)
                            .add_modifier(Modifier::BOLD),
                    };
                    Line::from(Span::styled(text.clone(), style)).render(Rect { y, ..area }, buf);
                    y += 1;
                    // blank line after heading
                    y += 1;
                }
                MdElement::Paragraph(text) => {
                    // Word-wrap the paragraph within the area width
                    let words: Vec<&str> = text.split_whitespace().collect();
                    let mut current = String::new();
                    let max_w = area.width as usize;

                    for word in words {
                        if current.is_empty() {
                            current = word.to_owned();
                        } else if current.len() + 1 + word.len() <= max_w {
                            current.push(' ');
                            current.push_str(word);
                        } else {
                            if y < area.y + area.height {
                                Line::from(parse_inline(&current, theme))
                                    .render(Rect { y, ..area }, buf);
                                y += 1;
                            }
                            current = word.to_owned();
                        }
                    }
                    if !current.is_empty() && y < area.y + area.height {
                        Line::from(parse_inline(&current, theme)).render(Rect { y, ..area }, buf);
                        y += 1;
                    }
                    // blank line after paragraph
                    y += 1;
                }
                MdElement::CodeBlock { language, code } => {
                    let code_lines: Vec<&str> = code.lines().collect();
                    let needed = (code_lines.len() + 2) as u16; // +2 for border top/bottom
                    if y + needed > area.y + area.height {
                        break;
                    }
                    let cb_area = Rect {
                        y,
                        height: needed,
                        ..area
                    };
                    CodeBlock::new(language.clone(), code.clone())
                        .theme(theme.clone())
                        .render(cb_area, buf);
                    y += needed;
                    // blank line after code block
                    y += 1;
                }
            }
        }
    }
}
