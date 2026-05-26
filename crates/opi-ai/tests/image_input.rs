//! Behavioral tests for task 3.4: image input.
//!
//! DoD: "InputContent::Image { source, media_type } variant added to public
//! protocol with documented CLI/TUI attachment contract, accepted media types,
//! size-limit behavior, and binary-safe source handling; provider capability
//! gating is explicit; Anthropic, OpenAI Chat, OpenAI Responses, and Gemini
//! providers serialize image content per each wire format; OpenRouter/Mistral
//! either serialize image-capable profiles or return clear unsupported-capability
//! errors; user-supplied image inputs round-trip through Agent/UserMessage
//! session JSONL entries and JSON mode events with stable media type/size
//! metadata; CLI/TUI attachment tests use isolated temp files and never read
//! user runtime data; fixture tests cover each provider path"

use opi_ai::message::{ImageSource, InputContent, MediaType};

// --- Image variant construction and serde ---

#[test]
fn image_variant_from_url() {
    let content = InputContent::Image {
        source: ImageSource::Url {
            url: "https://example.com/photo.png".into(),
        },
        media_type: MediaType::Png,
    };
    assert!(matches!(content, InputContent::Image { .. }));
}

#[test]
fn image_variant_from_base64() {
    let content = InputContent::Image {
        source: ImageSource::Base64 {
            data: "iVBORw0KGgo=".into(),
        },
        media_type: MediaType::Png,
    };
    assert!(matches!(content, InputContent::Image { .. }));
}

#[test]
fn image_variant_from_bytes() {
    let content = InputContent::Image {
        source: ImageSource::Bytes {
            data: vec![0x89, 0x50, 0x4E, 0x47],
        },
        media_type: MediaType::Png,
    };
    assert!(matches!(content, InputContent::Image { .. }));
}

#[test]
fn image_serde_roundtrip_url() {
    let content = InputContent::Image {
        source: ImageSource::Url {
            url: "https://example.com/img.jpg".into(),
        },
        media_type: MediaType::Jpeg,
    };
    let json = serde_json::to_string(&content).unwrap();
    let deserialized: InputContent = serde_json::from_str(&json).unwrap();
    assert_eq!(content, deserialized);
}

#[test]
fn image_serde_roundtrip_base64() {
    let content = InputContent::Image {
        source: ImageSource::Base64 {
            data: "iVBORw0KGgo=".into(),
        },
        media_type: MediaType::Png,
    };
    let json = serde_json::to_string(&content).unwrap();
    let deserialized: InputContent = serde_json::from_str(&json).unwrap();
    assert_eq!(content, deserialized);
}

#[test]
fn image_serde_roundtrip_bytes() {
    let content = InputContent::Image {
        source: ImageSource::Bytes {
            data: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A],
        },
        media_type: MediaType::Png,
    };
    let json = serde_json::to_string(&content).unwrap();
    let deserialized: InputContent = serde_json::from_str(&json).unwrap();
    assert_eq!(content, deserialized);
}

#[test]
fn image_serde_json_shape() {
    let content = InputContent::Image {
        source: ImageSource::Url {
            url: "https://example.com/img.png".into(),
        },
        media_type: MediaType::Png,
    };
    let json = serde_json::to_string(&content).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(val["type"], "image");
    assert_eq!(val["source"]["type"], "url");
    assert_eq!(val["source"]["url"], "https://example.com/img.png");
    assert_eq!(val["media_type"], "image/png");
}

// --- Accepted media types ---

#[test]
fn media_types_cover_standard_formats() {
    assert_eq!(MediaType::Png.as_str(), "image/png");
    assert_eq!(MediaType::Jpeg.as_str(), "image/jpeg");
    assert_eq!(MediaType::Gif.as_str(), "image/gif");
    assert_eq!(MediaType::WebP.as_str(), "image/webp");
}

#[test]
fn media_type_serde_roundtrip() {
    let mt = MediaType::Jpeg;
    let json = serde_json::to_string(&mt).unwrap();
    assert_eq!(json, "\"image/jpeg\"");
    let back: MediaType = serde_json::from_str(&json).unwrap();
    assert_eq!(mt, back);
}

// --- Binary-safe source handling ---

#[test]
fn bytes_source_preserves_binary_data() {
    let binary: Vec<u8> = (0u8..=255).collect();
    let content = InputContent::Image {
        source: ImageSource::Bytes {
            data: binary.clone(),
        },
        media_type: MediaType::Png,
    };
    let json = serde_json::to_string(&content).unwrap();
    let back: InputContent = serde_json::from_str(&json).unwrap();
    if let InputContent::Image {
        source: ImageSource::Bytes { data },
        ..
    } = back
    {
        assert_eq!(data, binary);
    } else {
        panic!("expected Image with Bytes source");
    }
}

// --- Existing text variant still works ---

#[test]
fn text_variant_unaffected() {
    let content = InputContent::Text {
        text: "hello".into(),
    };
    let json = serde_json::to_string(&content).unwrap();
    let back: InputContent = serde_json::from_str(&json).unwrap();
    assert_eq!(content, back);
}
