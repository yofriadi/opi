//! Task 10.3 contract tests for the generic `AgentHarness` seam.
//!
//! These tests exercise the harness phase state machine, turn snapshots, save
//! points, pending-write ordering, runtime-config snapshot discipline, busy
//! rejections, queue safe points, cancellation cleanup, and abort semantics.
//! They depend ONLY on `opi-agent` and `opi-ai` (MockProvider) -- no
//! `opi-coding-agent` product policy -- proving the seam is usable by a
//! non-CLI library caller.

use opi_agent::harness::{
    AgentHarness, HarnessError, HarnessRuntimeConfig, HarnessSession, JsonlHarnessSession, Phase,
};
use opi_agent::hooks::AgentHooks;
use opi_agent::session::{SessionEntry, SessionHeader, SessionReader};
use opi_agent::session_event::CompactionResult;
use opi_agent::{Agent, AgentError, AgentLoopConfig, AgentMessage};
use opi_ai::message::{InputContent, Message, UserMessage};
use opi_ai::test_support::MockProvider;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Minimal no-op `AgentHooks`: converts `AgentMessage::Llm` payloads back to
/// provider messages. The contract tests never drive a real agent loop, but
/// `Agent::new` requires a hooks implementation.
struct NoopHooks;

impl AgentHooks for NoopHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(messages
            .iter()
            .filter_map(|m| match m {
                AgentMessage::Llm(inner) => Some(inner.clone()),
                _ => None,
            })
            .collect())
    }
}

/// In-memory `HarnessSession` that records appended entries in order. Used for
/// fast queue/ordering checks that do not need a real JSONL file.
#[derive(Default)]
struct RecordingHarnessSession {
    entries: Vec<SessionEntry>,
}

impl HarnessSession for RecordingHarnessSession {
    fn append(&mut self, entry: &SessionEntry) -> std::io::Result<()> {
        self.entries.push(entry.clone());
        Ok(())
    }
    fn message_count(&self) -> std::io::Result<usize> {
        Ok(self.entries.len())
    }
}

/// `HarnessSession` whose `append` always fails. Used to prove accepted
/// pending writes survive a flush failure and that abort reports them.
struct FailingHarnessSession;

impl HarnessSession for FailingHarnessSession {
    fn append(&mut self, _entry: &SessionEntry) -> std::io::Result<()> {
        Err(std::io::Error::other("simulated flush failure"))
    }
    fn message_count(&self) -> std::io::Result<usize> {
        Ok(0)
    }
}

fn build_agent() -> Agent {
    let provider = MockProvider::new("mock", vec![]);
    Agent::new(
        Box::new(provider),
        vec![],
        "mock:model".to_string(),
        None,
        AgentLoopConfig::default(),
        Box::new(NoopHooks),
    )
}

fn user_message(text: &str) -> Message {
    Message::User(UserMessage {
        content: vec![InputContent::Text {
            text: text.to_string(),
        }],
        timestamp_ms: 0,
    })
}

fn jsonl_session(dir: &tempfile::TempDir, name: &str) -> JsonlHarnessSession {
    let path = dir.path().join(name);
    let header = SessionHeader::new(
        format!("harness-{name}"),
        "0".to_string(),
        dir.path().to_string_lossy().into_owned(),
        None,
    );
    JsonlHarnessSession::create(&path, header).expect("create jsonl session")
}

// ---------------------------------------------------------------------------
// Acceptance scenario: phase10-generic-harness-seam (SC3)
// ---------------------------------------------------------------------------

/// A non-CLI library caller exercises the generic harness phase, snapshot,
/// save-point, pending-write, and runtime-config semantics.
#[test]
fn generic_harness_phase_snapshot_savepoint_contract() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("session.jsonl");
    let header = SessionHeader::new(
        "harness-test".to_string(),
        "0".to_string(),
        dir.path().to_string_lossy().into_owned(),
        None,
    );
    let session = JsonlHarnessSession::create(&path, header).expect("create session");
    let mut harness = AgentHarness::new(
        build_agent(),
        Box::new(session),
        HarnessRuntimeConfig::default(),
    );

    // Initial phase + snapshot.
    assert_eq!(harness.phase(), Phase::Idle);
    let snap = harness.snapshot();
    assert_eq!(snap.phase, Phase::Idle);
    assert_eq!(snap.pending_writes, 0);
    assert!(snap.last_save_point.is_none());

    // Enqueue an agent-emitted message, then a pending extension-state write.
    let order_msg = harness
        .enqueue_message(user_message("hello"))
        .expect("enqueue message");
    assert_eq!(order_msg, 1);
    assert_eq!(harness.snapshot().pending_writes, 1);

    let order_ext = harness
        .enqueue_extension_state(serde_json::json!({ "state": 1 }))
        .expect("enqueue extension state");
    assert_eq!(order_ext, 2);
    assert_eq!(harness.snapshot().pending_writes, 2);

    // Flush at the idle save point drains the whole queue.
    let sp = harness.flush().expect("flush");
    assert_eq!(sp.at_phase, Phase::Idle);
    assert_eq!(sp.pending_before, 2);
    assert_eq!(sp.pending_after, 0);
    let snap = harness.snapshot();
    assert_eq!(snap.pending_writes, 0);
    assert_eq!(snap.last_save_point, Some(sp));

    // Agent-emitted messages persist BEFORE pending extension/session writes.
    let (_hdr, entries) = SessionReader::read_all(&path).expect("read back");
    assert_eq!(entries.len(), 2);
    assert!(matches!(entries[0], SessionEntry::Message(_)));
    assert!(matches!(entries[1], SessionEntry::ExtensionState(_)));
    assert_eq!(harness.snapshot().message_count, 2);

    // Runtime-config mutation affects FUTURE snapshots, not the in-flight turn.
    harness
        .runtime_config()
        .set_model("mock:v2".to_string())
        .set_max_tokens(Some(8192));
    harness.begin_turn().expect("begin turn");
    assert_eq!(harness.phase(), Phase::Turn);
    // Snapshot frozen at turn start reflects v2.
    assert_eq!(harness.snapshot().runtime_config.model, "mock:v2");
    assert_eq!(
        harness.snapshot().runtime_config.loop_config.max_tokens,
        Some(8192)
    );
    // Mutate the builder while the turn is in flight; the in-flight snapshot
    // is frozen and must NOT reflect the new value.
    harness.runtime_config().set_model("mock:v3".to_string());
    assert_eq!(harness.snapshot().runtime_config.model, "mock:v2");
    // End the turn: the settlement flush returns an idle save point.
    let turn_sp = harness.end_turn().expect("end turn");
    assert_eq!(turn_sp.at_phase, Phase::Idle);
    // After the turn, the builder's new value is visible to the next snapshot.
    assert_eq!(harness.snapshot().runtime_config.model, "mock:v3");

    // Structural op rejected while busy (compaction requires idle).
    harness.begin_turn().expect("begin turn 2");
    match harness.begin_compaction() {
        Err(HarnessError::Busy(Phase::Turn)) => {}
        other => panic!("expected Busy(Turn), got {other:?}"),
    }
    harness.end_turn().expect("end turn 2");

    // Compaction round-trip: idle -> compaction -> idle, persists an entry.
    harness.begin_compaction().expect("begin compaction");
    assert_eq!(harness.phase(), Phase::Compaction);
    let result = CompactionResult {
        summary: "compacted".to_string(),
        first_kept_entry_id: "entry-1".to_string(),
        tokens_before: 1000,
        tokens_after: 500,
    };
    let comp_sp = harness.end_compaction(&result).expect("end compaction");
    assert_eq!(comp_sp.at_phase, Phase::Idle);
    assert_eq!(comp_sp.pending_after, 0);
    assert_eq!(harness.phase(), Phase::Idle);

    // Branch-summary phase round-trip (entry representation deferred to 10.5).
    harness
        .begin_branch_summary()
        .expect("begin branch summary");
    assert_eq!(harness.phase(), Phase::BranchSummary);
    let bs_sp = harness.end_branch_summary().expect("end branch summary");
    assert_eq!(bs_sp.at_phase, Phase::Idle);
    assert_eq!(harness.phase(), Phase::Idle);

    // The compaction entry landed durably, after the previously-flushed writes.
    let (_hdr2, entries2) = SessionReader::read_all(&path).expect("read back 2");
    assert!(
        entries2
            .iter()
            .any(|e| matches!(e, SessionEntry::Compaction(_)))
    );
}

// ---------------------------------------------------------------------------
// Acceptance scenario: phase10-harness-contracts
// ---------------------------------------------------------------------------

/// Contract tests for phase guards, busy rejections, queued operations,
/// cancellation cleanup, and session write ordering.
#[test]
fn phase_guards_busy_rejections_and_abort_cleanup() {
    let mut harness = AgentHarness::new(
        build_agent(),
        Box::new(RecordingHarnessSession::default()),
        HarnessRuntimeConfig::default(),
    );

    // A second begin_turn while Turn is busy is rejected.
    harness.begin_turn().expect("begin turn");
    assert_eq!(harness.phase(), Phase::Turn);
    match harness.begin_turn() {
        Err(HarnessError::Busy(Phase::Turn)) => {}
        other => panic!("expected Busy(Turn), got {other:?}"),
    }

    // Structural + queue ops rejected while Turn is busy.
    assert!(matches!(
        harness.enqueue_message(user_message("x")),
        Err(HarnessError::Busy(Phase::Turn))
    ));
    assert!(matches!(
        harness.enqueue_extension_state(serde_json::json!({})),
        Err(HarnessError::Busy(Phase::Turn))
    ));
    assert!(matches!(
        harness.begin_compaction(),
        Err(HarnessError::Busy(Phase::Turn))
    ));
    assert!(matches!(
        harness.begin_branch_summary(),
        Err(HarnessError::Busy(Phase::Turn))
    ));
    assert!(matches!(
        harness.flush(),
        Err(HarnessError::Busy(Phase::Turn))
    ));

    // Runtime-config setters are NEVER busy-rejected (they return &mut builder).
    harness.runtime_config().set_max_tokens(Some(2048));

    harness.end_turn().expect("end turn");

    // Compaction phase rejects queue ops + begin_turn.
    harness.begin_compaction().expect("begin compaction");
    assert_eq!(harness.phase(), Phase::Compaction);
    assert!(matches!(
        harness.enqueue_extension_state(serde_json::json!({})),
        Err(HarnessError::Busy(Phase::Compaction))
    ));
    assert!(matches!(
        harness.begin_turn(),
        Err(HarnessError::Busy(Phase::Compaction))
    ));
    let result = CompactionResult {
        summary: "s".to_string(),
        first_kept_entry_id: "e".to_string(),
        tokens_before: 1,
        tokens_after: 1,
    };
    harness.end_compaction(&result).expect("end compaction");

    // end_turn when idle is rejected (no matching phase to end).
    assert!(matches!(
        harness.end_turn(),
        Err(HarnessError::Busy(Phase::Idle))
    ));

    // -- Abort ordering invariant: agent message persists before extension
    //    write, and abort does not silently discard accepted pending writes.
    let dir = tempfile::tempdir().expect("tempdir abort");
    let path = dir.path().join("abort.jsonl");
    let header = SessionHeader::new(
        "abort-test".to_string(),
        "0".to_string(),
        dir.path().to_string_lossy().into_owned(),
        None,
    );
    let session = JsonlHarnessSession::create(&path, header).expect("create abort session");
    let mut h = AgentHarness::new(
        build_agent(),
        Box::new(session),
        HarnessRuntimeConfig::default(),
    );
    h.enqueue_message(user_message("m1")).expect("enqueue m1");
    h.enqueue_extension_state(serde_json::json!({ "k": "s1" }))
        .expect("enqueue s1");
    h.begin_turn().expect("begin turn");
    h.abort().expect("abort flushes accepted writes");
    assert_eq!(h.phase(), Phase::Idle);
    let (_hdr, entries) = SessionReader::read_all(&path).expect("read abort session");
    assert_eq!(entries.len(), 2);
    assert!(matches!(entries[0], SessionEntry::Message(_))); // m1 before s1
    assert!(matches!(entries[1], SessionEntry::ExtensionState(_)));

    // -- Abort cancels the agent's cancellation token.
    let mut hturn = AgentHarness::new(
        build_agent(),
        Box::new(RecordingHarnessSession::default()),
        HarnessRuntimeConfig::default(),
    );
    hturn.begin_turn().expect("begin turn");
    let token = hturn.cancel_token();
    assert!(!token.is_cancelled());
    hturn.abort().expect("abort");
    assert!(token.is_cancelled());
    assert_eq!(hturn.phase(), Phase::Idle);

    // -- Abort on a fresh harness with no pending writes is a no-op success.
    let mut hfresh = AgentHarness::new(
        build_agent(),
        Box::new(RecordingHarnessSession::default()),
        HarnessRuntimeConfig::default(),
    );
    hfresh.abort().expect("abort fresh");
    assert_eq!(hfresh.phase(), Phase::Idle);

    // -- Queue ordering across two idle cycles is preserved end-to-end.
    let dir2 = tempfile::tempdir().expect("tempdir cycles");
    let path2 = dir2.path().join("cycles.jsonl");
    let session2 = jsonl_session(&dir2, "cycles.jsonl");
    let mut hc = AgentHarness::new(
        build_agent(),
        Box::new(session2),
        HarnessRuntimeConfig::default(),
    );
    hc.enqueue_message(user_message("m1")).expect("m1");
    hc.enqueue_extension_state(serde_json::json!({ "i": 1 }))
        .expect("s1");
    hc.flush().expect("flush A");
    hc.enqueue_message(user_message("m2")).expect("m2");
    hc.enqueue_extension_state(serde_json::json!({ "i": 2 }))
        .expect("s2");
    hc.flush().expect("flush B");
    let (_hdr, entries) = SessionReader::read_all(&path2).expect("read cycles");
    assert_eq!(entries.len(), 4);
    assert!(matches!(entries[0], SessionEntry::Message(_)));
    assert!(matches!(entries[1], SessionEntry::ExtensionState(_)));
    assert!(matches!(entries[2], SessionEntry::Message(_)));
    assert!(matches!(entries[3], SessionEntry::ExtensionState(_)));

    // -- Flush failure preserves the queue: accepted pending writes are never lost.
    let mut hf = AgentHarness::new(
        build_agent(),
        Box::new(FailingHarnessSession),
        HarnessRuntimeConfig::default(),
    );
    hf.enqueue_extension_state(serde_json::json!({ "a": 1 }))
        .expect("enqueue a");
    hf.enqueue_extension_state(serde_json::json!({ "b": 2 }))
        .expect("enqueue b");
    assert_eq!(hf.snapshot().pending_writes, 2);
    match hf.flush() {
        Err(HarnessError::Write(_)) => {}
        other => panic!("expected Write error, got {other:?}"),
    }
    assert_eq!(hf.snapshot().pending_writes, 2);

    // -- Abort whose internal flush fails: reports unflushed count, resets phase
    //    to idle (reusable), and keeps the writes queued (not discarded).
    let mut hf2 = AgentHarness::new(
        build_agent(),
        Box::new(FailingHarnessSession),
        HarnessRuntimeConfig::default(),
    );
    hf2.enqueue_extension_state(serde_json::json!({ "a": 1 }))
        .expect("enqueue");
    let n = hf2.snapshot().pending_writes;
    hf2.begin_turn().expect("begin turn");
    match hf2.abort() {
        Err(HarnessError::AbortLeftPending(count)) => assert_eq!(count, n),
        other => panic!("expected AbortLeftPending({n}), got {other:?}"),
    }
    assert_eq!(hf2.phase(), Phase::Idle);
    assert_eq!(hf2.snapshot().pending_writes, n);
}
