//! Generic agent harness seam (Phase 10, Workstream 10.2).
//!
//! [`AgentHarness`] is the generic, product-agnostic orchestration layer above
//! [`crate::Agent`]. It owns the conversation lifecycle concerns that are NOT
//! specific to the coding-agent product:
//!
//! - explicit lifecycle [`Phase`]s (`Idle`, `Turn`, `Compaction`, `BranchSummary`);
//! - structural operations rejected while busy;
//! - queue operations accepted only at documented safe points (idle);
//! - runtime-config mutation that affects *future* turn snapshots, never an
//!   in-flight turn;
//! - a pending-write queue whose flush orders agent-emitted messages *before*
//!   pending extension/session writes;
//! - save points at operation settlement;
//! - `abort` that leaves no active operation and never silently discards an
//!   already-accepted pending write.
//!
//! `Agent` remains the low-level loop + provider + hooks runtime. `AgentHarness`
//! wraps an `Agent` by value and delegates control/cancel/message accessors to
//! it unchanged. In Phase 10.3 the phase transitions are state-machine guards
//! plus snapshot/save-point discipline: they do not themselves invoke the agent
//! loop. The coding-agent product harness (`opi_coding_agent::CodingHarness`)
//! will route its turn loop through this seam in task 10.4; until then this is a
//! published, contract-tested, unstable-0.x library surface.
//!
//! Branch summaries are a guarded lifecycle phase here, but their durable entry
//! representation is intentionally deferred to the session-facade work (task
//! 10.5), which owns the "branch summaries as context messages, metadata, or
//! both" decision recorded in Workstream 10.3.

use std::path::Path;

use opi_ai::message::Message;
use opi_ai::provider::ThinkingConfig;

use crate::agent::{Agent, AgentControl};
use crate::loop_types::AgentLoopConfig;
use crate::message::AgentMessage;
use crate::session::{
    CompactionEntry, ExtensionStateEntry, MessageEntry, SessionEntry, SessionHeader, SessionWriter,
};
use crate::session_event::CompactionResult;

/// Lifecycle phase of an [`AgentHarness`].
///
/// A harness is `Idle` between operations. `Turn` covers a single
/// provider/tool cycle; `Compaction` and `BranchSummary` are inter-turn
/// operations and may only begin from `Idle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No operation in progress. Queue operations and structural operations are
    /// accepted only in this phase.
    Idle,
    /// An agent turn is in progress.
    Turn,
    /// A context compaction is in progress.
    Compaction,
    /// A branch-summary operation is in progress.
    BranchSummary,
}

/// Errors returned by [`AgentHarness`] operations.
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    /// A structural or queue operation was attempted while the harness was busy
    /// in the carried phase.
    #[error("operation rejected: harness is busy in phase {0:?}")]
    Busy(Phase),
    /// A session write failed during a flush. Accepted pending writes that could
    /// not be flushed remain queued and are never discarded.
    #[error("session write failed: {0}")]
    Write(#[from] std::io::Error),
    /// [`AgentHarness::abort`] cancelled the active operation but its best-effort
    /// flush left the carried number of accepted pending writes unflushed. The
    /// harness is reset to [`Phase::Idle`] and the writes remain queued.
    #[error("abort left {0} accepted pending write(s) unflushed")]
    AbortLeftPending(usize),
}

/// Convenience alias.
pub type HarnessResult<T> = Result<T, HarnessError>;

/// Kind of a queued pending write. Flush order places
/// [`PendingWriteKind::AgentMessage`] writes before
/// [`PendingWriteKind::ExtensionState`] writes so agent-emitted messages
/// persist first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingWriteKind {
    /// An agent-emitted write (message or compaction entry).
    AgentMessage,
    /// A pending extension/session-state write.
    ExtensionState,
}

/// A queued durable write awaiting a save-point flush.
#[derive(Debug, Clone)]
pub struct PendingWrite {
    /// Monotonic sequence assigned at enqueue; preserves append order within a
    /// kind bucket even if the queue is reordered for flush.
    pub order: u64,
    /// Flush priority bucket.
    pub kind: PendingWriteKind,
    /// The durable session entry to append.
    pub entry: SessionEntry,
}

/// Ordered, priority-sorted queue of accepted pending writes.
///
/// [`PendingWriteQueue::drain_ordered`] yields [`PendingWriteKind::AgentMessage`]
/// writes before [`PendingWriteKind::ExtensionState`] writes, preserving
/// enqueue order within each bucket.
#[derive(Debug, Default)]
pub struct PendingWriteQueue {
    items: Vec<PendingWrite>,
    counter: u64,
}

impl PendingWriteQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the queue holds no pending writes.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of pending writes.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Append a write, returning its assigned monotonic order.
    pub fn enqueue(&mut self, entry: SessionEntry, kind: PendingWriteKind) -> u64 {
        self.counter += 1;
        let order = self.counter;
        self.items.push(PendingWrite { order, kind, entry });
        order
    }

    /// Drain the queue in flush order: agent-emitted writes first, then
    /// extension-state writes, each bucket stable by enqueue order.
    pub fn drain_ordered(&mut self) -> Vec<PendingWrite> {
        let mut items = std::mem::take(&mut self.items);
        items.sort_by_key(|pw| (kind_rank(pw.kind), pw.order));
        items
    }

    /// Re-insert writes that survived a failed flush, preserving their original
    /// order so a later flush re-sorts them identically.
    fn reinsert(&mut self, writes: Vec<PendingWrite>) {
        self.items.extend(writes);
    }
}

fn kind_rank(kind: PendingWriteKind) -> u8 {
    match kind {
        PendingWriteKind::AgentMessage => 0,
        PendingWriteKind::ExtensionState => 1,
    }
}

/// Frozen runtime configuration for a turn.
///
/// This wraps [`AgentLoopConfig`] plus the active model and is the unit a turn
/// snapshots. Mutations made through [`HarnessRuntimeConfigBuilder`] only take
/// effect for a *future* snapshot; an in-flight turn keeps its frozen snapshot.
#[derive(Debug, Clone, Default)]
pub struct HarnessRuntimeConfig {
    /// Loop parameters (max turns/tokens, retry, thinking, temperature).
    pub loop_config: AgentLoopConfig,
    /// Active model spec.
    pub model: String,
}

/// Live, mutable runtime configuration. Setters affect the next
/// [`HarnessRuntimeConfigBuilder::snapshot`], not any snapshot already issued.
#[derive(Debug)]
pub struct HarnessRuntimeConfigBuilder {
    loop_config: AgentLoopConfig,
    model: String,
}

impl HarnessRuntimeConfigBuilder {
    fn from_config(config: HarnessRuntimeConfig) -> Self {
        Self {
            loop_config: config.loop_config,
            model: config.model,
        }
    }

    /// Set the active model for future turns.
    pub fn set_model(&mut self, model: String) -> &mut Self {
        self.model = model;
        self
    }

    /// Set the max output tokens for future turns.
    pub fn set_max_tokens(&mut self, max_tokens: Option<u64>) -> &mut Self {
        self.loop_config.max_tokens = max_tokens;
        self
    }

    /// Set the thinking/reasoning configuration for future turns.
    pub fn set_thinking(&mut self, thinking: Option<ThinkingConfig>) -> &mut Self {
        self.loop_config.thinking = thinking;
        self
    }

    /// Set the retry configuration for future turns.
    pub fn set_retry(&mut self, retry: Option<opi_ai::retry::RetryConfig>) -> &mut Self {
        self.loop_config.retry = retry;
        self
    }

    /// Set the max turns for future turns.
    pub fn set_max_turns(&mut self, max_turns: u32) -> &mut Self {
        self.loop_config.max_turns = max_turns;
        self
    }

    /// Set the sampling temperature for future turns.
    pub fn set_temperature(&mut self, temperature: Option<f64>) -> &mut Self {
        self.loop_config.temperature = temperature;
        self
    }

    /// Freeze an immutable snapshot of the current runtime configuration.
    pub fn snapshot(&self) -> HarnessRuntimeConfig {
        HarnessRuntimeConfig {
            loop_config: self.loop_config.clone(),
            model: self.model.clone(),
        }
    }
}

/// A save point recorded when the pending-write queue is flushed.
///
/// `pending_before`/`pending_after` describe how many accepted writes were
/// drained and how many remain queued after the flush.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavePoint {
    /// Monotonic save-point sequence.
    pub seq: u64,
    /// Phase at which the save point was recorded.
    pub at_phase: Phase,
    /// Pending writes present before the flush.
    pub pending_before: usize,
    /// Pending writes remaining after the flush (0 on a fully successful flush).
    pub pending_after: usize,
}

/// Read-only snapshot of harness state.
#[derive(Debug, Clone)]
pub struct HarnessSnapshot {
    /// Current lifecycle phase.
    pub phase: Phase,
    /// Durable entry count reported by the session backend.
    pub message_count: usize,
    /// Accepted pending writes not yet flushed.
    pub pending_writes: usize,
    /// Runtime config visible to the current phase (frozen for an in-flight turn).
    pub runtime_config: HarnessRuntimeConfig,
    /// Most recent save point, if any.
    pub last_save_point: Option<SavePoint>,
}

/// Durable session backend for an [`AgentHarness`].
///
/// `append` is expected to be durable on success (the JSONL implementation
/// syncs each entry). This trait is the facade boundary that lets product
/// harnesses (e.g. `opi_coding_agent::CodingHarness` in task 10.4) bridge their
/// own session coordinator without forcing `opi-agent` to depend on it, and it
/// lets contract tests substitute recording/failing backends.
pub trait HarnessSession {
    /// Durable-append a session entry.
    fn append(&mut self, entry: &SessionEntry) -> std::io::Result<()>;
    /// Number of durable entries written so far.
    fn message_count(&self) -> std::io::Result<usize>;
}

/// JSONL-backed [`HarnessSession`] over the v1 [`SessionWriter`].
pub struct JsonlHarnessSession {
    writer: SessionWriter,
    count: usize,
}

impl JsonlHarnessSession {
    /// Create a new JSONL session file with the given header.
    pub fn create(path: &Path, header: SessionHeader) -> std::io::Result<Self> {
        let writer = SessionWriter::create(path, header)?;
        Ok(Self { writer, count: 0 })
    }
}

impl HarnessSession for JsonlHarnessSession {
    fn append(&mut self, entry: &SessionEntry) -> std::io::Result<()> {
        self.writer.append(entry)?;
        self.count += 1;
        Ok(())
    }

    fn message_count(&self) -> std::io::Result<usize> {
        Ok(self.count)
    }
}

/// Generic agent harness seam.
///
/// Wraps an [`Agent`] and a [`HarnessSession`] backend and enforces the
/// lifecycle/queue/save-point/runtime-config contract documented at the module
/// level. Construct with [`AgentHarness::new`].
pub struct AgentHarness {
    agent: Agent,
    session: Box<dyn HarnessSession>,
    queue: PendingWriteQueue,
    phase: Phase,
    config_builder: HarnessRuntimeConfigBuilder,
    turn_snapshot: Option<HarnessRuntimeConfig>,
    last_save_point: Option<SavePoint>,
    savepoint_seq: u64,
    last_entry_id: Option<String>,
    id_counter: u64,
}

impl AgentHarness {
    /// Create a new harness wrapping `agent`, persisting through `session`,
    /// initialized with `defaults` as the runtime configuration.
    pub fn new(
        agent: Agent,
        session: Box<dyn HarnessSession>,
        defaults: HarnessRuntimeConfig,
    ) -> Self {
        let config_builder = HarnessRuntimeConfigBuilder::from_config(defaults);
        Self {
            agent,
            session,
            queue: PendingWriteQueue::new(),
            phase: Phase::Idle,
            config_builder,
            turn_snapshot: None,
            last_save_point: None,
            savepoint_seq: 0,
            last_entry_id: None,
            id_counter: 0,
        }
    }

    /// Current lifecycle phase.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Read-only snapshot of harness state. During [`Phase::Turn`] the runtime
    /// config is the frozen turn snapshot, so mutations to the builder do not
    /// leak into the in-flight turn.
    pub fn snapshot(&self) -> HarnessSnapshot {
        let runtime_config = match self.phase {
            Phase::Turn => self
                .turn_snapshot
                .clone()
                .unwrap_or_else(|| self.config_builder.snapshot()),
            _ => self.config_builder.snapshot(),
        };
        HarnessSnapshot {
            phase: self.phase,
            message_count: self.session.message_count().unwrap_or(0),
            pending_writes: self.queue.len(),
            runtime_config,
            last_save_point: self.last_save_point,
        }
    }

    /// Live runtime configuration. Setters here affect only future turn
    /// snapshots, never an in-flight turn.
    pub fn runtime_config(&mut self) -> &mut HarnessRuntimeConfigBuilder {
        &mut self.config_builder
    }

    /// Enqueue an agent-emitted message write. Accepted only at [`Phase::Idle`];
    /// returns the assigned pending-write order.
    pub fn enqueue_message(&mut self, message: Message) -> HarnessResult<u64> {
        self.require_idle()?;
        let id = self.next_id();
        let parent_id = self.last_entry_id.take();
        let timestamp = self.next_timestamp();
        let entry = SessionEntry::Message(MessageEntry {
            id: id.clone(),
            parent_id,
            timestamp,
            message,
        });
        self.last_entry_id = Some(id);
        Ok(self.queue.enqueue(entry, PendingWriteKind::AgentMessage))
    }

    /// Enqueue a pending extension/session-state write. Accepted only at
    /// [`Phase::Idle`]. Always flushes after any agent-emitted message enqueued
    /// in the same batch.
    pub fn enqueue_extension_state(&mut self, state: serde_json::Value) -> HarnessResult<u64> {
        self.require_idle()?;
        let id = self.next_id();
        let parent_id = self.last_entry_id.take();
        let timestamp = self.next_timestamp();
        let entry = SessionEntry::ExtensionState(ExtensionStateEntry {
            id: id.clone(),
            parent_id,
            timestamp,
            state,
        });
        self.last_entry_id = Some(id);
        Ok(self.queue.enqueue(entry, PendingWriteKind::ExtensionState))
    }

    /// Flush the pending-write queue at a save point. Valid only at
    /// [`Phase::Idle`]. Agent-emitted writes are appended before
    /// extension-state writes.
    pub fn flush(&mut self) -> HarnessResult<SavePoint> {
        if self.phase != Phase::Idle {
            return Err(HarnessError::Busy(self.phase));
        }
        let pending_before = self.queue.len();
        self.flush_internal()?;
        let pending_after = self.queue.len();
        let sp = self.record_save_point(pending_before, pending_after);
        Ok(sp)
    }

    /// Begin an agent turn: [`Phase::Idle`] -> [`Phase::Turn`]. Freezes a
    /// runtime-config snapshot for the turn. (State-machine guard only in
    /// Phase 10.3; the loop itself is wired in task 10.4.)
    pub fn begin_turn(&mut self) -> HarnessResult<()> {
        self.require_idle()?;
        self.turn_snapshot = Some(self.config_builder.snapshot());
        self.phase = Phase::Turn;
        Ok(())
    }

    /// End the current turn: [`Phase::Turn`] -> [`Phase::Idle`] and flush at
    /// operation settlement. Returns the settlement save point.
    pub fn end_turn(&mut self) -> HarnessResult<SavePoint> {
        if self.phase != Phase::Turn {
            return Err(HarnessError::Busy(self.phase));
        }
        self.phase = Phase::Idle;
        self.turn_snapshot = None;
        self.flush()
    }

    /// Begin a compaction: [`Phase::Idle`] -> [`Phase::Compaction`].
    pub fn begin_compaction(&mut self) -> HarnessResult<()> {
        self.require_idle()?;
        self.phase = Phase::Compaction;
        Ok(())
    }

    /// End the current compaction: enqueues a compaction entry as an
    /// agent-emitted write, returns to [`Phase::Idle`], and flushes.
    pub fn end_compaction(&mut self, result: &CompactionResult) -> HarnessResult<SavePoint> {
        if self.phase != Phase::Compaction {
            return Err(HarnessError::Busy(self.phase));
        }
        let entry = self.build_compaction_entry(result);
        self.queue.enqueue(entry, PendingWriteKind::AgentMessage);
        self.phase = Phase::Idle;
        self.flush()
    }

    /// Begin a branch-summary operation: [`Phase::Idle`] ->
    /// [`Phase::BranchSummary`].
    pub fn begin_branch_summary(&mut self) -> HarnessResult<()> {
        self.require_idle()?;
        self.phase = Phase::BranchSummary;
        Ok(())
    }

    /// End the current branch-summary operation: [`Phase::BranchSummary`] ->
    /// [`Phase::Idle`] and flush. The durable branch-summary entry
    /// representation is deferred to task 10.5 (session-facade workstream).
    pub fn end_branch_summary(&mut self) -> HarnessResult<SavePoint> {
        if self.phase != Phase::BranchSummary {
            return Err(HarnessError::Busy(self.phase));
        }
        self.phase = Phase::Idle;
        self.flush()
    }

    /// Abort the active operation: cancel the wrapped agent, then best-effort
    /// flush ALL accepted pending writes. Never silently discards an accepted
    /// write. On a flush failure, returns [`HarnessError::AbortLeftPending`]
    /// with the number of unflushed writes; the harness is still reset to
    /// [`Phase::Idle`] (reusable) and the writes remain queued.
    pub fn abort(&mut self) -> HarnessResult<()> {
        self.agent.abort();
        let outcome = self.flush_internal();
        self.phase = Phase::Idle;
        self.turn_snapshot = None;
        match outcome {
            Ok(()) => Ok(()),
            Err(_) => Err(HarnessError::AbortLeftPending(self.queue.len())),
        }
    }

    /// Snapshot the wrapped agent's conversation buffer.
    pub fn messages_snapshot(&self) -> Vec<AgentMessage> {
        self.agent.messages_snapshot()
    }

    /// Clonable control handle for the wrapped agent.
    pub fn control_handle(&self) -> AgentControl {
        self.agent.control_handle()
    }

    /// Cancellation token for the wrapped agent.
    pub fn cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.agent.cancel_token()
    }

    // -- Internal helpers ---------------------------------------------------

    fn require_idle(&self) -> HarnessResult<()> {
        if self.phase != Phase::Idle {
            return Err(HarnessError::Busy(self.phase));
        }
        Ok(())
    }

    /// Drain the queue in order and append each entry. On a write failure, the
    /// unflushed tail is re-queued (order preserved) and the error is returned;
    /// the queue is never silently emptied across a failure.
    fn flush_internal(&mut self) -> Result<(), std::io::Error> {
        let mut ordered = self.queue.drain_ordered();
        let mut idx = 0;
        while idx < ordered.len() {
            if let Err(e) = self.session.append(&ordered[idx].entry) {
                let tail: Vec<_> = ordered.drain(idx..).collect();
                self.queue.reinsert(tail);
                return Err(e);
            }
            idx += 1;
        }
        Ok(())
    }

    fn record_save_point(&mut self, pending_before: usize, pending_after: usize) -> SavePoint {
        self.savepoint_seq += 1;
        let sp = SavePoint {
            seq: self.savepoint_seq,
            at_phase: self.phase,
            pending_before,
            pending_after,
        };
        self.last_save_point = Some(sp);
        sp
    }

    fn next_id(&mut self) -> String {
        self.id_counter += 1;
        format!("entry-{}", self.id_counter)
    }

    fn next_timestamp(&self) -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis().to_string())
            .unwrap_or_else(|_| "0".to_string())
    }

    fn build_compaction_entry(&mut self, result: &CompactionResult) -> SessionEntry {
        let id = self.next_id();
        let parent_id = self.last_entry_id.take();
        let timestamp = self.next_timestamp();
        let entry = SessionEntry::Compaction(CompactionEntry {
            id: id.clone(),
            parent_id,
            timestamp,
            summary: result.summary.clone(),
            first_kept_entry_id: result.first_kept_entry_id.clone(),
            tokens_before: result.tokens_before,
            tokens_after: result.tokens_after,
        });
        self.last_entry_id = Some(id);
        entry
    }
}
