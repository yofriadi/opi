//! Configurable keybindings for TUI input handling.
//!
//! Provides [`Keybindings`] with semantic actions (submit, abort, new_line)
//! and [`KeyCombo`] for representing key + modifier combinations. The TUI
//! event loop matches crossterm key events against these bindings.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// A key action that can be bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Enter,
    Escape,
    Tab,
    Backspace,
    Char(char),
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

/// A key combination (key + optional modifiers).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub key: Key,
    pub modifiers: Modifiers,
}

/// Configurable keybindings for TUI input actions.
#[derive(Debug, Clone, PartialEq)]
pub struct Keybindings {
    pub submit: KeyCombo,
    pub abort: KeyCombo,
    pub new_line: KeyCombo,
}

// ---------------------------------------------------------------------------
// Parsing errors
// ---------------------------------------------------------------------------

/// Error from parsing an invalid key combo string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyComboParseError {
    #[error("empty key combo string")]
    Empty,
    #[error("unknown key: {0:?}")]
    UnknownKey(String),
}

// ---------------------------------------------------------------------------
// KeyCombo impl
// ---------------------------------------------------------------------------

impl KeyCombo {
    pub fn new(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::default(),
        }
    }

    pub fn alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }

    pub fn ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    pub fn shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }
}

impl FromStr for KeyCombo {
    type Err = KeyComboParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(KeyComboParseError::Empty);
        }

        let lower = s.to_lowercase();
        let parts: Vec<&str> = lower.split('+').collect();
        let key_part = *parts.last().ok_or(KeyComboParseError::Empty)?;

        if key_part.is_empty() {
            return Err(KeyComboParseError::Empty);
        }

        let key = if key_part.len() == 1 {
            Key::Char(key_part.chars().next().unwrap())
        } else {
            match key_part {
                "enter" | "return" => Key::Enter,
                "escape" | "esc" => Key::Escape,
                "tab" => Key::Tab,
                "backspace" | "bs" => Key::Backspace,
                other => return Err(KeyComboParseError::UnknownKey(other.to_string())),
            }
        };

        let mut mods = Modifiers::default();
        for &part in &parts[..parts.len() - 1] {
            match part {
                "alt" => mods.alt = true,
                "ctrl" | "control" => mods.ctrl = true,
                "shift" => mods.shift = true,
                other => return Err(KeyComboParseError::UnknownKey(other.to_string())),
            }
        }

        Ok(Self {
            key,
            modifiers: mods,
        })
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifiers.ctrl {
            parts.push("ctrl");
        }
        if self.modifiers.alt {
            parts.push("alt");
        }
        if self.modifiers.shift {
            parts.push("shift");
        }
        let key_name = match self.key {
            Key::Enter => "enter",
            Key::Escape => "escape",
            Key::Tab => "tab",
            Key::Backspace => "backspace",
            Key::Char(c) => {
                let s = c.to_string();
                parts.push(&s);
                return write!(f, "{}", parts.join("+"));
            }
        };
        parts.push(key_name);
        write!(f, "{}", parts.join("+"))
    }
}

// ---------------------------------------------------------------------------
// Keybindings impl
// ---------------------------------------------------------------------------

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            submit: KeyCombo::new(Key::Enter),
            abort: KeyCombo::new(Key::Escape),
            new_line: KeyCombo::new(Key::Enter).alt(),
        }
    }
}

impl Keybindings {
    pub fn submit(mut self, combo: KeyCombo) -> Self {
        self.submit = combo;
        self
    }

    pub fn abort(mut self, combo: KeyCombo) -> Self {
        self.abort = combo;
        self
    }

    pub fn new_line(mut self, combo: KeyCombo) -> Self {
        self.new_line = combo;
        self
    }

    /// Build keybindings from a config string map, using defaults for
    /// missing entries.
    pub fn from_config_map(map: &HashMap<String, String>) -> Result<Self, KeyComboParseError> {
        let mut kb = Self::default();
        if let Some(v) = map.get("submit") {
            kb.submit = KeyCombo::from_str(v)?;
        }
        if let Some(v) = map.get("abort") {
            kb.abort = KeyCombo::from_str(v)?;
        }
        if let Some(v) = map.get("new_line") {
            kb.new_line = KeyCombo::from_str(v)?;
        }
        Ok(kb)
    }
}
