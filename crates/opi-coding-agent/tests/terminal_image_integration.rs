//! Terminal image integration tests for task 3.6.
//!
//! Validates that image content from agent tool results and user inputs
//! is correctly converted to TUI image messages and rendered in the
//! conversation display.

use opi_ai::message::{ImageSource, MediaType, OutputContent};
use opi_tui::{
    ImageData, ImagePayload, MediaType as TuiMediaType, Message as TuiMessage, Role as TuiRole,
    TerminalGraphicsProtocol,
};

/// Convert an opi-ai MediaType to an opi-tui MediaType.
fn convert_media_type(mt: &MediaType) -> TuiMediaType {
    match mt {
        MediaType::Png => TuiMediaType::Png,
        MediaType::Jpeg => TuiMediaType::Jpeg,
        MediaType::Gif => TuiMediaType::Gif,
        MediaType::WebP => TuiMediaType::WebP,
        _ => TuiMediaType::Png,
    }
}

/// Extract raw bytes from an ImageSource.
fn source_to_bytes(source: &ImageSource) -> Option<Vec<u8>> {
    match source {
        ImageSource::Base64 { data } => {
            use base64::Engine;
            Some(
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap_or_default(),
            )
        }
        ImageSource::Bytes { data } => Some(data.clone()),
        ImageSource::Url { .. } => None,
        _ => None,
    }
}

/// Convert OutputContent::Image items from a tool result to TUI image messages.
fn image_contents_to_tui_messages(
    contents: &[OutputContent],
    protocol: TerminalGraphicsProtocol,
    role: TuiRole,
) -> Vec<TuiMessage> {
    contents
        .iter()
        .filter_map(|c| match c {
            OutputContent::Image { source, media_type } => {
                let bytes = source_to_bytes(source)?;
                if bytes.is_empty() {
                    return None;
                }
                let data = ImageData {
                    bytes,
                    media_type: convert_media_type(media_type),
                    width: None,
                    height: None,
                };
                Some(TuiMessage::image(
                    role.clone(),
                    ImagePayload { data, protocol },
                ))
            }
            _ => None,
        })
        .collect()
}

// --- Conversion tests ---

#[test]
fn convert_base64_image_to_tui_message() {
    let contents = vec![
        OutputContent::Text {
            text: "Screenshot captured".into(),
        },
        OutputContent::Image {
            source: ImageSource::Base64 {
                data: "iVBORw0KGgo=".into(),
            },
            media_type: MediaType::Png,
        },
    ];
    let messages = image_contents_to_tui_messages(
        &contents,
        TerminalGraphicsProtocol::Fallback,
        TuiRole::Tool,
    );
    assert_eq!(messages.len(), 1);
    let msg = &messages[0];
    assert_eq!(msg.role, TuiRole::Tool);
    assert!(msg.image.is_some());
    assert!(msg.content.contains("[Image:"));
    assert!(msg.content.contains("PNG"));
}

#[test]
fn convert_bytes_image_to_tui_message() {
    let contents = vec![OutputContent::Image {
        source: ImageSource::Bytes {
            data: vec![0x89, 0x50, 0x4E, 0x47],
        },
        media_type: MediaType::Png,
    }];
    let messages = image_contents_to_tui_messages(
        &contents,
        TerminalGraphicsProtocol::Fallback,
        TuiRole::Tool,
    );
    assert_eq!(messages.len(), 1);
    let payload = messages[0].image.as_ref().unwrap();
    assert_eq!(payload.data.bytes, vec![0x89, 0x50, 0x4E, 0x47]);
    assert_eq!(payload.data.media_type, TuiMediaType::Png);
}

#[test]
fn convert_url_image_skipped() {
    let contents = vec![OutputContent::Image {
        source: ImageSource::Url {
            url: "https://example.com/img.png".into(),
        },
        media_type: MediaType::Png,
    }];
    let messages = image_contents_to_tui_messages(
        &contents,
        TerminalGraphicsProtocol::Fallback,
        TuiRole::Tool,
    );
    assert!(
        messages.is_empty(),
        "URL images should be skipped in TUI (no async fetch available)"
    );
}

#[test]
fn convert_text_only_no_image_messages() {
    let contents = vec![
        OutputContent::Text {
            text: "Just text".into(),
        },
        OutputContent::Text {
            text: "More text".into(),
        },
    ];
    let messages = image_contents_to_tui_messages(
        &contents,
        TerminalGraphicsProtocol::Fallback,
        TuiRole::Tool,
    );
    assert!(messages.is_empty());
}

#[test]
fn convert_all_media_types() {
    for (ai_mt, tui_name) in [
        (MediaType::Png, TuiMediaType::Png),
        (MediaType::Jpeg, TuiMediaType::Jpeg),
        (MediaType::Gif, TuiMediaType::Gif),
        (MediaType::WebP, TuiMediaType::WebP),
    ] {
        let contents = vec![OutputContent::Image {
            source: ImageSource::Bytes {
                data: vec![0x00, 0x01],
            },
            media_type: ai_mt,
        }];
        let messages = image_contents_to_tui_messages(
            &contents,
            TerminalGraphicsProtocol::Fallback,
            TuiRole::Tool,
        );
        assert_eq!(
            messages.len(),
            1,
            "Expected one image message for {tui_name:?}"
        );
        assert_eq!(
            messages[0].image.as_ref().unwrap().data.media_type,
            tui_name
        );
    }
}

#[test]
fn protocol_carried_through_to_tui() {
    let contents = vec![OutputContent::Image {
        source: ImageSource::Bytes { data: vec![0x00] },
        media_type: MediaType::Png,
    }];
    for protocol in [
        TerminalGraphicsProtocol::Kitty,
        TerminalGraphicsProtocol::Iterm2,
        TerminalGraphicsProtocol::Sixel,
        TerminalGraphicsProtocol::Fallback,
    ] {
        let messages = image_contents_to_tui_messages(&contents, protocol, TuiRole::Tool);
        assert_eq!(messages[0].image.as_ref().unwrap().protocol, protocol);
    }
}

#[test]
fn binary_preservation_through_conversion() {
    let original: Vec<u8> = (0u8..=255).collect();
    let contents = vec![OutputContent::Image {
        source: ImageSource::Bytes {
            data: original.clone(),
        },
        media_type: MediaType::Png,
    }];
    let messages = image_contents_to_tui_messages(
        &contents,
        TerminalGraphicsProtocol::Fallback,
        TuiRole::Tool,
    );
    assert_eq!(messages[0].image.as_ref().unwrap().data.bytes, original);
}

#[test]
fn base64_decode_preserves_bytes() {
    use base64::Engine;
    let original = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let encoded = base64::engine::general_purpose::STANDARD.encode(&original);
    let contents = vec![OutputContent::Image {
        source: ImageSource::Base64 { data: encoded },
        media_type: MediaType::Png,
    }];
    let messages = image_contents_to_tui_messages(
        &contents,
        TerminalGraphicsProtocol::Fallback,
        TuiRole::Tool,
    );
    assert_eq!(messages[0].image.as_ref().unwrap().data.bytes, original);
}

#[test]
fn mixed_text_and_image_contents() {
    let contents = vec![
        OutputContent::Text {
            text: "Tool ran successfully".into(),
        },
        OutputContent::Image {
            source: ImageSource::Bytes {
                data: vec![0xFF, 0xD8, 0xFF],
            },
            media_type: MediaType::Jpeg,
        },
        OutputContent::Text {
            text: "See image above".into(),
        },
    ];
    let messages = image_contents_to_tui_messages(
        &contents,
        TerminalGraphicsProtocol::Fallback,
        TuiRole::Tool,
    );
    assert_eq!(
        messages.len(),
        1,
        "Only image content produces TUI messages"
    );
    assert!(messages[0].content.contains("JPEG"));
}
