//! Keybindings tests (task 2.13).
//!
//! DoD: "KeybindingsConfig struct added to OpiConfig with [keybindings] TOML parsing,
//!       configurable submit/abort/new_line passed to opi-tui input handler,
//!       tested with non-default bindings"

use std::str::FromStr;

use opi_tui::{Key, KeyCombo, Keybindings, Modifiers};

// ---------------------------------------------------------------------------
// KeyCombo parsing
// ---------------------------------------------------------------------------

#[test]
fn keycombo_from_str_enter() {
    let kc = KeyCombo::from_str("enter").unwrap();
    assert_eq!(kc.key, Key::Enter);
    assert_eq!(kc.modifiers, Modifiers::default());
}

#[test]
fn keycombo_from_str_escape() {
    let kc = KeyCombo::from_str("escape").unwrap();
    assert_eq!(kc.key, Key::Escape);
    assert_eq!(kc.modifiers, Modifiers::default());
}

#[test]
fn keycombo_from_str_alt_enter() {
    let kc = KeyCombo::from_str("alt+enter").unwrap();
    assert_eq!(kc.key, Key::Enter);
    assert!(kc.modifiers.alt);
    assert!(!kc.modifiers.ctrl);
    assert!(!kc.modifiers.shift);
}

#[test]
fn keycombo_from_str_ctrl_c() {
    let kc = KeyCombo::from_str("ctrl+c").unwrap();
    assert_eq!(kc.key, Key::Char('c'));
    assert!(!kc.modifiers.alt);
    assert!(kc.modifiers.ctrl);
}

#[test]
fn keycombo_from_str_shift_tab() {
    let kc = KeyCombo::from_str("shift+tab").unwrap();
    assert_eq!(kc.key, Key::Tab);
    assert!(kc.modifiers.shift);
}

#[test]
fn keycombo_from_str_single_char() {
    let kc = KeyCombo::from_str("q").unwrap();
    assert_eq!(kc.key, Key::Char('q'));
    assert_eq!(kc.modifiers, Modifiers::default());
}

#[test]
fn keycombo_from_str_invalid_returns_error() {
    assert!(KeyCombo::from_str("").is_err());
    assert!(KeyCombo::from_str("ctrl+").is_err());
}

#[test]
fn keycombo_display_roundtrip() {
    let combos = &["enter", "escape", "alt+enter", "ctrl+c", "shift+tab"];
    for &s in combos {
        let kc = KeyCombo::from_str(s).unwrap();
        assert_eq!(kc.to_string().to_lowercase(), s);
    }
}

// ---------------------------------------------------------------------------
// Keybindings defaults
// ---------------------------------------------------------------------------

#[test]
fn keybindings_default_submit_is_enter() {
    let kb = Keybindings::default();
    assert_eq!(kb.submit, KeyCombo::from_str("enter").unwrap());
}

#[test]
fn keybindings_default_abort_is_escape() {
    let kb = Keybindings::default();
    assert_eq!(kb.abort, KeyCombo::from_str("escape").unwrap());
}

#[test]
fn keybindings_default_new_line_is_alt_enter() {
    let kb = Keybindings::default();
    assert_eq!(kb.new_line, KeyCombo::from_str("alt+enter").unwrap());
}

// ---------------------------------------------------------------------------
// Keybindings builder with non-default bindings
// ---------------------------------------------------------------------------

#[test]
fn keybindings_custom_submit() {
    let kb = Keybindings::default().submit(KeyCombo::from_str("ctrl+s").unwrap());
    assert_eq!(kb.submit.key, Key::Char('s'));
    assert!(kb.submit.modifiers.ctrl);
    // Other fields unchanged
    assert_eq!(kb.abort, KeyCombo::from_str("escape").unwrap());
}

#[test]
fn keybindings_custom_abort() {
    let kb = Keybindings::default().abort(KeyCombo::from_str("ctrl+c").unwrap());
    assert_eq!(kb.abort.key, Key::Char('c'));
    assert!(kb.abort.modifiers.ctrl);
}

#[test]
fn keybindings_custom_new_line() {
    let kb = Keybindings::default().new_line(KeyCombo::from_str("ctrl+enter").unwrap());
    assert_eq!(kb.new_line.key, Key::Enter);
    assert!(kb.new_line.modifiers.ctrl);
}

// ---------------------------------------------------------------------------
// Keybindings from string map (config integration)
// ---------------------------------------------------------------------------

#[test]
fn keybindings_from_map_partial_override() {
    let map = std::collections::HashMap::from([("submit".to_string(), "ctrl+j".to_string())]);
    let kb = Keybindings::from_config_map(&map).unwrap();
    assert_eq!(kb.submit, KeyCombo::from_str("ctrl+j").unwrap());
    // Non-overridden fields keep defaults
    assert_eq!(kb.abort, Keybindings::default().abort);
    assert_eq!(kb.new_line, Keybindings::default().new_line);
}

#[test]
fn keybindings_from_map_empty_uses_defaults() {
    let map = std::collections::HashMap::<String, String>::new();
    let kb = Keybindings::from_config_map(&map).unwrap();
    assert_eq!(kb, Keybindings::default());
}

#[test]
fn keybindings_from_map_invalid_value_returns_error() {
    let map =
        std::collections::HashMap::from([("submit".to_string(), "!!!invalid!!!".to_string())]);
    assert!(Keybindings::from_config_map(&map).is_err());
}
