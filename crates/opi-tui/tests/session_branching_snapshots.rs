//! Snapshot tests for BranchPicker widget rendering (task 4.9).

use opi_tui::branch_picker::{BranchItem, BranchPickerState};
use opi_tui::{BranchPicker, Theme, resolve_theme};
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

fn sample_branches() -> Vec<BranchItem> {
    vec![
        BranchItem {
            tip_id: "e4".into(),
            label: "Main conversation".into(),
            metadata: "5 messages".into(),
            is_active: true,
        },
        BranchItem {
            tip_id: "e2a".into(),
            label: "Branch A".into(),
            metadata: "3 messages".into(),
            is_active: false,
        },
        BranchItem {
            tip_id: "e2b".into(),
            label: "Branch B".into(),
            metadata: "2 messages".into(),
            is_active: false,
        },
    ]
}

fn two_branches() -> Vec<BranchItem> {
    vec![
        BranchItem {
            tip_id: "e3".into(),
            label: "Trunk".into(),
            metadata: "3 msgs".into(),
            is_active: true,
        },
        BranchItem {
            tip_id: "e2b".into(),
            label: "Alt path".into(),
            metadata: "2 msgs".into(),
            is_active: false,
        },
    ]
}

// ---------------------------------------------------------------------------
// 80x24 snapshots
// ---------------------------------------------------------------------------

#[test]
fn branch_picker_basic_80x24() {
    let state = BranchPickerState::new(sample_branches());
    let widget = BranchPicker::new(&state, "Branches").theme(Theme::default());
    insta::assert_snapshot!("branch_picker_basic_80x24", render(widget, 80, 24));
}

#[test]
fn branch_picker_second_selected_80x24() {
    let mut state = BranchPickerState::new(sample_branches());
    state.move_down();
    let widget = BranchPicker::new(&state, "Branches").theme(Theme::default());
    insta::assert_snapshot!(
        "branch_picker_second_selected_80x24",
        render(widget, 80, 24)
    );
}

#[test]
fn branch_picker_two_branches_80x24() {
    let state = BranchPickerState::new(two_branches());
    let widget = BranchPicker::new(&state, "Branches").theme(Theme::default());
    insta::assert_snapshot!("branch_picker_two_branches_80x24", render(widget, 80, 24));
}

#[test]
fn branch_picker_empty_80x24() {
    let state = BranchPickerState::new(vec![]);
    let widget = BranchPicker::new(&state, "Branches").theme(Theme::default());
    insta::assert_snapshot!("branch_picker_empty_80x24", render(widget, 80, 24));
}

#[test]
fn branch_picker_navigate_up_clamps_80x24() {
    let mut state = BranchPickerState::new(sample_branches());
    state.move_up(); // Should stay at 0
    let widget = BranchPicker::new(&state, "Branches").theme(Theme::default());
    insta::assert_snapshot!(
        "branch_picker_navigate_up_clamps_80x24",
        render(widget, 80, 24)
    );
}

#[test]
fn branch_picker_navigate_to_last_80x24() {
    let mut state = BranchPickerState::new(sample_branches());
    state.move_down();
    state.move_down();
    let widget = BranchPicker::new(&state, "Branches").theme(Theme::default());
    insta::assert_snapshot!(
        "branch_picker_navigate_to_last_80x24",
        render(widget, 80, 24)
    );
}

// ---------------------------------------------------------------------------
// 120x40 snapshots
// ---------------------------------------------------------------------------

#[test]
fn branch_picker_basic_120x40() {
    let state = BranchPickerState::new(sample_branches());
    let widget = BranchPicker::new(&state, "Branches").theme(Theme::default());
    insta::assert_snapshot!("branch_picker_basic_120x40", render(widget, 120, 40));
}

#[test]
fn branch_picker_second_selected_120x40() {
    let mut state = BranchPickerState::new(sample_branches());
    state.move_down();
    let widget = BranchPicker::new(&state, "Branches").theme(Theme::default());
    insta::assert_snapshot!(
        "branch_picker_second_selected_120x40",
        render(widget, 120, 40)
    );
}

// ---------------------------------------------------------------------------
// Theme snapshots
// ---------------------------------------------------------------------------

#[test]
fn branch_picker_monokai_theme_80x24() {
    let state = BranchPickerState::new(sample_branches());
    let widget = BranchPicker::new(&state, "Branches").theme(resolve_theme("monokai"));
    insta::assert_snapshot!("branch_picker_monokai_theme_80x24", render(widget, 80, 24));
}
