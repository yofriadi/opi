//! Snapshot tests for DiffView widget (task 2.11).
//!
//! DoD: "DiffView widget for edit/patch visualization, snapshot tests at 80x24 and 120x40"

use opi_tui::DiffView;
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
// DiffView: basic edit
// ---------------------------------------------------------------------------

#[test]
fn diff_view_simple_edit_80x24() {
    let old = "fn main() {\n    println!(\"hello\");\n}\n";
    let new = "fn main() {\n    println!(\"hello, world\");\n}\n";
    let dv = DiffView::new("src/main.rs", old, new);
    insta::assert_snapshot!("diff_view_simple_edit_80x24", render(dv, 80, 24));
}

#[test]
fn diff_view_simple_edit_120x40() {
    let old = "fn main() {\n    println!(\"hello\");\n}\n";
    let new = "fn main() {\n    println!(\"hello, world\");\n}\n";
    let dv = DiffView::new("src/main.rs", old, new);
    insta::assert_snapshot!("diff_view_simple_edit_120x40", render(dv, 120, 40));
}

// ---------------------------------------------------------------------------
// DiffView: additions only
// ---------------------------------------------------------------------------

#[test]
fn diff_view_additions_only_80x24() {
    let old = "fn main() {\n}\n";
    let new = "fn main() {\n    println!(\"hello\");\n}\n";
    let dv = DiffView::new("src/main.rs", old, new);
    insta::assert_snapshot!("diff_view_additions_only_80x24", render(dv, 80, 24));
}

// ---------------------------------------------------------------------------
// DiffView: removals only
// ---------------------------------------------------------------------------

#[test]
fn diff_view_removals_only_80x24() {
    let old = "fn main() {\n    println!(\"hello\");\n    println!(\"world\");\n}\n";
    let new = "fn main() {\n}\n";
    let dv = DiffView::new("src/main.rs", old, new);
    insta::assert_snapshot!("diff_view_removals_only_80x24", render(dv, 80, 24));
}

// ---------------------------------------------------------------------------
// DiffView: no changes
// ---------------------------------------------------------------------------

#[test]
fn diff_view_no_changes_80x24() {
    let content = "fn main() {\n    println!(\"hello\");\n}\n";
    let dv = DiffView::new("src/main.rs", content, content);
    insta::assert_snapshot!("diff_view_no_changes_80x24", render(dv, 80, 24));
}

// ---------------------------------------------------------------------------
// DiffView: multi-hunk diff
// ---------------------------------------------------------------------------

#[test]
fn diff_view_multi_hunk_80x24() {
    let old = "use std::io;\n\nfn main() {\n    println!(\"hello\");\n}\n\nfn helper() {\n    println!(\"help\");\n}\n";
    let new = "use std::io;\nuse std::fs;\n\nfn main() {\n    println!(\"hello, world\");\n}\n\nfn helper() {\n    println!(\"helper\");\n}\n";
    let dv = DiffView::new("src/lib.rs", old, new);
    insta::assert_snapshot!("diff_view_multi_hunk_80x24", render(dv, 80, 24));
}

// ---------------------------------------------------------------------------
// DiffView: large diff (truncation)
// ---------------------------------------------------------------------------

#[test]
fn diff_view_truncation_80x24() {
    let old_lines: Vec<String> = (0..30).map(|i| format!("line {i}")).collect();
    let new_lines: Vec<String> = (0..30).map(|i| format!("line {i} modified")).collect();
    let old = old_lines.join("\n");
    let new = new_lines.join("\n");
    let dv = DiffView::new("big_file.txt", &old, &new);
    insta::assert_snapshot!("diff_view_truncation_80x24", render(dv, 80, 24));
}

#[test]
fn diff_view_truncation_120x40() {
    let old_lines: Vec<String> = (0..30).map(|i| format!("line {i}")).collect();
    let new_lines: Vec<String> = (0..30).map(|i| format!("line {i} modified")).collect();
    let old = old_lines.join("\n");
    let new = new_lines.join("\n");
    let dv = DiffView::new("big_file.txt", &old, &new);
    insta::assert_snapshot!("diff_view_truncation_120x40", render(dv, 120, 40));
}

// ---------------------------------------------------------------------------
// DiffView: empty files
// ---------------------------------------------------------------------------

#[test]
fn diff_view_empty_to_content_80x24() {
    let dv = DiffView::new("new_file.rs", "", "fn main() {}\n");
    insta::assert_snapshot!("diff_view_empty_to_content_80x24", render(dv, 80, 24));
}

#[test]
fn diff_view_content_to_empty_80x24() {
    let dv = DiffView::new("deleted_file.rs", "fn main() {}\n", "");
    insta::assert_snapshot!("diff_view_content_to_empty_80x24", render(dv, 80, 24));
}

// ---------------------------------------------------------------------------
// DiffView: edit diff-preview metadata (Phase 11.5)
// ---------------------------------------------------------------------------

/// Phase 11.5: the ratatui DiffView surface that renders changed-file edits.
/// The edit tool emits before/after string previews and interactive.rs feeds
/// them to DiffView; this snapshots the rendered surface for a representative
/// single-line edit so the changed-file rendering path is covered and reviewed
/// (not auto-accepted).
#[test]
fn phase11_edit_diff_preview_metadata_snapshot() {
    let old = "fn greet() {\n    println!(\"hi\");\n}\n";
    let new = "fn greet() {\n    println!(\"hello\");\n}\n";
    let dv = DiffView::new("src/greet.rs", old, new);
    insta::assert_snapshot!(
        "phase11_edit_diff_preview_metadata_snapshot",
        render(dv, 80, 24)
    );
}
