//! Picker integration tests (task 3.11).
//!
//! Tests the bridge between provider registry / session listing and the
//! SelectList widget, verifying model picker and session picker data
//! collection and result handling.

use std::path::Path;

use opi_agent::session::{LeafEntry, MessageEntry, SessionEntry};
use opi_agent::session_branch::SessionTree;
use opi_ai::message::{InputContent, Message, UserMessage};
use opi_ai::provider::{EventStream, ModelInfo, Provider, ProviderError, Request};
use opi_ai::stream::AssistantStreamEvent;
use opi_coding_agent::picker;
use opi_tui::select_list::SelectListState;

/// Minimal provider with configurable models for picker tests.
struct TestProvider {
    id: String,
    models: Vec<ModelInfo>,
}

impl TestProvider {
    fn new(id: &str, models: Vec<ModelInfo>) -> Self {
        Self {
            id: id.into(),
            models,
        }
    }
}

impl Provider for TestProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn models(&self) -> &[ModelInfo] {
        &self.models
    }
    fn stream(&self, _request: Request) -> EventStream {
        // Picker tests never call stream(); return empty stream.
        let stream: Vec<Result<AssistantStreamEvent, ProviderError>> = vec![];
        Box::pin(futures_util::stream::iter(stream))
    }
}

// ---------------------------------------------------------------------------
// Model picker
// ---------------------------------------------------------------------------

fn sample_registry() -> opi_ai::registry::ProviderRegistry {
    let models = vec![
        ModelInfo {
            id: "claude-sonnet-4-5-20250514".into(),
            display_name: "Claude Sonnet 4.5".into(),
            context_window: 200000,
            max_output_tokens: 8192,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: true,
        },
        ModelInfo {
            id: "claude-opus-4-20250514".into(),
            display_name: "Claude Opus 4".into(),
            context_window: 200000,
            max_output_tokens: 8192,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: true,
        },
    ];
    let provider = TestProvider::new("anthropic", models);
    let mut registry = opi_ai::registry::ProviderRegistry::new();
    registry.register(Box::new(provider));
    registry
}

#[test]
fn model_picker_items_from_registry() {
    let registry = sample_registry();
    let items = picker::model_picker_items(&registry);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "anthropic:claude-sonnet-4-5-20250514");
    assert_eq!(items[0].display, "Claude Sonnet 4.5");
    assert_eq!(items[0].metadata, "anthropic");
}

#[test]
fn model_picker_filter_and_select() {
    let registry = sample_registry();
    let items = picker::model_picker_items(&registry);
    let mut state = SelectListState::new(items);
    state.set_filter("opus");
    assert_eq!(state.visible_count(), 1);
    let selected = state.confirm().unwrap();
    assert_eq!(selected.id, "anthropic:claude-opus-4-20250514");
}

#[test]
fn model_picker_multiple_providers() {
    let p1 = TestProvider::new(
        "anthropic",
        vec![ModelInfo {
            id: "claude-sonnet-4-5-20250514".into(),
            display_name: "Claude Sonnet 4.5".into(),
            context_window: 200000,
            max_output_tokens: 8192,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: true,
        }],
    );
    let p2 = TestProvider::new(
        "openai",
        vec![ModelInfo {
            id: "gpt-4o".into(),
            display_name: "GPT-4o".into(),
            context_window: 128000,
            max_output_tokens: 4096,
            supports_images: true,
            supports_streaming: true,
            supports_thinking: false,
        }],
    );
    let mut registry = opi_ai::registry::ProviderRegistry::new();
    registry.register(Box::new(p1));
    registry.register(Box::new(p2));

    let items = picker::model_picker_items(&registry);
    assert_eq!(items.len(), 2);

    let mut state = SelectListState::new(items);
    state.set_filter("gpt");
    assert_eq!(state.visible_count(), 1);
    assert_eq!(state.confirm().unwrap().id, "openai:gpt-4o");
}

#[test]
fn model_picker_items_include_registry_model_overrides() {
    let mut registry = sample_registry();
    registry
        .register_model(
            "anthropic",
            ModelInfo {
                id: "custom-sonnet".into(),
                display_name: "Custom Sonnet".into(),
                context_window: 200000,
                max_output_tokens: 8192,
                supports_images: true,
                supports_streaming: true,
                supports_thinking: true,
            },
        )
        .unwrap();

    let items = picker::model_picker_items(&registry);

    assert!(
        items
            .iter()
            .any(|item| item.id == "anthropic:custom-sonnet" && item.display == "Custom Sonnet")
    );
}

#[test]
fn branch_picker_items_from_session_tree() {
    let tree = SessionTree::from_entries(&[
        test_user_entry("e1", None, "Root summary"),
        test_user_entry("e2a", Some("e1"), "Branch A"),
        test_user_entry("e2b", Some("e1"), "Branch B"),
        SessionEntry::Leaf(LeafEntry {
            id: "leaf-1".into(),
            parent_id: None,
            timestamp: "2026-06-01T12:03:00Z".into(),
            entry_id: "e2a".into(),
        }),
    ]);

    let items = picker::branch_picker_items(&tree);

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].id, "e1");
    assert!(items[0].display.contains("Root summary"));
    assert_eq!(items[1].id, "e2a");
    assert!(items[1].display.contains("Branch A"));
    assert!(items[1].metadata.contains("active"));
}

fn test_user_entry(id: &str, parent_id: Option<&str>, text: &str) -> SessionEntry {
    SessionEntry::Message(MessageEntry {
        id: id.into(),
        parent_id: parent_id.map(str::to_owned),
        timestamp: "2026-06-01T12:00:00Z".into(),
        message: Message::User(UserMessage {
            content: vec![InputContent::Text { text: text.into() }],
            timestamp_ms: 0,
        }),
    })
}

// ---------------------------------------------------------------------------
// Session picker
// ---------------------------------------------------------------------------

#[test]
fn session_picker_items_from_directory() {
    let dir = tempfile::tempdir().unwrap();
    create_test_session(
        dir.path(),
        "sess-001",
        "2026-05-26T10:00:00Z",
        "/home/user/project",
    );
    create_test_session(
        dir.path(),
        "sess-002",
        "2026-05-26T11:00:00Z",
        "/home/user/other",
    );

    let items = picker::session_picker_items(dir.path()).unwrap();
    assert_eq!(items.len(), 2);
    // Sorted newest-first by timestamp
    assert_eq!(items[0].id, "sess-002");
    assert!(items[1].display.contains("project"));
    assert_eq!(items[1].id, "sess-001");
}

#[test]
fn session_picker_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let items = picker::session_picker_items(dir.path()).unwrap();
    assert!(items.is_empty());
}

#[test]
fn session_picker_filter_and_select() {
    let dir = tempfile::tempdir().unwrap();
    create_test_session(
        dir.path(),
        "sess-001",
        "2026-05-26T10:00:00Z",
        "/home/user/project",
    );
    create_test_session(
        dir.path(),
        "sess-002",
        "2026-05-26T11:00:00Z",
        "/home/user/other",
    );

    let items = picker::session_picker_items(dir.path()).unwrap();
    let mut state = SelectListState::new(items);
    state.set_filter("other");
    assert_eq!(state.visible_count(), 1);
    let selected = state.confirm().unwrap();
    assert_eq!(selected.id, "sess-002");
}

#[test]
fn session_picker_corrupt_file_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    create_test_session(
        dir.path(),
        "sess-001",
        "2026-05-26T10:00:00Z",
        "/home/user/project",
    );

    // Write a corrupt file.
    std::fs::write(dir.path().join("corrupt.jsonl"), "not valid json\n").unwrap();

    let items = picker::session_picker_items(dir.path()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "sess-001");
}

#[test]
fn session_picker_long_cwd_is_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let long_cwd =
        "/home/user/very/deeply/nested/directory/structure/that/exceeds/forty/characters";
    create_test_session(dir.path(), "sess-001", "2026-05-26T10:00:00Z", long_cwd);

    let items = picker::session_picker_items(dir.path()).unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].display.starts_with("..."));
    assert!(items[0].display.len() <= 40);
}

#[test]
fn session_picker_multibyte_cwd_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    // CJK characters: each is 3 bytes in UTF-8. 20 chars = 60 bytes.
    let cjk_cwd = "/home/\u{4f60}\u{597d}\u{4e16}\u{754c}\u{6587}\u{4ef6}\u{76ee}\u{5f55}\u{8def}\u{5f84}\u{6d4b}\u{8bd5}\u{6570}\u{636e}\u{9879}\u{76ee}\u{5de5}\u{7a0b}\u{4ee3}\u{7801}/end";
    create_test_session(dir.path(), "sess-cjk", "2026-05-26T10:00:00Z", cjk_cwd);

    let items = picker::session_picker_items(dir.path()).unwrap();
    assert_eq!(items.len(), 1);
    // Should not panic, and display should be valid UTF-8.
    assert!(items[0].display.starts_with("...") || items[0].display.len() <= 40);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_test_session(dir: &Path, id: &str, timestamp: &str, cwd: &str) {
    use std::io::Write;
    let header = serde_json::json!({
        "type": "session",
        "version": 1,
        "id": id,
        "timestamp": timestamp,
        "cwd": cwd,
    });
    let path = dir.join(format!("{id}.jsonl"));
    let mut f = std::fs::File::create(path).unwrap();
    writeln!(f, "{header}").unwrap();
}
