//! Terminal image rendering tests for task 3.6.
//!
//! Validates Kitty/iTerm2/Sixel escape sequence generation, capability
//! detection, and text-placeholder fallback in opi-tui.

use opi_tui::terminal_image::{
    CapabilitySource, ImageData, MediaType, TerminalGraphicsProtocol, detect_graphics_protocol,
    iterm_escape, kitty_escape, sixel_escape, text_fallback,
};

// --- Kitty escape sequence generation ---

#[test]
fn kitty_escape_base64_payload() {
    let data = ImageData {
        bytes: vec![0x89, 0x50, 0x4E, 0x47],
        media_type: MediaType::Png,
        width: Some(100),
        height: Some(50),
    };
    let escape = kitty_escape(&data);
    assert!(
        escape.starts_with("\x1b_G"),
        "Kitty escape must start with ESC_G"
    );
    assert!(
        escape.contains("f=24"),
        "Kitty escape must specify format f=24 for PNG"
    );
    assert!(
        escape.contains("s=100"),
        "Kitty escape must contain width s=100"
    );
    assert!(
        escape.contains("v=50"),
        "Kitty escape must contain height v=50"
    );
    assert!(
        escape.ends_with("\x1b\\"),
        "Kitty escape must end with ESC\\"
    );
}

#[test]
fn kitty_escape_jpeg_format() {
    let data = ImageData {
        bytes: vec![0xFF, 0xD8, 0xFF, 0xE0],
        media_type: MediaType::Jpeg,
        width: Some(200),
        height: Some(150),
    };
    let escape = kitty_escape(&data);
    assert!(
        escape.contains("f=24"),
        "Kitty uses f=24 for JPEG too (RGB24)"
    );
    assert!(escape.contains("s=200"));
    assert!(escape.contains("v=150"));
}

#[test]
fn kitty_escape_no_dimensions() {
    let data = ImageData {
        bytes: vec![0x00, 0x01, 0x02],
        media_type: MediaType::Png,
        width: None,
        height: None,
    };
    let escape = kitty_escape(&data);
    assert!(
        !escape.contains("s="),
        "No width field when dimensions unknown"
    );
    assert!(
        !escape.contains("v="),
        "No height field when dimensions unknown"
    );
}

// --- iTerm2 escape sequence generation ---

#[test]
fn iterm_escape_base64_payload() {
    let data = ImageData {
        bytes: vec![0x89, 0x50, 0x4E, 0x47],
        media_type: MediaType::Png,
        width: Some(100),
        height: Some(50),
    };
    let escape = iterm_escape(&data);
    assert!(
        escape.starts_with("\x1b]1337;File=inline=1"),
        "iTerm2 escape must start with OSC 1337"
    );
    assert!(escape.ends_with("\x07"), "iTerm2 escape must end with BEL");
}

#[test]
fn iterm_escape_includes_size() {
    let data = ImageData {
        bytes: vec![0x01, 0x02],
        media_type: MediaType::Png,
        width: Some(640),
        height: Some(480),
    };
    let escape = iterm_escape(&data);
    assert!(
        escape.contains("width=640") || escape.contains("size"),
        "iTerm2 escape should reference image dimensions"
    );
}

// --- Sixel escape sequence generation ---

#[test]
fn sixel_escape_structure() {
    let data = ImageData {
        bytes: vec![0x00, 0x01, 0x02, 0x03],
        media_type: MediaType::Png,
        width: Some(10),
        height: Some(10),
    };
    let escape = sixel_escape(&data);
    assert!(
        escape.starts_with("\x1bPq") || escape.starts_with("\x1b["),
        "Sixel escape must start with DCS or CSI sequence"
    );
    assert!(
        escape.contains("\x1b\\"),
        "Sixel escape must contain ST terminator"
    );
}

// --- Text fallback ---

#[test]
fn text_fallback_png() {
    let data = ImageData {
        bytes: vec![0x89, 0x50, 0x4E, 0x47],
        media_type: MediaType::Png,
        width: Some(800),
        height: Some(600),
    };
    let fallback = text_fallback(&data);
    assert!(
        fallback.contains("[Image:"),
        "Fallback must start with [Image: marker"
    );
    assert!(
        fallback.contains("800x600"),
        "Fallback must include dimensions"
    );
    assert!(
        fallback.contains("PNG"),
        "Fallback must include media type name"
    );
    assert!(fallback.ends_with("]"), "Fallback must end with ]");
}

#[test]
fn text_fallback_no_dimensions() {
    let data = ImageData {
        bytes: vec![0x01],
        media_type: MediaType::Jpeg,
        width: None,
        height: None,
    };
    let fallback = text_fallback(&data);
    assert!(
        fallback.contains("JPEG"),
        "Fallback must include media type even without dimensions"
    );
    assert!(
        !fallback.contains("x"),
        "Fallback must not include x dimension separator when unknown"
    );
}

#[test]
fn text_fallback_all_media_types() {
    for (media_type, name) in [
        (MediaType::Png, "PNG"),
        (MediaType::Jpeg, "JPEG"),
        (MediaType::Gif, "GIF"),
        (MediaType::WebP, "WebP"),
    ] {
        let data = ImageData {
            bytes: vec![0x00],
            media_type,
            width: Some(100),
            height: Some(100),
        };
        let fallback = text_fallback(&data);
        assert!(
            fallback.contains(name),
            "Fallback for {name:?} must contain media type name"
        );
    }
}

// --- Capability detection ---

#[test]
fn detect_kitty_from_term_env() {
    let result =
        detect_graphics_protocol(Some("xterm-kitty"), None, None, &CapabilitySource::EnvVars);
    assert_eq!(result, TerminalGraphicsProtocol::Kitty);
}

#[test]
fn detect_iterm_from_term_program() {
    let result = detect_graphics_protocol(
        Some("xterm-256color"),
        Some("iTerm.app"),
        None,
        &CapabilitySource::EnvVars,
    );
    assert_eq!(result, TerminalGraphicsProtocol::Iterm2);
}

#[test]
fn detect_sixel_from_term_features() {
    let result = detect_graphics_protocol(
        Some("xterm-ghostty"),
        None,
        Some("sixel"),
        &CapabilitySource::EnvVars,
    );
    assert_eq!(result, TerminalGraphicsProtocol::Sixel);
}

#[test]
fn detect_fallback_when_no_protocol() {
    let result = detect_graphics_protocol(
        Some("xterm-256color"),
        None,
        None,
        &CapabilitySource::EnvVars,
    );
    assert_eq!(result, TerminalGraphicsProtocol::Fallback);
}

#[test]
fn detect_no_graphics_in_dumb_terminal() {
    let result = detect_graphics_protocol(Some("dumb"), None, None, &CapabilitySource::EnvVars);
    assert_eq!(result, TerminalGraphicsProtocol::Fallback);
}

#[test]
fn kitty_takes_precedence_over_iterm() {
    let result = detect_graphics_protocol(
        Some("xterm-kitty"),
        Some("iTerm.app"),
        None,
        &CapabilitySource::EnvVars,
    );
    assert_eq!(
        result,
        TerminalGraphicsProtocol::Kitty,
        "Kitty TERM should win over iTerm TERM_PROGRAM"
    );
}

// --- ImageData construction ---

#[test]
fn image_data_construction() {
    let data = ImageData {
        bytes: vec![0x89, 0x50, 0x4E, 0x47],
        media_type: MediaType::Png,
        width: Some(640),
        height: Some(480),
    };
    assert_eq!(data.media_type, MediaType::Png);
    assert_eq!(data.width, Some(640));
    assert_eq!(data.height, Some(480));
    assert_eq!(data.bytes.len(), 4);
}

// --- MediaType display/str ---

#[test]
fn media_type_str_roundtrip() {
    assert_eq!(MediaType::Png.as_str(), "image/png");
    assert_eq!(MediaType::Jpeg.as_str(), "image/jpeg");
    assert_eq!(MediaType::Gif.as_str(), "image/gif");
    assert_eq!(MediaType::WebP.as_str(), "image/webp");
}
