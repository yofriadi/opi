//! Session runtime integration tests (task 5).
//!
//! Tests the full lifecycle: harness creates a session, persists messages as
//! JSONL, and the session can be read back and reconstructed for resume.

use opi_agent::message::AgentMessage;
use opi_agent::session::{MessageEntry, SessionEntry, SessionHeader, SessionReader, SessionWriter};
use opi_ai::message::{InputContent, Message, UserMessage};
use opi_ai::test_support::{self, MockProvider};
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::harness::CodingHarness;
use opi_coding_agent::session_coordinator::SessionCoordinator;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_header(id: &str, cwd: &str) -> SessionHeader {
    SessionHeader::new(id.into(), "2026-05-22T12:00:00Z".into(), cwd.into(), None)
}

fn test_message_entry(id: &str, text: &str) -> SessionEntry {
    SessionEntry::Message(MessageEntry {
        id: id.into(),
        parent_id: None,
        timestamp: "2026-05-22T12:00:01Z".into(),
        message: Message::User(UserMessage {
            content: vec![InputContent::Text { text: text.into() }],
            timestamp_ms: 0,
        }),
    })
}

/// Set env var safely (edition 2024 requires unsafe for set_var).
fn set_sessions_dir(dir: &std::path::Path) {
    // SAFETY: test-only env var mutation; no other thread reads this var
    // concurrently during the test.
    unsafe { std::env::set_var("OPI_SESSIONS_DIR", dir); }
}

/// Remove env var safely (edition 2024 requires unsafe for remove_var).
fn clear_sessions_dir() {
    // SAFETY: test-only env var mutation; no other thread reads this var
    // concurrently during the test.
    unsafe { std::env::remove_var("OPI_SESSIONS_DIR"); }
}

// ---------------------------------------------------------------------------
// SessionCoordinator tests
// ---------------------------------------------------------------------------

#[test]
fn session_coordinator_creates_jsonl_file() {
    let dir = tempfile::tempdir().unwrap();
    let coord = SessionCoordinator::new(
        dir.path(),
        "/test/cwd",
        opi_agent::compaction::CompactionConfig::default(),
    )
    .unwrap();

    // Verify file exists
    let jsonl_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "jsonl")
        })
        .collect();
    assert_eq!(jsonl_files.len(), 1, "should create exactly one JSONL file");

    // Verify header is readable
    let (header, entries) = SessionReader::read_all(&jsonl_files[0].path()).unwrap();
    assert_eq!(header.type_, "session");
    assert_eq!(header.cwd, "/test/cwd");
    assert_eq!(header.id, coord.session_id());
    assert!(entries.is_empty(), "fresh session should have no entries");
}

#[test]
fn session_coordinator_persists_messages_on_turn_end() {
    let dir = tempfile::tempdir().unwrap();
    let mut coord = SessionCoordinator::new(
        dir.path(),
        "/test",
        opi_agent::compaction::CompactionConfig::default(),
    )
    .unwrap();

    let messages = vec![
        AgentMessage::Llm(Message::User(UserMessage {
            content: vec![InputContent::Text {
                text: "Hello".into(),
            }],
            timestamp_ms: 0,
        })),
        AgentMessage::Llm(Message::User(UserMessage {
            content: vec![InputContent::Text {
                text: "World".into(),
            }],
            timestamp_ms: 0,
        })),
    ];

    coord.on_turn_end(&messages, &opi_ai::stream::Usage::default());

    // Read back
    let jsonl_path = dir.path().join(format!("{}.jsonl", coord.session_id()));
    let (_header, entries) = SessionReader::read_all(&jsonl_path).unwrap();
    assert_eq!(entries.len(), 2, "should have two message entries");
}

#[test]
fn session_coordinator_accumulates_usage() {
    let dir = tempfile::tempdir().unwrap();
    let mut coord = SessionCoordinator::new(
        dir.path(),
        "/test",
        opi_agent::compaction::CompactionConfig::default(),
    )
    .unwrap();

    let usage = opi_ai::stream::Usage {
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };

    coord.on_turn_end(&[], &usage);
    assert_eq!(coord.usage().turn_count(), 1);
    assert_eq!(coord.usage().total_input_tokens(), 100);
    assert_eq!(coord.usage().total_output_tokens(), 50);

    coord.on_turn_end(&[], &usage);
    assert_eq!(coord.usage().turn_count(), 2);
    assert_eq!(coord.usage().total_input_tokens(), 200);
}

// ---------------------------------------------------------------------------
// Harness session wiring tests
// ---------------------------------------------------------------------------
// These tests mutate OPI_SESSIONS_DIR so they must run sequentially.

static SESSION_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
async fn harness_creates_session_file_on_prompt() {
    let _lock = SESSION_TEST_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    set_sessions_dir(dir.path());

    let response = test_support::text_response("Hello!");
    let provider = MockProvider::new("mock", vec![response]);

    let mut harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
    );

    let result = harness.prompt("Hi").await.unwrap();
    assert!(!result.is_empty(), "should have messages");

    // Verify session was created
    let session = harness.session();
    assert!(session.is_some(), "harness should have an active session");

    // Verify JSONL file was written
    let jsonl_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "jsonl")
        })
        .collect();
    assert_eq!(jsonl_files.len(), 1, "should create one session file");

    let (header, entries) = SessionReader::read_all(&jsonl_files[0].path()).unwrap();
    assert_eq!(header.type_, "session");
    assert!(!entries.is_empty(), "session should have persisted entries");

    clear_sessions_dir();
}

// ---------------------------------------------------------------------------
// Reconstruct context tests
// ---------------------------------------------------------------------------

#[test]
fn reconstruct_context_from_session_entries() {
    let dir = tempfile::tempdir().unwrap();
    let header = make_header("test-sess", "/repo");
    let path = dir.path().join("test-sess.jsonl");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    writer.append(&test_message_entry("e1", "Hello")).unwrap();
    writer.append(&test_message_entry("e2", "World")).unwrap();

    let (_header, entries) = SessionReader::read_all(&path).unwrap();
    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    assert_eq!(messages.len(), 2);
    for msg in &messages {
        assert!(
            matches!(msg, AgentMessage::Llm(Message::User(_))),
            "should reconstruct LLM messages"
        );
    }
}

#[test]
fn reconstruct_context_skips_non_message_entries() {
    use opi_agent::session::CompactionEntry;

    let dir = tempfile::tempdir().unwrap();
    let header = make_header("test-sess", "/repo");
    let path = dir.path().join("test-sess.jsonl");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    writer.append(&test_message_entry("e1", "Hello")).unwrap();
    writer
        .append(&SessionEntry::Compaction(CompactionEntry {
            id: "c1".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:02Z".into(),
            summary: "compacted".into(),
            first_kept_entry_id: "e1".into(),
            tokens_before: 100,
            tokens_after: 50,
        }))
        .unwrap();
    writer.append(&test_message_entry("e2", "World")).unwrap();

    let (_header, entries) = SessionReader::read_all(&path).unwrap();
    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    assert_eq!(
        messages.len(),
        2,
        "should skip compaction entries and only return messages"
    );
}

// ---------------------------------------------------------------------------
// Full lifecycle test: write, read, verify
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_lifecycle_write_read_verify() {
    let _lock = SESSION_TEST_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    set_sessions_dir(dir.path());

    // Create harness and run a prompt
    let response = test_support::text_response("I am an assistant");
    let provider = MockProvider::new("mock", vec![response]);

    let mut harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
    );

    let _result = harness.prompt("Who are you?").await.unwrap();

    // Get the session ID
    let session_id = harness.session().unwrap().session_id().to_owned();

    // Verify the session file can be read back
    let session_path = dir.path().join(format!("{session_id}.jsonl"));
    assert!(session_path.exists(), "session file should exist");

    let (header, entries) = SessionReader::read_all(&session_path).unwrap();
    assert_eq!(header.id, session_id);
    assert!(!entries.is_empty(), "should have persisted entries");

    // Verify reconstruct produces the right number of messages
    let reconstructed = opi_coding_agent::session_cli::reconstruct_context(&entries);
    assert!(
        !reconstructed.is_empty(),
        "reconstructed messages should not be empty"
    );

    clear_sessions_dir();
}

// ---------------------------------------------------------------------------
// Multi-turn session persistence test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multi_turn_session_persistence() {
    let _lock = SESSION_TEST_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    set_sessions_dir(dir.path());

    let first = test_support::text_response("First response");
    let second = test_support::text_response("Second response");
    let provider = MockProvider::new("mock", vec![first, second]);

    let mut harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
    );

    let result1 = harness.prompt("Hello").await.unwrap();
    let result2 = harness.continue_("Tell me more").await.unwrap();

    assert!(result1.len() >= 2, "first turn should have messages");
    assert!(result2.len() >= 4, "second turn should have more messages");

    // Check usage accumulation
    let session = harness.session().unwrap();
    assert_eq!(
        session.usage().turn_count(),
        2,
        "should track 2 turns"
    );

    // Read back session file
    let session_id = session.session_id().to_owned();
    let session_path = dir.path().join(format!("{session_id}.jsonl"));
    let (_header, entries) = SessionReader::read_all(&session_path).unwrap();

    // Should have entries from both turns
    assert!(
        entries.len() >= 4,
        "should have entries from both turns, got {}",
        entries.len()
    );

    clear_sessions_dir();
}
