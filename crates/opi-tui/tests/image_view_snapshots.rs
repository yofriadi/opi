//! ImageView widget and MessageList image integration tests for task 3.6.
//!
//! Validates the ImageView ratatui widget, Message extension for images,
//! and MessageList rendering with image content using snapshot tests.

use opi_tui::{
    ImageData, ImagePayload, MediaType, Message, MessageList, Role,
    terminal_image::TerminalGraphicsProtocol,
};
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

// --- Message with image payload construction ---

#[test]
fn message_with_image_payload() {
    let msg = Message::image(
        Role::Assistant,
        ImagePayload {
            data: ImageData {
                bytes: vec![0x89, 0x50, 0x4E, 0x47],
                media_type: MediaType::Png,
                width: Some(800),
                height: Some(600),
            },
            protocol: TerminalGraphicsProtocol::Fallback,
        },
    );
    assert_eq!(msg.role, Role::Assistant);
    assert!(msg.image.is_some());
    assert!(msg.diff.is_none());
}

#[test]
fn message_with_text_and_image() {
    let msg = Message::new(Role::Assistant, "Here is the screenshot:").with_image(ImagePayload {
        data: ImageData {
            bytes: vec![0x00],
            media_type: MediaType::Png,
            width: Some(100),
            height: Some(100),
        },
        protocol: TerminalGraphicsProtocol::Fallback,
    });
    assert_eq!(msg.content, "Here is the screenshot:");
    assert!(msg.image.is_some());
}

// --- MessageList with image messages (fallback rendering) ---

#[test]
fn message_list_with_fallback_image_80x24() {
    let messages = vec![
        Message::new(Role::User, "Take a screenshot"),
        Message::image(
            Role::Tool,
            ImagePayload {
                data: ImageData {
                    bytes: vec![0x89, 0x50, 0x4E, 0x47],
                    media_type: MediaType::Png,
                    width: Some(800),
                    height: Some(600),
                },
                protocol: TerminalGraphicsProtocol::Fallback,
            },
        ),
    ];
    let widget = MessageList::new(messages);
    insta::assert_snapshot!(
        "message_list_with_fallback_image_80x24",
        render(widget, 80, 24)
    );
}

#[test]
fn message_list_with_fallback_image_120x40() {
    let messages = vec![
        Message::new(Role::User, "Show me the dashboard"),
        Message::image(
            Role::Tool,
            ImagePayload {
                data: ImageData {
                    bytes: vec![0xFF, 0xD8, 0xFF],
                    media_type: MediaType::Jpeg,
                    width: Some(1920),
                    height: Some(1080),
                },
                protocol: TerminalGraphicsProtocol::Fallback,
            },
        ),
        Message::new(Role::Assistant, "Here is the dashboard view."),
    ];
    let widget = MessageList::new(messages);
    insta::assert_snapshot!(
        "message_list_with_fallback_image_120x40",
        render(widget, 120, 40)
    );
}

#[test]
fn message_list_with_kitty_protocol_image() {
    let messages = vec![Message::image(
        Role::Tool,
        ImagePayload {
            data: ImageData {
                bytes: vec![0x89, 0x50, 0x4E, 0x47],
                media_type: MediaType::Png,
                width: Some(640),
                height: Some(480),
            },
            protocol: TerminalGraphicsProtocol::Kitty,
        },
    )];
    let widget = MessageList::new(messages);
    // Kitty protocol renders escape sequences; fallback text is also
    // rendered alongside for terminals that don't support it
    insta::assert_snapshot!(
        "message_list_with_kitty_image_80x24",
        render(widget, 80, 24)
    );
}

#[test]
fn message_list_mixed_text_and_image_80x24() {
    let messages = vec![
        Message::new(Role::User, "Capture the current page"),
        Message::new(Role::Tool, "Screenshot saved"),
        Message::image(
            Role::Tool,
            ImagePayload {
                data: ImageData {
                    bytes: vec![0x00],
                    media_type: MediaType::Png,
                    width: Some(1280),
                    height: Some(720),
                },
                protocol: TerminalGraphicsProtocol::Fallback,
            },
        ),
        Message::new(Role::Assistant, "I've captured the page as shown above."),
    ];
    let widget = MessageList::new(messages);
    insta::assert_snapshot!(
        "message_list_mixed_text_and_image_80x24",
        render(widget, 80, 24)
    );
}

#[test]
fn message_list_image_no_dimensions_fallback() {
    let messages = vec![Message::image(
        Role::Tool,
        ImagePayload {
            data: ImageData {
                bytes: vec![0x00],
                media_type: MediaType::Gif,
                width: None,
                height: None,
            },
            protocol: TerminalGraphicsProtocol::Fallback,
        },
    )];
    let widget = MessageList::new(messages);
    insta::assert_snapshot!("message_list_image_no_dims_80x10", render(widget, 80, 10));
}
