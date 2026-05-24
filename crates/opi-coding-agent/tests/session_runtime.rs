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
    unsafe {
        std::env::set_var("OPI_SESSIONS_DIR", dir);
    }
}

/// Remove env var safely (edition 2024 requires unsafe for remove_var).
fn clear_sessions_dir() {
    // SAFETY: test-only env var mutation; no other thread reads this var
    // concurrently during the test.
    unsafe {
        std::env::remove_var("OPI_SESSIONS_DIR");
    }
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
        "anthropic:claude-sonnet-4",
    )
    .unwrap();

    // Verify file exists
    let jsonl_files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
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
        "anthropic:claude-sonnet-4",
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

    coord
        .on_turn_end_simple(&messages, &opi_ai::stream::Usage::default())
        .unwrap();

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
        "anthropic:claude-sonnet-4",
    )
    .unwrap();

    let usage = opi_ai::stream::Usage {
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };

    coord.on_turn_end_simple(&[], &usage).unwrap();
    assert_eq!(coord.usage().turn_count(), 1);
    assert_eq!(coord.usage().total_input_tokens(), 100);
    assert_eq!(coord.usage().total_output_tokens(), 50);

    coord.on_turn_end_simple(&[], &usage).unwrap();
    assert_eq!(coord.usage().turn_count(), 2);
    assert_eq!(coord.usage().total_input_tokens(), 200);
}

// ---------------------------------------------------------------------------
// Harness session wiring tests
// ---------------------------------------------------------------------------
// These tests mutate OPI_SESSIONS_DIR so they must run sequentially.

static SESSION_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquire the session-test lock, recovering from any prior poisoning.
fn session_lock() -> std::sync::MutexGuard<'static, ()> {
    match SESSION_TEST_LOCK.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn harness_creates_session_file_on_prompt() {
    let _lock = session_lock();
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
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
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
fn reconstruct_context_includes_compaction_summaries() {
    use opi_agent::message::AgentMessage;
    use opi_agent::session::CompactionEntry;

    let dir = tempfile::tempdir().unwrap();
    let header = make_header("test-sess", "/repo");
    let path = dir.path().join("test-sess.jsonl");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    // first_kept_entry_id points at e2 — e1 must be dropped, e2 kept.
    writer.append(&test_message_entry("e1", "Hello")).unwrap();
    writer.append(&test_message_entry("e2", "World")).unwrap();
    writer
        .append(&SessionEntry::Compaction(CompactionEntry {
            id: "c1".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:02Z".into(),
            summary: "compacted".into(),
            first_kept_entry_id: "e2".into(),
            tokens_before: 100,
            tokens_after: 50,
        }))
        .unwrap();
    writer
        .append(&test_message_entry("e3", "Follow up"))
        .unwrap();

    let (_header, entries) = SessionReader::read_all(&path).unwrap();
    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    // Expected: [summary(c1), e2, e3]. The kept tail (e2) was already in
    // JSONL before the Compaction entry — runtime never re-emits kept
    // entries after the marker.
    assert_eq!(
        messages.len(),
        3,
        "compaction preserves kept tail from before the marker plus post-compaction entries"
    );
    assert!(
        matches!(&messages[0], AgentMessage::CompactionSummary(cs) if cs.summary == "compacted")
    );
    let text_at = |idx: usize| -> String {
        match &messages[idx] {
            AgentMessage::Llm(Message::User(u)) => match &u.content[0] {
                InputContent::Text { text } => text.clone(),
                _ => panic!("expected text content"),
            },
            _ => panic!("expected user llm message at {idx}"),
        }
    };
    assert_eq!(text_at(1), "World", "kept-tail entry e2 must survive");
    assert_eq!(
        text_at(2),
        "Follow up",
        "post-compaction entry e3 must survive"
    );
}

#[test]
fn reconstruct_context_missing_first_kept_id_falls_back_to_summary_only() {
    // Defensive: a corrupt/forward-incompatible session whose first_kept_entry_id
    // does not match any prior entry must not crash — drop the pre-summary tail.
    use opi_agent::message::AgentMessage;
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
            first_kept_entry_id: "missing".into(),
            tokens_before: 100,
            tokens_after: 50,
        }))
        .unwrap();
    writer.append(&test_message_entry("e2", "Post")).unwrap();

    let (_header, entries) = SessionReader::read_all(&path).unwrap();
    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    assert_eq!(messages.len(), 2);
    assert!(matches!(&messages[0], AgentMessage::CompactionSummary(_)));
    assert!(matches!(&messages[1], AgentMessage::Llm(_)));
}

// ---------------------------------------------------------------------------
// Full lifecycle test: write, read, verify
// ---------------------------------------------------------------------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn full_lifecycle_write_read_verify() {
    let _lock = session_lock();
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
#[allow(clippy::await_holding_lock)]
async fn multi_turn_session_persistence() {
    let _lock = session_lock();
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
    assert_eq!(session.usage().turn_count(), 2, "should track 2 turns");

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

// ---------------------------------------------------------------------------
// Compaction tests
// ---------------------------------------------------------------------------

#[test]
fn compaction_shrinks_buffer_and_returns_summary_plus_kept() {
    use opi_agent::compaction::CompactionConfig;
    use opi_ai::message::AssistantContent;
    use opi_ai::stream::Usage;

    let dir = tempfile::tempdir().unwrap();
    let mut coord = SessionCoordinator::new(
        dir.path(),
        "/test",
        // Tiny threshold so compaction triggers immediately.
        CompactionConfig {
            enabled: true,
            threshold_tokens: 1,
        },
        "anthropic:claude-sonnet-4",
    )
    .unwrap();

    let user = |t: &str| {
        AgentMessage::Llm(Message::User(UserMessage {
            content: vec![InputContent::Text { text: t.into() }],
            timestamp_ms: 0,
        }))
    };
    let assistant = |t: &str| {
        let mut a = test_support::base_assistant();
        a.content.push(AssistantContent::Text { text: t.into() });
        AgentMessage::Llm(Message::Assistant(a))
    };

    let messages: Vec<AgentMessage> = (0..8)
        .flat_map(|i| {
            vec![
                user(&format!("user message number {i} with extra padding text")),
                assistant(&format!(
                    "assistant response number {i} with extra padding text"
                )),
            ]
        })
        .collect();

    let usage = Usage {
        input_tokens: 100,
        output_tokens: 100,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };

    let out = coord
        .on_turn_end_simple(&messages, &usage)
        .unwrap()
        .expect("compaction should trigger above threshold");

    // After compaction the new buffer is [summary, ...kept_messages].
    assert!(matches!(
        &out.new_agent_messages[0],
        AgentMessage::CompactionSummary(_)
    ));
    assert!(
        out.new_agent_messages.len() < messages.len(),
        "compacted buffer must be smaller than input ({} vs {})",
        out.new_agent_messages.len(),
        messages.len()
    );
    // tokens_after should not exceed tokens_before — equality only happens
    // when nothing fit on the kept side (unusual for our 25% split).
    assert!(out.tokens_after <= out.tokens_before);
}

#[test]
fn compaction_engine_reads_pricing_and_reports_cost() {
    use opi_agent::compaction::CompactionConfig;
    use opi_ai::stream::Usage;

    let dir = tempfile::tempdir().unwrap();
    let mut coord = SessionCoordinator::new(
        dir.path(),
        "/test",
        CompactionConfig::default(),
        "anthropic:claude-sonnet-4",
    )
    .unwrap();

    // Sonnet pricing: $3/Mtok input, $15/Mtok output
    let usage = Usage {
        input_tokens: 1_000_000,
        output_tokens: 500_000,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };
    coord.on_turn_end_simple(&[], &usage).unwrap();

    let cost = coord.cost_summary().expect("sonnet pricing should resolve");
    assert!((cost.input_cost - 3.0).abs() < 1e-6);
    assert!((cost.output_cost - 7.5).abs() < 1e-6);
    assert!((cost.total_cost() - 10.5).abs() < 1e-6);
}

#[test]
fn cost_summary_returns_none_for_unknown_model() {
    use opi_agent::compaction::CompactionConfig;

    let dir = tempfile::tempdir().unwrap();
    let coord = SessionCoordinator::new(
        dir.path(),
        "/test",
        CompactionConfig::default(),
        "future:unknown-model",
    )
    .unwrap();

    assert!(coord.cost_summary().is_none());
}

// ---------------------------------------------------------------------------
// Resume: open_existing reuses the original session file
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Compaction events flow through harness subscribers
// ---------------------------------------------------------------------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn harness_emits_compaction_events_on_threshold() {
    use std::sync::Arc;

    let _lock = session_lock();
    let dir = tempfile::tempdir().unwrap();
    set_sessions_dir(dir.path());

    // Use threshold=0 so compaction triggers regardless of MockProvider usage
    // (which is zero by default).
    let mut config = OpiConfig::default();
    config.compaction.threshold_tokens = 0;

    let response = test_support::text_response("ok");
    let provider = MockProvider::new("mock", vec![response]);
    let mut harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        config,
        std::env::current_dir().unwrap(),
    );

    let starts = Arc::new(std::sync::Mutex::new(0u32));
    let ends = Arc::new(std::sync::Mutex::new(0u32));
    let starts_c = starts.clone();
    let ends_c = ends.clone();
    harness.subscribe(Box::new(move |event| match event {
        opi_agent::event::AgentEvent::CompactionStart { .. } => {
            *starts_c.lock().unwrap() += 1;
        }
        opi_agent::event::AgentEvent::CompactionEnd { .. } => {
            *ends_c.lock().unwrap() += 1;
        }
        _ => {}
    }));

    // Need at least two messages for compaction to be possible.
    harness.prompt("first prompt").await.unwrap();

    let s = *starts.lock().unwrap();
    let e = *ends.lock().unwrap();
    assert_eq!(s, e, "every CompactionStart should have a matching End");
    assert!(s >= 1, "expected at least one CompactionStart");

    clear_sessions_dir();
}

#[test]
fn open_existing_appends_to_original_file() {
    use opi_agent::compaction::CompactionConfig;
    use opi_ai::stream::Usage;

    let dir = tempfile::tempdir().unwrap();

    // Create initial session and persist a turn.
    let (session_path, session_id) = {
        let mut coord = SessionCoordinator::new(
            dir.path(),
            "/repo",
            CompactionConfig::default(),
            "anthropic:claude-sonnet-4",
        )
        .unwrap();
        let path = coord.session_path().to_path_buf();
        let id = coord.session_id().to_owned();

        let msg = AgentMessage::Llm(Message::User(UserMessage {
            content: vec![InputContent::Text {
                text: "first".into(),
            }],
            timestamp_ms: 0,
        }));
        coord.on_turn_end_simple(&[msg], &Usage::default()).unwrap();
        (path, id)
    };

    let (header_before, entries_before) =
        opi_agent::session::SessionReader::read_all(&session_path).unwrap();
    assert_eq!(entries_before.len(), 1);

    // Open the same file and append another turn.
    let mut resumed = SessionCoordinator::open_existing(
        session_path.clone(),
        session_id.clone(),
        &entries_before,
        1, // one prior agent message
        CompactionConfig::default(),
        "anthropic:claude-sonnet-4",
    )
    .unwrap();

    let msg = AgentMessage::Llm(Message::User(UserMessage {
        content: vec![InputContent::Text {
            text: "second".into(),
        }],
        timestamp_ms: 0,
    }));
    resumed
        .on_turn_end_simple(&[msg], &Usage::default())
        .unwrap();

    let (header_after, entries_after) =
        opi_agent::session::SessionReader::read_all(&session_path).unwrap();
    assert_eq!(header_after.id, header_before.id);
    assert_eq!(
        entries_after.len(),
        2,
        "resumed session should grow, not start over"
    );
}

#[test]
fn open_existing_preserves_kept_tail_after_compaction() {
    // The runtime writes the Compaction entry AFTER the kept-tail messages
    // were already persisted in earlier turns. Resuming must keep those tail
    // entries in the compaction buffer; otherwise future compactions / token
    // accounting see an empty post-summary buffer.
    use opi_agent::compaction::CompactionConfig;
    use opi_agent::session::{CompactionEntry, MessageEntry};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("resume.jsonl");
    let header = make_header("resume", "/repo");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    // Two pre-compaction entries.
    writer
        .append(&SessionEntry::Message(MessageEntry {
            id: "msg-1".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:00Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text { text: "old".into() }],
                timestamp_ms: 0,
            }),
        }))
        .unwrap();
    writer
        .append(&SessionEntry::Message(MessageEntry {
            id: "msg-2".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:01Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text {
                    text: "kept-tail".into(),
                }],
                timestamp_ms: 0,
            }),
        }))
        .unwrap();
    // Compaction: keep msg-2 as the first-kept entry. msg-1 is dropped.
    writer
        .append(&SessionEntry::Compaction(CompactionEntry {
            id: "cmp-1".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:02Z".into(),
            summary: "old context summary".into(),
            first_kept_entry_id: "msg-2".into(),
            tokens_before: 100,
            tokens_after: 30,
        }))
        .unwrap();
    drop(writer);

    let (_header, entries) = SessionReader::read_all(&path).unwrap();
    let resumed = SessionCoordinator::open_existing(
        path.clone(),
        "resume".into(),
        &entries,
        2, // post-resume agent buffer has [summary, msg-2]
        CompactionConfig::default(),
        "anthropic:claude-sonnet-4",
    )
    .unwrap();

    let kept = resumed.compaction_entries();
    assert_eq!(
        kept.len(),
        2,
        "entries should be [summary, msg-2] after resume with Compaction; got {} entries",
        kept.len(),
    );
    // First entry is the compaction summary.
    assert!(matches!(
        &kept[0].message,
        AgentMessage::CompactionSummary(_)
    ));
    // Second entry is the kept tail.
    assert_eq!(kept[1].id, "msg-2");
}

// ---------------------------------------------------------------------------
// Multi-assistant turn usage aggregation
// ---------------------------------------------------------------------------
//
// A single user prompt that triggers a tool call produces two assistant
// messages in one turn: the tool-call response, then the final response.
// Both carry their own `usage`. The cumulative session usage must include
// BOTH — not just the last one.

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn multi_assistant_turn_accumulates_all_assistant_usages() {
    use std::pin::Pin;

    use opi_agent::tool::{Tool, ToolError, ToolResult};
    use opi_ai::message::{AssistantContent, OutputContent, ToolCall, ToolDef};
    use opi_ai::stream::{AssistantStreamEvent, StopReason, Usage};
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    let _lock = session_lock();
    let dir = tempfile::tempdir().unwrap();
    set_sessions_dir(dir.path());

    // Tool-call assistant: 100 in / 30 out.
    let tool_call = ToolCall {
        id: "tc-1".into(),
        name: "noop".into(),
        arguments: "{}".into(),
    };
    let mut tool_partial = test_support::base_assistant();
    tool_partial.content.push(AssistantContent::ToolCall {
        tool_call: tool_call.clone(),
    });
    tool_partial.usage = Usage {
        input_tokens: 100,
        output_tokens: 30,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };
    let tool_response = vec![
        AssistantStreamEvent::Start {
            partial: test_support::base_assistant(),
        },
        AssistantStreamEvent::ToolCallEnd {
            content_index: 0,
            tool_call,
            partial: tool_partial.clone(),
        },
        AssistantStreamEvent::Done {
            reason: StopReason::ToolUse,
            message: tool_partial,
        },
    ];

    // Final text assistant: 200 in / 50 out.
    let mut final_partial = test_support::base_assistant();
    final_partial.content.push(AssistantContent::Text {
        text: "done".into(),
    });
    final_partial.usage = Usage {
        input_tokens: 200,
        output_tokens: 50,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };
    let final_response = vec![
        AssistantStreamEvent::Start {
            partial: test_support::base_assistant(),
        },
        AssistantStreamEvent::TextDelta {
            content_index: 0,
            delta: "done".into(),
            partial: final_partial.clone(),
        },
        AssistantStreamEvent::Done {
            reason: StopReason::Stop,
            message: final_partial,
        },
    ];

    let provider = MockProvider::new("mock", vec![tool_response, final_response]);

    // Minimal no-op tool so the agent can satisfy the tool call.
    struct NoopTool;
    impl Tool for NoopTool {
        fn definition(&self) -> ToolDef {
            ToolDef {
                name: "noop".into(),
                description: "noop tool".into(),
                input_schema: json!({"type": "object"}),
            }
        }
        fn execute(
            &self,
            _call_id: &str,
            _arguments: serde_json::Value,
            _signal: CancellationToken,
            _on_update: Option<opi_agent::tool::UpdateCallback>,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<ToolResult, ToolError>> + Send>>
        {
            Box::pin(async move {
                Ok(ToolResult {
                    content: vec![OutputContent::Text { text: "ok".into() }],
                    details: None,
                    is_error: false,
                    terminate: false,
                })
            })
        }
    }

    let mut harness = CodingHarness::new(
        Box::new(provider),
        "mock-model".into(),
        OpiConfig::default(),
        std::env::current_dir().unwrap(),
    );
    harness.add_tool(Box::new(NoopTool));

    let _ = harness.prompt("Use the tool").await.unwrap();

    let session = harness.session().expect("session should be active");
    let usage = session.usage();

    // The two assistant messages were emitted in the SAME user prompt, so
    // turn_count should be 1 (one persist_turn call) while tokens are the
    // sum of both assistants.
    assert_eq!(usage.turn_count(), 1, "one prompt should be one turn");
    assert_eq!(
        usage.total_input_tokens(),
        300,
        "input tokens must sum tool-call (100) + final (200)"
    );
    assert_eq!(
        usage.total_output_tokens(),
        80,
        "output tokens must sum tool-call (30) + final (50)"
    );

    clear_sessions_dir();
}

// ---------------------------------------------------------------------------
// Active-branch resume via Leaf entries
// ---------------------------------------------------------------------------

#[test]
fn reconstruct_context_follows_last_leaf_to_active_branch() {
    use opi_agent::message::AgentMessage;
    use opi_agent::session::LeafEntry;

    // A branched session: root e1 has two children e2a (branch A) and
    // e2b (branch B). Branch A has follow-up e3a; branch B has none. The
    // active branch is A (Leaf -> e3a). File-order replay would include
    // both branches; the active-branch walk must return only e1 -> e2a -> e3a.
    let dir = tempfile::tempdir().unwrap();
    let header = make_header("branch-sess", "/repo");
    let path = dir.path().join("branch-sess.jsonl");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    let user = |id: &str, parent: Option<&str>, text: &str| {
        SessionEntry::Message(MessageEntry {
            id: id.into(),
            parent_id: parent.map(|s| s.into()),
            timestamp: "2026-05-23T12:00:00Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text { text: text.into() }],
                timestamp_ms: 0,
            }),
        })
    };

    writer.append(&user("e1", None, "root")).unwrap();
    writer.append(&user("e2a", Some("e1"), "branch-a")).unwrap();
    writer.append(&user("e2b", Some("e1"), "branch-b")).unwrap();
    writer
        .append(&user("e3a", Some("e2a"), "follow-a"))
        .unwrap();
    writer
        .append(&SessionEntry::Leaf(LeafEntry {
            id: "l1".into(),
            parent_id: Some("e3a".into()),
            timestamp: "2026-05-23T12:00:01Z".into(),
            entry_id: "e3a".into(),
        }))
        .unwrap();
    drop(writer);

    let (_h, entries) = SessionReader::read_all(&path).unwrap();
    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    // Expect exactly the active branch: e1, e2a, e3a (the sibling e2b is
    // dropped).
    assert_eq!(
        messages.len(),
        3,
        "active-branch walk must drop sibling branch e2b; got {} messages",
        messages.len()
    );
    let text_at = |idx: usize| -> String {
        match &messages[idx] {
            AgentMessage::Llm(Message::User(u)) => match &u.content[0] {
                InputContent::Text { text } => text.clone(),
                _ => panic!("expected text content"),
            },
            _ => panic!("expected user llm message at {idx}"),
        }
    };
    assert_eq!(text_at(0), "root");
    assert_eq!(text_at(1), "branch-a");
    assert_eq!(text_at(2), "follow-a");
}

#[test]
fn reconstruct_context_uses_last_leaf_when_multiple_present() {
    use opi_agent::message::AgentMessage;
    use opi_agent::session::LeafEntry;

    // Two leaf entries written in order. The branch pointer is mutable, so
    // the last leaf wins — newer activity supersedes older.
    let dir = tempfile::tempdir().unwrap();
    let header = make_header("two-leaf-sess", "/repo");
    let path = dir.path().join("two-leaf-sess.jsonl");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    let user = |id: &str, parent: Option<&str>, text: &str| {
        SessionEntry::Message(MessageEntry {
            id: id.into(),
            parent_id: parent.map(|s| s.into()),
            timestamp: "2026-05-23T12:00:00Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text { text: text.into() }],
                timestamp_ms: 0,
            }),
        })
    };

    writer.append(&user("e1", None, "root")).unwrap();
    writer.append(&user("e2a", Some("e1"), "branch-a")).unwrap();
    writer.append(&user("e2b", Some("e1"), "branch-b")).unwrap();
    // Older leaf points at branch A.
    writer
        .append(&SessionEntry::Leaf(LeafEntry {
            id: "l-old".into(),
            parent_id: Some("e2a".into()),
            timestamp: "2026-05-23T12:00:01Z".into(),
            entry_id: "e2a".into(),
        }))
        .unwrap();
    // User switched to branch B; newer leaf wins.
    writer
        .append(&SessionEntry::Leaf(LeafEntry {
            id: "l-new".into(),
            parent_id: Some("e2b".into()),
            timestamp: "2026-05-23T12:00:02Z".into(),
            entry_id: "e2b".into(),
        }))
        .unwrap();
    drop(writer);

    let (_h, entries) = SessionReader::read_all(&path).unwrap();
    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    assert_eq!(
        messages.len(),
        2,
        "must follow newest leaf (-> e2b), not older one"
    );
    if let AgentMessage::Llm(Message::User(u)) = &messages[1]
        && let InputContent::Text { text } = &u.content[0]
    {
        assert_eq!(text, "branch-b");
    } else {
        panic!("expected branch-b at index 1");
    }
}

#[test]
fn reconstruct_context_without_leaf_falls_back_to_file_order() {
    // Sessions written by the current runtime do not yet emit Leaf entries.
    // Those linear sessions must continue to replay in file order so resume
    // keeps working.
    use opi_agent::message::AgentMessage;

    let dir = tempfile::tempdir().unwrap();
    let header = make_header("linear-sess", "/repo");
    let path = dir.path().join("linear-sess.jsonl");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    writer.append(&test_message_entry("e1", "a")).unwrap();
    writer.append(&test_message_entry("e2", "b")).unwrap();
    writer.append(&test_message_entry("e3", "c")).unwrap();
    drop(writer);

    let (_h, entries) = SessionReader::read_all(&path).unwrap();
    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    assert_eq!(messages.len(), 3);
    let text_at = |idx: usize| -> String {
        match &messages[idx] {
            AgentMessage::Llm(Message::User(u)) => match &u.content[0] {
                InputContent::Text { text } => text.clone(),
                _ => panic!("expected text"),
            },
            _ => panic!("expected user message"),
        }
    };
    assert_eq!(text_at(0), "a");
    assert_eq!(text_at(1), "b");
    assert_eq!(text_at(2), "c");
}

#[test]
fn reconstruct_context_active_branch_with_compaction() {
    // Active-branch walk must still apply compaction semantics: messages
    // on the active branch that precede `first_kept_entry_id` are dropped
    // and replaced by the summary, kept tail is preserved.
    use opi_agent::message::AgentMessage;
    use opi_agent::session::{CompactionEntry, LeafEntry};

    let dir = tempfile::tempdir().unwrap();
    let header = make_header("branch-compact-sess", "/repo");
    let path = dir.path().join("branch-compact-sess.jsonl");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    let user = |id: &str, parent: Option<&str>, text: &str| {
        SessionEntry::Message(MessageEntry {
            id: id.into(),
            parent_id: parent.map(|s| s.into()),
            timestamp: "2026-05-23T12:00:00Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text { text: text.into() }],
                timestamp_ms: 0,
            }),
        })
    };

    // Active chain: e1 -> e2 -> c1 (keeps e2) -> e3
    // Sibling: e2b (branch B) — must be dropped.
    writer.append(&user("e1", None, "old")).unwrap();
    writer.append(&user("e2", Some("e1"), "kept-tail")).unwrap();
    writer.append(&user("e2b", Some("e1"), "sibling")).unwrap();
    writer
        .append(&SessionEntry::Compaction(CompactionEntry {
            id: "c1".into(),
            parent_id: Some("e2".into()),
            timestamp: "2026-05-23T13:00:00Z".into(),
            summary: "summary".into(),
            first_kept_entry_id: "e2".into(),
            tokens_before: 100,
            tokens_after: 30,
        }))
        .unwrap();
    writer.append(&user("e3", Some("c1"), "post")).unwrap();
    writer
        .append(&SessionEntry::Leaf(LeafEntry {
            id: "l1".into(),
            parent_id: Some("e3".into()),
            timestamp: "2026-05-23T13:00:01Z".into(),
            entry_id: "e3".into(),
        }))
        .unwrap();
    drop(writer);

    let (_h, entries) = SessionReader::read_all(&path).unwrap();
    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    // Active chain after compaction: [summary, e2, e3]. Sibling e2b dropped.
    assert_eq!(messages.len(), 3, "expected summary + kept-tail + post");
    assert!(matches!(&messages[0], AgentMessage::CompactionSummary(cs) if cs.summary == "summary"));
    if let AgentMessage::Llm(Message::User(u)) = &messages[1]
        && let InputContent::Text { text } = &u.content[0]
    {
        assert_eq!(text, "kept-tail");
    } else {
        panic!("expected kept-tail at index 1");
    }
    if let AgentMessage::Llm(Message::User(u)) = &messages[2]
        && let InputContent::Text { text } = &u.content[0]
    {
        assert_eq!(text, "post");
    } else {
        panic!("expected post at index 2");
    }
}

// ---------------------------------------------------------------------------
// open_existing with branched LeafEntry — regression test
// ---------------------------------------------------------------------------

#[test]
fn open_existing_with_branched_leaf_excludes_sibling_from_compaction_buffer() {
    // Regression: open_existing() must follow the active branch (via
    // select_ordered_entries) when loading entries into the compaction buffer.
    // A naive file-order replay would include sibling branch messages.
    use opi_agent::compaction::CompactionConfig;
    use opi_agent::session::{LeafEntry, MessageEntry};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("branched.jsonl");
    let header = make_header("branched", "/repo");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    // Root message (shared by both branches)
    writer
        .append(&SessionEntry::Message(MessageEntry {
            id: "e1".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:00Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text {
                    text: "root".into(),
                }],
                timestamp_ms: 0,
            }),
        }))
        .unwrap();

    // Branch A: e1 -> e2a -> e3a
    writer
        .append(&SessionEntry::Message(MessageEntry {
            id: "e2a".into(),
            parent_id: Some("e1".into()),
            timestamp: "2026-05-22T12:00:01Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text {
                    text: "branch-a-msg".into(),
                }],
                timestamp_ms: 0,
            }),
        }))
        .unwrap();

    // Branch B: e1 -> e2b (sibling)
    writer
        .append(&SessionEntry::Message(MessageEntry {
            id: "e2b".into(),
            parent_id: Some("e1".into()),
            timestamp: "2026-05-22T12:00:02Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text {
                    text: "branch-b-sibling".into(),
                }],
                timestamp_ms: 0,
            }),
        }))
        .unwrap();

    // Leaf points to e2a as the active branch tip.
    writer
        .append(&SessionEntry::Leaf(LeafEntry {
            id: "leaf-1".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:03Z".into(),
            entry_id: "e2a".into(),
        }))
        .unwrap();

    drop(writer);

    let (_header, entries) = SessionReader::read_all(&path).unwrap();
    let resumed = SessionCoordinator::open_existing(
        path,
        "branched".into(),
        &entries,
        2, // agent has [root, branch-a-msg]
        CompactionConfig::default(),
        "anthropic:claude-sonnet-4",
    )
    .unwrap();

    let buffer = resumed.compaction_entries();
    // Must contain root (e1) and branch A (e2a), but NOT sibling branch B (e2b).
    assert_eq!(
        buffer.len(),
        2,
        "expected 2 entries (root + branch A), got {} entries: {:?}",
        buffer.len(),
        buffer.iter().map(|e| &e.id).collect::<Vec<_>>()
    );
    assert_eq!(buffer[0].id, "e1", "first entry must be root");
    assert_eq!(buffer[1].id, "e2a", "second entry must be branch A");

    // Verify the sibling is definitely not present.
    let ids: Vec<&str> = buffer.iter().map(|e| e.id.as_str()).collect();
    assert!(
        !ids.contains(&"e2b"),
        "sibling branch B (e2b) must not appear in compaction buffer, got: {ids:?}"
    );
}

// ---------------------------------------------------------------------------
// open_existing replays usage from persisted assistant messages
// ---------------------------------------------------------------------------

#[test]
fn open_existing_replays_usage_from_assistant_messages() {
    use opi_agent::compaction::CompactionConfig;
    use opi_agent::session::MessageEntry;
    use opi_ai::message::{AssistantContent, Message};
    use opi_ai::stream::Usage;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage-replay.jsonl");
    let header = make_header("usage-replay", "/repo");
    let mut writer = SessionWriter::create(&path, header).unwrap();

    // Write a user message (no usage).
    writer
        .append(&SessionEntry::Message(MessageEntry {
            id: "msg-1".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:00Z".into(),
            message: Message::User(UserMessage {
                content: vec![InputContent::Text {
                    text: "hello".into(),
                }],
                timestamp_ms: 0,
            }),
        }))
        .unwrap();

    // Write two assistant messages with usage.
    let mut asst1 = test_support::base_assistant();
    asst1
        .content
        .push(AssistantContent::Text { text: "hi".into() });
    asst1.usage = Usage {
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 10,
        cache_write_tokens: 5,
    };
    writer
        .append(&SessionEntry::Message(MessageEntry {
            id: "msg-2".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:01Z".into(),
            message: Message::Assistant(asst1),
        }))
        .unwrap();

    let mut asst2 = test_support::base_assistant();
    asst2.content.push(AssistantContent::Text {
        text: "world".into(),
    });
    asst2.usage = Usage {
        input_tokens: 200,
        output_tokens: 80,
        cache_read_tokens: 20,
        cache_write_tokens: 10,
    };
    writer
        .append(&SessionEntry::Message(MessageEntry {
            id: "msg-3".into(),
            parent_id: None,
            timestamp: "2026-05-22T12:00:02Z".into(),
            message: Message::Assistant(asst2),
        }))
        .unwrap();
    drop(writer);

    let (_header, entries) = SessionReader::read_all(&path).unwrap();
    let resumed = SessionCoordinator::open_existing(
        path,
        "usage-replay".into(),
        &entries,
        3,
        CompactionConfig::default(),
        "anthropic:claude-sonnet-4",
    )
    .unwrap();

    let usage = resumed.usage();
    assert_eq!(
        usage.turn_count(),
        1,
        "should count 1 user message as 1 turn, not 2 assistant messages"
    );
    assert_eq!(
        usage.total_input_tokens(),
        300,
        "input tokens must sum both assistants"
    );
    assert_eq!(
        usage.total_output_tokens(),
        130,
        "output tokens must sum both assistants"
    );
    assert_eq!(
        usage.total_cache_read_tokens(),
        30,
        "cache read tokens must sum both assistants"
    );
    assert_eq!(
        usage.total_cache_write_tokens(),
        15,
        "cache write tokens must sum both assistants"
    );
}
