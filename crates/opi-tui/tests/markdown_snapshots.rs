//! Snapshot tests for MarkdownView and CodeBlock (task 1.13).
//!
//! DoD: "markdown and fenced code snapshots"

use opi_tui::{CodeBlock, MarkdownView};
use ratatui::{Terminal, backend::TestBackend, widgets::Widget};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn render<W: Widget>(widget: W, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| f.render_widget(widget, f.area()))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// MarkdownView
// ---------------------------------------------------------------------------

#[test]
fn markdown_view_heading_80x24() {
    let md = MarkdownView::new("# Main Title\n\n## Subtitle\n");
    insta::assert_snapshot!("markdown_view_heading_80x24", render(md, 80, 24));
}

#[test]
fn markdown_view_bold_italic_80x24() {
    let md = MarkdownView::new("This is **bold** and *italic* text.\n");
    insta::assert_snapshot!("markdown_view_bold_italic_80x24", render(md, 80, 24));
}

#[test]
fn markdown_view_paragraphs_80x24() {
    let md = MarkdownView::new(
        "First paragraph here.\n\nSecond paragraph with more text.\n\nThird one.\n",
    );
    insta::assert_snapshot!("markdown_view_paragraphs_80x24", render(md, 80, 24));
}

#[test]
fn markdown_view_with_code_block_80x24() {
    let md = MarkdownView::new(
        "Here is some code:\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\nAfter the code block.\n",
    );
    insta::assert_snapshot!("markdown_view_with_code_block_80x24", render(md, 80, 24));
}

#[test]
fn markdown_view_mixed_120x40() {
    let md = MarkdownView::new(
        "# Getting Started\n\nWelcome to **opi**, your AI coding assistant.\n\n## Features\n\n- Fast responses\n- Tool integration\n\n```bash\ncargo install opi\n```\n\nEnjoy!\n",
    );
    insta::assert_snapshot!("markdown_view_mixed_120x40", render(md, 120, 40));
}

// ---------------------------------------------------------------------------
// CodeBlock
// ---------------------------------------------------------------------------

#[test]
fn code_block_rust_80x10() {
    let cb = CodeBlock::new("rust", "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n");
    insta::assert_snapshot!("code_block_rust_80x10", render(cb, 80, 10));
}

#[test]
fn code_block_no_language_80x10() {
    let cb = CodeBlock::new("", "some plain text\ncode here\n");
    insta::assert_snapshot!("code_block_no_language_80x10", render(cb, 80, 10));
}

#[test]
fn code_block_multiline_120x20() {
    let code = "use std::collections::HashMap;\n\nfn main() {\n    let mut map = HashMap::new();\n    map.insert(\"key\", \"value\");\n    println!(\"{:?}\", map);\n}\n";
    let cb = CodeBlock::new("rust", code);
    insta::assert_snapshot!("code_block_multiline_120x20", render(cb, 120, 20));
}
