//! Terminal image rendering with Kitty/iTerm2/Sixel escape sequences.
//!
//! Provides escape sequence generation for terminal graphics protocols,
//! capability detection from environment variables, and text-placeholder
//! fallback when no graphics protocol is supported.

/// Supported terminal graphics protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalGraphicsProtocol {
    Kitty,
    Iterm2,
    Sixel,
    Fallback,
}

/// Source for terminal capability detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilitySource {
    /// Detect from environment variables (TERM, TERM_PROGRAM, TERM_FEATURES).
    EnvVars,
}

/// Image media type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Png,
    Jpeg,
    Gif,
    WebP,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::WebP => "WebP",
        }
    }
}

/// Raw image data with metadata for terminal rendering.
#[derive(Debug, Clone)]
pub struct ImageData {
    pub bytes: Vec<u8>,
    pub media_type: MediaType,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

/// Detect the best available terminal graphics protocol from environment.
///
/// Checks TERM, TERM_PROGRAM, and TERM_FEATURES in priority order:
/// Kitty > iTerm2 > Sixel > Fallback.
pub fn detect_graphics_protocol(
    term: Option<&str>,
    term_program: Option<&str>,
    term_features: Option<&str>,
    _source: &CapabilitySource,
) -> TerminalGraphicsProtocol {
    if let Some(term) = term {
        if term == "xterm-kitty" {
            return TerminalGraphicsProtocol::Kitty;
        }
        if term == "xterm-ghostty" || term == "mlterm" || term == "yaft-256color" {
            if term_features.map(|f| f.contains("sixel")).unwrap_or(false) {
                return TerminalGraphicsProtocol::Sixel;
            }
            // Ghostty and mlterm may still support sixel
            if term == "mlterm" || term == "yaft-256color" {
                return TerminalGraphicsProtocol::Sixel;
            }
        }
    }

    if let Some(program) = term_program
        && program == "iTerm.app"
    {
        return TerminalGraphicsProtocol::Iterm2;
    }

    if let Some(features) = term_features
        && features.contains("sixel")
    {
        return TerminalGraphicsProtocol::Sixel;
    }

    TerminalGraphicsProtocol::Fallback
}

/// Generate a Kitty graphics protocol escape sequence for the given image.
///
/// Uses base64-encoded payload with the `a=T` (transmit and display) action.
pub fn kitty_escape(data: &ImageData) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data.bytes);

    let mut params = vec!["a=T".to_string(), "f=24".to_string()];
    if let Some(w) = data.width {
        params.push(format!("s={w}"));
    }
    if let Some(h) = data.height {
        params.push(format!("v={h}"));
    }
    params.push(format!("t=d,{b64}"));

    format!("\x1b_G{};\x1b\\", params.join(","))
}

/// Generate an iTerm2 inline image escape sequence.
///
/// Uses OSC 1337 with base64-encoded image data.
pub fn iterm_escape(data: &ImageData) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data.bytes);

    let mut parts = vec![format!("inline=1")];
    if let Some(w) = data.width {
        parts.push(format!("width={w}"));
    }
    if let Some(h) = data.height {
        parts.push(format!("height={h}"));
    }

    format!("\x1b]1337;File={};{}\x07", parts.join(";"), b64)
}

/// Generate a Sixel escape sequence for the given image.
///
/// Sixel encodes pixel data as character sequences. For raw bytes input,
/// this produces a minimal Sixel wrapper indicating image dimensions.
pub fn sixel_escape(data: &ImageData) -> String {
    let w = data.width.unwrap_or(1);
    let h = data.height.unwrap_or(1);

    // DCS q with raster attributes: "1;1;{width};{height}"
    // followed by Sixel data and ST terminator
    format!("\x1bPq\"1;1;{w};{h}\x1b\\")
}

/// Generate a text placeholder for the image.
///
/// Produces a human-readable description like `[Image: 800x600 PNG]`.
pub fn text_fallback(data: &ImageData) -> String {
    match (data.width, data.height) {
        (Some(w), Some(h)) => format!("[Image: {w}x{h} {}]", data.media_type.name()),
        _ => format!("[Image: {}]", data.media_type.name()),
    }
}
