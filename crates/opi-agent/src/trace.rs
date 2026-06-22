//! Unstable local trace envelope and trace sink (Phase 7 task 7.3).
//!
//! This module is the in-process substrate for a local, explicit, on-demand
//! trace of a single agent run. It is deliberately NOT telemetry: nothing is
//! sent anywhere, nothing is persisted unless a caller constructs a
//! [`FileTraceSink`], and no run is traced by default. The envelope is 0.x and
//! may change shape between releases until the runtime-event mapping
//! stabilizes in a later phase; every record carries [`TRACE_SCHEMA_VERSION`]
//! so consumers can branch on it.
//!
//! A trace is collected through a [`TraceCollector`] bound to one [`TraceSink`]
//! and an optional [`DiagnosticSink`]. The collector
//! models the run boundary:
//!
//! - [`TraceCollector::prepare`] runs **before** the run. A failure here (e.g.
//!   the trace file cannot be created) is surfaced to the caller so the run can
//!   be aborted — **fail closed**.
//! - [`TraceCollector::record`] ... `.emit()` runs **during** the run, once per
//!   record. A write failure here MUST NOT abort the run: the collector records
//!   a `trace_sink_failed` diagnostic, disables the sink, and drops further
//!   records — **fail open**.
//! - [`TraceCollector::finish`] runs **after** the run.
//!
//! Details are always redacted through [`crate::redact`] according to the
//! collector's [`RedactionMode`] before they
//! leave the process.
//!
//! Scope: this is substrate only. Wiring the collector into the agent loop is a
//! later task; nothing here touches `agent_loop`.
//!
//! Emission is intended to be single-threaded per run (one collector per run,
//! driven sequentially by the agent loop once wired in). The `disabled` flag
//! uses acquire/release ordering, which is sufficient for that usage.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use thiserror::Error;
use time::OffsetDateTime;

use crate::diagnostic::{RedactionMode, SOURCE_AGENT, Severity, code};
use crate::redact;
use crate::{Diagnostic, DiagnosticSink};

/// Unstable trace-envelope schema version stamped on every record.
///
/// This is NOT a stable public protocol version. The envelope is 0.x: the
/// record shape may change between releases until the runtime-event mapping
/// stabilizes. Consumers must tolerate additive or breaking changes.
pub const TRACE_SCHEMA_VERSION: u32 = 1;

/// What kind of event a [`TraceRecord`] describes.
///
/// `#[non_exhaustive]` because the runtime-event mapping is not yet frozen; a
/// later phase may add or refine kinds. The variants below cover the minimum
/// record categories the trace model requires (run, turn, provider, tool,
/// diagnostic-linked) plus the natural sub-events the agent loop already
/// produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TraceKind {
    RunStarted,
    RunEnded,
    TurnStarted,
    TurnEnded,
    ProviderRequest,
    ProviderStreamCompletion,
    ProviderRetry,
    ProviderFailure,
    ToolCallStarted,
    ToolCallCompleted,
    ToolCallFailed,
    ToolCallCancelled,
    /// A hook dispatch was skipped because the implementing extension or
    /// adapter did not declare that hook in its capabilities. Makes the
    /// "adapter implements only a subset" case visible in trace data. Details
    /// carry the hook name and the adapter/extension name.
    HookSkipped,
    /// A diagnostic observation linked into the trace; carries `severity` and
    /// `diagnostic_code`.
    DiagnosticLinked,
}

/// A single record in the unstable local trace envelope.
///
/// Serialize-only (like [`Diagnostic`]): records are emitted and written out,
/// never parsed back as typed values. `source` is `&'static str` for the same
/// reason as [`Diagnostic::source`] — it is a stable subsystem identifier.
#[derive(Debug, Clone, Serialize)]
pub struct TraceRecord {
    /// Unstable envelope version ([`TRACE_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// The run this record belongs to.
    pub run_id: String,
    /// The turn within the run, when applicable. Absent for run-scoped records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Monotonic per-run sequence number, assigned at emission time.
    pub sequence: u64,
    /// Wall-clock emission time as Unix seconds.
    pub timestamp: i64,
    /// Owning subsystem (e.g. `SOURCE_AGENT`, `SOURCE_PROVIDER`, `SOURCE_TOOL`).
    pub source: &'static str,
    /// Record category.
    pub kind: TraceKind,
    /// Diagnostic severity, present on diagnostic-linked records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    /// Stable diagnostic code, present on diagnostic-linked records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_code: Option<&'static str>,
    /// Redacted structured details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Trace-sink lifecycle errors.
///
/// Distinguished by phase so callers can tell a *before-run* failure
/// ([`TraceError::Prepare`], fail-closed) from a *during-run* failure
/// ([`TraceError::Write`], fail-open) or an *after-run* failure
/// ([`TraceError::Finish`]).
#[derive(Debug, Error)]
pub enum TraceError {
    /// Pre-run preparation failed (e.g. the trace file could not be created).
    #[error("trace sink prepare failed: {0}")]
    Prepare(#[source] std::io::Error),
    /// A during-run write failed.
    #[error("trace sink write failed: {0}")]
    Write(#[source] std::io::Error),
    /// A post-run finish failed.
    #[error("trace sink finish failed: {0}")]
    Finish(#[source] std::io::Error),
}

/// Where a trace envelope is durably written.
///
/// The three-method lifecycle mirrors a run boundary: [`TraceSink::prepare`]
/// before the run, [`TraceSink::write`] once per record during the run, and
/// [`TraceSink::finish`] after the run.
pub trait TraceSink: Send + Sync {
    /// Prepare the sink before a run. Failures here are fail-closed: the caller
    /// aborts the run rather than running without tracing.
    fn prepare(&self) -> Result<(), TraceError>;
    /// Write a single record during a run. Failures here are fail-open: the
    /// collector handles them and the run continues.
    fn write(&self, record: &TraceRecord) -> Result<(), TraceError>;
    /// Flush/close the sink after a run. Failures here are best-effort.
    fn finish(&self) -> Result<(), TraceError>;
}

/// Collects trace records for a single run.
///
/// Holds one [`TraceSink`] and an optional diagnostic sink. The collector
/// assigns monotonic sequence numbers, stamps the schema version and timestamp,
/// and redacts details according to its [`RedactionMode`] before writing.
pub struct TraceCollector {
    run_id: String,
    mode: RedactionMode,
    sequence: AtomicU64,
    sink: Arc<dyn TraceSink>,
    diagnostics: Option<Arc<dyn DiagnosticSink>>,
    disabled: AtomicBool,
}

impl TraceCollector {
    /// Create a collector for `run_id` that writes redacted records to `sink`,
    /// reporting sink failures (fail-open) to `diagnostics` when provided.
    pub fn new(
        run_id: impl Into<String>,
        mode: RedactionMode,
        sink: Arc<dyn TraceSink>,
        diagnostics: Option<Arc<dyn DiagnosticSink>>,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            mode,
            sequence: AtomicU64::new(0),
            sink,
            diagnostics,
            disabled: AtomicBool::new(false),
        }
    }

    /// Prepare the sink before the run (fail-closed).
    pub fn prepare(&self) -> Result<(), TraceError> {
        self.sink.prepare()
    }

    /// Begin building a trace record for `source` / `kind`.
    pub fn record(&self, source: &'static str, kind: TraceKind) -> TraceRecordBuilder<'_> {
        TraceRecordBuilder {
            collector: self,
            turn_id: None,
            source,
            kind,
            severity: None,
            diagnostic_code: None,
            details: None,
        }
    }

    /// Flush the sink after the run and stop accepting further records.
    pub fn finish(&self) {
        let _ = self.sink.finish();
        self.disable();
    }

    fn disable(&self) {
        self.disabled.store(true, Ordering::Release);
    }

    fn emit_inner(
        &self,
        turn_id: Option<String>,
        source: &'static str,
        kind: TraceKind,
        severity: Option<Severity>,
        diagnostic_code: Option<&'static str>,
        details: Option<serde_json::Value>,
    ) {
        if self.disabled.load(Ordering::Acquire) {
            return;
        }
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);
        let record = TraceRecord {
            schema_version: TRACE_SCHEMA_VERSION,
            run_id: self.run_id.clone(),
            turn_id,
            sequence,
            timestamp: OffsetDateTime::now_utc().unix_timestamp(),
            source,
            kind,
            severity,
            diagnostic_code,
            details: details.map(|value| redact(&value, self.mode)),
        };
        if self.sink.write(&record).is_err() {
            // Fail open: do not abort the run. Record a diagnostic, then drop
            // every remaining record so the sink is never written again.
            self.disable();
            if let Some(diag) = &self.diagnostics {
                diag.record(Diagnostic::new(
                    Severity::Warning,
                    code::CODE_TRACE_SINK_FAILED,
                    SOURCE_AGENT,
                    "trace sink disabled after write failure; remaining run records dropped",
                ));
            }
        }
    }
}

/// Builder for a single [`TraceRecord`] owned by a [`TraceCollector`].
pub struct TraceRecordBuilder<'a> {
    collector: &'a TraceCollector,
    turn_id: Option<String>,
    source: &'static str,
    kind: TraceKind,
    severity: Option<Severity>,
    diagnostic_code: Option<&'static str>,
    details: Option<serde_json::Value>,
}

impl<'a> TraceRecordBuilder<'a> {
    /// Attach a turn id (records are otherwise run-scoped).
    pub fn turn(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = Some(turn_id.into());
        self
    }

    /// Attach a diagnostic severity (diagnostic-linked records).
    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// Attach a stable diagnostic code (diagnostic-linked records).
    pub fn diagnostic_code(mut self, code: &'static str) -> Self {
        self.diagnostic_code = Some(code);
        self
    }

    /// Attach structured details (redacted on emit).
    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Emit the record through the collector.
    pub fn emit(self) {
        self.collector.emit_inner(
            self.turn_id,
            self.source,
            self.kind,
            self.severity,
            self.diagnostic_code,
            self.details,
        );
    }
}

/// In-memory trace sink that records every emitted [`TraceRecord`].
///
/// Intended for tests and in-process captures that want the full ordered
/// sequence. Production sinks write to disk ([`FileTraceSink`]).
#[derive(Default)]
pub struct RecordingTraceSink {
    records: Mutex<Vec<TraceRecord>>,
}

impl RecordingTraceSink {
    /// Create an empty recording trace sink.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<TraceRecord>> {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Snapshot (clone) of the recorded trace records in emission order.
    pub fn snapshot(&self) -> Vec<TraceRecord> {
        self.lock().clone()
    }

    /// Number of records recorded so far.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether no records have been recorded.
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Clear all recorded trace records.
    pub fn clear(&self) {
        self.lock().clear();
    }
}

impl TraceSink for RecordingTraceSink {
    fn prepare(&self) -> Result<(), TraceError> {
        self.clear();
        Ok(())
    }
    fn write(&self, record: &TraceRecord) -> Result<(), TraceError> {
        self.lock().push(record.clone());
        Ok(())
    }
    fn finish(&self) -> Result<(), TraceError> {
        Ok(())
    }
}

/// Trace sink that writes one JSONL line per record to a file.
///
/// The file is created (and truncated) only when [`TraceSink::prepare`] runs;
/// constructing the sink does not touch the filesystem, so tracing stays opt-in.
/// A `prepare` failure (missing parent directory, permission denied) is
/// fail-closed. Write failures during a run are handled by the collector
/// (fail-open).
///
/// Records are written through a line writer that flushes after every newline,
/// so a trace survives a mid-run process crash — the post-mortem use case the
/// trace exists for. OS-level durability (an explicit `fsync`) is not guaranteed.
pub struct FileTraceSink {
    path: PathBuf,
    writer: Mutex<Option<std::io::LineWriter<std::fs::File>>>,
}

impl FileTraceSink {
    /// Configure a sink for `path`. No file is created until [`TraceSink::prepare`].
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            writer: Mutex::new(None),
        }
    }

    /// The configured file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl TraceSink for FileTraceSink {
    fn prepare(&self) -> Result<(), TraceError> {
        let mut guard = self.writer.lock().unwrap_or_else(|p| p.into_inner());
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
            .map_err(TraceError::Prepare)?;
        *guard = Some(std::io::LineWriter::new(file));
        Ok(())
    }

    fn write(&self, record: &TraceRecord) -> Result<(), TraceError> {
        let mut guard = self.writer.lock().unwrap_or_else(|p| p.into_inner());
        let Some(writer) = guard.as_mut() else {
            return Err(TraceError::Write(std::io::Error::other(
                "trace sink used before prepare",
            )));
        };
        let line = serde_json::to_string(record)
            .map_err(|err| TraceError::Write(std::io::Error::other(err.to_string())))?;
        writer
            .write_all(line.as_bytes())
            .map_err(TraceError::Write)?;
        writer.write_all(b"\n").map_err(TraceError::Write)?;
        Ok(())
    }

    fn finish(&self) -> Result<(), TraceError> {
        let mut guard = self.writer.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(writer) = guard.as_mut() {
            writer.flush().map_err(TraceError::Finish)?;
        }
        Ok(())
    }
}
