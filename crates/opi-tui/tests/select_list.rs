//! SelectList widget tests (task 3.11).
//!
//! Covers fuzzy matching, state management, navigation, selection stability,
//! empty state, and large-list handling.

use opi_tui::select_list::{SelectItem, SelectListState, fuzzy_match};

// ---------------------------------------------------------------------------
// fuzzy_match
// ---------------------------------------------------------------------------

#[test]
fn fuzzy_match_exact() {
    let (_score, indices) = fuzzy_match("hello", "hello").unwrap();
    assert!(!indices.is_empty());
    assert_eq!(indices, [0, 1, 2, 3, 4]);
}

#[test]
fn fuzzy_match_case_insensitive() {
    let result = fuzzy_match("HELLO", "hello");
    assert!(result.is_some());
    let result = fuzzy_match("hello", "HELLO");
    assert!(result.is_some());
}

#[test]
fn fuzzy_match_prefix_scores_highest() {
    let prefix = fuzzy_match("he", "hello").unwrap().0;
    let middle = fuzzy_match("ll", "hello").unwrap().0;
    assert!(
        prefix > middle,
        "prefix match should score higher: {prefix} vs {middle}"
    );
}

#[test]
fn fuzzy_match_consecutive_scores_higher() {
    let consecutive = fuzzy_match("hel", "hello").unwrap().0;
    let spread = fuzzy_match("hlo", "hello").unwrap().0;
    assert!(
        consecutive > spread,
        "consecutive should score higher: {consecutive} vs {spread}"
    );
}

#[test]
fn fuzzy_match_substring() {
    assert!(fuzzy_match("llo", "hello").is_some());
}

#[test]
fn fuzzy_match_chars_out_of_order_fails() {
    assert!(fuzzy_match("olleh", "hello").is_none());
}

#[test]
fn fuzzy_match_empty_pattern_matches_everything() {
    assert!(fuzzy_match("", "anything").is_some());
}

#[test]
fn fuzzy_match_too_long_pattern_fails() {
    assert!(fuzzy_match("hello world foo", "hello").is_none());
}

#[test]
fn fuzzy_match_indices_mark_matched_positions() {
    let (_, indices) = fuzzy_match("hlo", "hello").unwrap();
    assert_eq!(indices, [0, 2, 4]);
}

// ---------------------------------------------------------------------------
// SelectListState construction
// ---------------------------------------------------------------------------

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

#[test]
fn state_new_has_all_items_visible() {
    let state = SelectListState::new(sample_items());
    assert_eq!(state.visible_count(), 5);
    assert_eq!(state.selected_index(), 0);
}

#[test]
fn state_new_empty_items() {
    let state = SelectListState::new(vec![]);
    assert_eq!(state.visible_count(), 0);
    assert_eq!(state.selected_index(), 0);
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

#[test]
fn filter_narrows_results() {
    let mut state = SelectListState::new(sample_items());
    state.set_filter("claude");
    assert_eq!(state.visible_count(), 2);
    let displays: Vec<&str> = state.visible().iter().map(|i| i.display.as_str()).collect();
    assert!(displays.contains(&"Claude Sonnet 4.5"));
    assert!(displays.contains(&"Claude Opus 4"));
}

#[test]
fn filter_no_match_gives_empty() {
    let mut state = SelectListState::new(sample_items());
    state.set_filter("zzzzz");
    assert_eq!(state.visible_count(), 0);
}

#[test]
fn filter_clear_shows_all() {
    let mut state = SelectListState::new(sample_items());
    state.set_filter("claude");
    assert_eq!(state.visible_count(), 2);
    state.set_filter("");
    assert_eq!(state.visible_count(), 5);
}

#[test]
fn filter_preserves_selected_within_visible() {
    let mut state = SelectListState::new(sample_items());
    state.move_down(); // index 1: Claude Opus 4
    state.set_filter("claude");
    // Should still be selected if visible, otherwise clamp
    assert!(state.selected_index() < state.visible_count());
}

#[test]
fn filter_resets_selection_when_current_hidden() {
    let mut state = SelectListState::new(sample_items());
    state.move_down(); // index 1
    state.move_down(); // index 2: GPT-4o
    state.set_filter("claude");
    // GPT-4o is hidden; selection should clamp to 0
    assert_eq!(state.selected_index(), 0);
}

#[test]
fn filter_matches_metadata() {
    let mut state = SelectListState::new(sample_items());
    state.set_filter("openai");
    assert_eq!(state.visible_count(), 2);
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

#[test]
fn move_down_advances_selection() {
    let mut state = SelectListState::new(sample_items());
    assert_eq!(state.selected_index(), 0);
    state.move_down();
    assert_eq!(state.selected_index(), 1);
}

#[test]
fn move_down_clamps_at_end() {
    let mut state = SelectListState::new(sample_items());
    for _ in 0..10 {
        state.move_down();
    }
    assert_eq!(state.selected_index(), 4);
}

#[test]
fn move_up_from_start_stays_at_zero() {
    let mut state = SelectListState::new(sample_items());
    state.move_up();
    assert_eq!(state.selected_index(), 0);
}

#[test]
fn move_up_decrements() {
    let mut state = SelectListState::new(sample_items());
    state.move_down();
    state.move_down();
    assert_eq!(state.selected_index(), 2);
    state.move_up();
    assert_eq!(state.selected_index(), 1);
}

#[test]
fn page_down_advances_by_page_size() {
    let mut state = SelectListState::new(sample_items());
    state.page_down(3);
    assert_eq!(state.selected_index(), 3);
}

#[test]
fn page_down_clamps_at_end() {
    let mut state = SelectListState::new(sample_items());
    state.page_down(100);
    assert_eq!(state.selected_index(), 4);
}

#[test]
fn page_up_goes_to_start() {
    let mut state = SelectListState::new(sample_items());
    state.move_down();
    state.move_down();
    state.page_up(2);
    assert_eq!(state.selected_index(), 0);
}

#[test]
fn navigation_on_empty_list_is_noop() {
    let mut state = SelectListState::new(vec![]);
    state.move_down();
    state.move_up();
    state.page_down(5);
    state.page_up(5);
    assert_eq!(state.selected_index(), 0);
}

// ---------------------------------------------------------------------------
// Selection outcome
// ---------------------------------------------------------------------------

#[test]
fn confirm_returns_selected_item() {
    let state = SelectListState::new(sample_items());
    let item = state.confirm();
    assert!(item.is_some());
    assert_eq!(item.unwrap().id, "anthropic:claude-sonnet-4-5-20250514");
}

#[test]
fn confirm_on_empty_returns_none() {
    let state = SelectListState::new(vec![]);
    assert!(state.confirm().is_none());
}

#[test]
fn confirm_after_filter_returns_filtered_item() {
    let mut state = SelectListState::new(sample_items());
    state.set_filter("gemini");
    assert_eq!(state.visible_count(), 1);
    let item = state.confirm().unwrap();
    assert_eq!(item.id, "gemini:gemini-2.5-flash");
}

// ---------------------------------------------------------------------------
// Scroll offset
// ---------------------------------------------------------------------------

#[test]
fn scroll_offset_starts_at_zero() {
    let state = SelectListState::new(sample_items());
    assert_eq!(state.scroll_offset(), 0);
}

#[test]
fn scroll_offset_adjusts_on_page_down() {
    let mut state = SelectListState::new(sample_items());
    state.page_down(3);
    assert!(state.scroll_offset() <= state.selected_index());
}

// ---------------------------------------------------------------------------
// Large list stability
// ---------------------------------------------------------------------------

#[test]
fn large_list_filter_and_navigation() {
    let items: Vec<SelectItem> = (0..1000)
        .map(|i| SelectItem {
            id: format!("item-{i}"),
            display: format!("Item number {i}"),
            metadata: if i % 2 == 0 { "even" } else { "odd" }.into(),
        })
        .collect();

    let mut state = SelectListState::new(items);
    assert_eq!(state.visible_count(), 1000);

    state.set_filter("even");
    assert_eq!(state.visible_count(), 500); // 0, 2, 4, ..., 998

    state.move_down();
    assert_eq!(state.selected_index(), 1);

    state.page_down(100);
    assert!(state.selected_index() < state.visible_count());

    let item = state.confirm().unwrap();
    assert!(item.id.starts_with("item-"));
}

#[test]
fn large_list_fuzzy_filter_performance() {
    let items: Vec<SelectItem> = (0..1000)
        .map(|i| SelectItem {
            id: format!("model-{i}"),
            display: format!("Model Variant {i} v2"),
            metadata: "test".into(),
        })
        .collect();

    let mut state = SelectListState::new(items);
    state.set_filter("variant 500");
    // Should match items containing "variant" and "500" in order
    assert!(state.visible_count() > 0);
    assert!(state.visible_count() <= 10); // narrow match
}
