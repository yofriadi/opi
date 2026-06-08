//! Snapshot tests for SelectList widget rendering (task 3.11).

use opi_tui::select_list::{SelectItem, SelectListState};
use opi_tui::{Theme, resolve_theme};
use ratatui::{Terminal, backend::TestBackend, widgets::Widget};

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

fn sample_items() -> Vec<SelectItem> {
    vec![
        SelectItem {
            id: "anthropic:claude-sonnet-4-5-20250514".into(),
            display: "Claude Sonnet 4.5".into(),
            metadata: "Anthropic".into(),
        },
        SelectItem {
            id: "anthropic:claude-opus-4-20250514".into(),
            display: "Claude Opus 4".into(),
            metadata: "Anthropic".into(),
        },
        SelectItem {
            id: "openai:gpt-4o".into(),
            display: "GPT-4o".into(),
            metadata: "OpenAI".into(),
        },
        SelectItem {
            id: "openai:gpt-4o-mini".into(),
            display: "GPT-4o Mini".into(),
            metadata: "OpenAI".into(),
        },
        SelectItem {
            id: "gemini:gemini-2.5-flash".into(),
            display: "Gemini 2.5 Flash".into(),
            metadata: "Google".into(),
        },
    ]
}

fn cjk_items() -> Vec<SelectItem> {
    vec![
        SelectItem {
            id: "session-cn-root".into(),
            display: "主分支 会话".into(),
            metadata: "12 条消息".into(),
        },
        SelectItem {
            id: "session-cn-alt".into(),
            display: "修复路径".into(),
            metadata: "8 条消息".into(),
        },
        SelectItem {
            id: "session-jp-alt".into(),
            display: "検証ブランチ".into(),
            metadata: "3 件".into(),
        },
    ]
}

// ---------------------------------------------------------------------------
// 80x24 snapshots
// ---------------------------------------------------------------------------

#[test]
fn select_list_basic_80x24() {
    let state = SelectListState::new(sample_items());
    let widget = opi_tui::select_list::SelectList::new(&state, "Models").theme(Theme::default());
    insta::assert_snapshot!("select_list_basic_80x24", render(widget, 80, 24));
}

#[test]
fn select_list_with_filter_80x24() {
    let mut state = SelectListState::new(sample_items());
    state.set_filter("gpt");
    let widget = opi_tui::select_list::SelectList::new(&state, "Models").theme(Theme::default());
    insta::assert_snapshot!("select_list_with_filter_80x24", render(widget, 80, 24));
}

#[test]
fn select_list_empty_results_80x24() {
    let mut state = SelectListState::new(sample_items());
    state.set_filter("zzzzz");
    let widget = opi_tui::select_list::SelectList::new(&state, "Models").theme(Theme::default());
    insta::assert_snapshot!("select_list_empty_results_80x24", render(widget, 80, 24));
}

#[test]
fn select_list_second_selected_80x24() {
    let mut state = SelectListState::new(sample_items());
    state.move_down();
    let widget = opi_tui::select_list::SelectList::new(&state, "Models").theme(Theme::default());
    insta::assert_snapshot!("select_list_second_selected_80x24", render(widget, 80, 24));
}

#[test]
fn select_list_empty_items_80x24() {
    let state = SelectListState::new(vec![]);
    let widget = opi_tui::select_list::SelectList::new(&state, "Sessions").theme(Theme::default());
    insta::assert_snapshot!("select_list_empty_items_80x24", render(widget, 80, 24));
}

#[test]
fn select_list_cjk_labels_40x10() {
    let state = SelectListState::new(cjk_items());
    let widget = opi_tui::select_list::SelectList::new(&state, "Sessions").theme(Theme::default());
    insta::assert_snapshot!("select_list_cjk_labels_40x10", render(widget, 40, 10));
}

// ---------------------------------------------------------------------------
// 120x40 snapshots
// ---------------------------------------------------------------------------

#[test]
fn select_list_basic_120x40() {
    let state = SelectListState::new(sample_items());
    let widget = opi_tui::select_list::SelectList::new(&state, "Models").theme(Theme::default());
    insta::assert_snapshot!("select_list_basic_120x40", render(widget, 120, 40));
}

#[test]
fn select_list_with_filter_120x40() {
    let mut state = SelectListState::new(sample_items());
    state.set_filter("gpt");
    let widget = opi_tui::select_list::SelectList::new(&state, "Models").theme(Theme::default());
    insta::assert_snapshot!("select_list_with_filter_120x40", render(widget, 120, 40));
}

// ---------------------------------------------------------------------------
// Theme snapshots
// ---------------------------------------------------------------------------

#[test]
fn select_list_monokai_theme_80x24() {
    let state = SelectListState::new(sample_items());
    let widget =
        opi_tui::select_list::SelectList::new(&state, "Models").theme(resolve_theme("monokai"));
    insta::assert_snapshot!("select_list_monokai_theme_80x24", render(widget, 80, 24));
}
