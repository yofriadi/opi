//! Phase 7 task 7.3 — local trace envelope and trace sink.
//!
//! These tests pin the unstable 0.x trace envelope substrate that lives in
//! `opi-agent` before any CLI/RPC surface wires it up (that wiring is task
//! 7.5). They cover: the schema-version stamp, monotonic per-run sequence
//! numbers, the run/turn/provider/tool/diagnostic-linked record structure,
//! summary and verbose redaction of details, and the trace-sink failure
//! contract — fail closed before a run (a prepare error aborts) and fail open
//! during a run (a write error emits a `trace_sink_failed` diagnostic then
//! disables the sink so the run continues).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use opi_agent::diagnostic::code::*;
use opi_agent::diagnostic::{RedactionMode, SOURCE_AGENT, SOURCE_PROVIDER, SOURCE_TOOL, Severity};
use opi_agent::diagnostic_sink::RecordingSink;
use opi_agent::{
    FileTraceSink, RecordingTraceSink, TRACE_SCHEMA_VERSION, TraceCollector, TraceError, TraceKind,
    TraceRecord, TraceSink,
};

// A 20+ char Anthropic-style key body that SecretRedactor scrubs by value
// pattern (sk-ant-[a-zA-Z0-9]{20,}) in both Summary and Verbose modes.
const SCRUBBED_KEY: &str = "sk-ant-AAAAAAAAAAAAAAAAAAAAsecret";

// ---------------------------------------------------------------------------
// Schema version + envelope fields
// ---------------------------------------------------------------------------

#[test]
fn schema_version_is_unstable_v1() {
    // Unstable 0.x envelope; pin the literal so a bump is a deliberate change.
    assert_eq!(TRACE_SCHEMA_VERSION, 1u32);
}

#[test]
fn every_record_carries_schema_version_run_id_source() {
    let sink = Arc::new(RecordingTraceSink::new());
    let collector = TraceCollector::new("run-7", RedactionMode::default(), sink.clone(), None);
    collector
        .prepare()
        .expect("recording sink prepare is infallible");
    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();

    let rec = &sink.snapshot()[0];
    assert_eq!(rec.schema_version, TRACE_SCHEMA_VERSION);
    assert_eq!(rec.run_id, "run-7");
    assert_eq!(rec.source, SOURCE_AGENT);
    assert!(rec.timestamp > 0, "timestamp must be populated");
}

#[test]
fn trace_kind_serializes_to_stable_snake_case() {
    let cases: &[(TraceKind, &str)] = &[
        (TraceKind::RunStarted, "run_started"),
        (TraceKind::RunEnded, "run_ended"),
        (TraceKind::TurnStarted, "turn_started"),
        (TraceKind::TurnEnded, "turn_ended"),
        (TraceKind::ProviderRequest, "provider_request"),
        (
            TraceKind::ProviderStreamCompletion,
            "provider_stream_completion",
        ),
        (TraceKind::ProviderRetry, "provider_retry"),
        (TraceKind::ProviderFailure, "provider_failure"),
        (TraceKind::ToolCallStarted, "tool_call_started"),
        (TraceKind::ToolCallCompleted, "tool_call_completed"),
        (TraceKind::ToolCallFailed, "tool_call_failed"),
        (TraceKind::ToolCallCancelled, "tool_call_cancelled"),
        (TraceKind::DiagnosticLinked, "diagnostic_linked"),
    ];
    let mut seen = std::collections::HashSet::new();
    for (kind, expected) in cases {
        let json = serde_json::to_string(kind).expect("kind serializes");
        assert_eq!(
            json,
            format!("\"{expected}\""),
            "wrong snake_case for {kind:?}"
        );
        assert!(
            seen.insert(json),
            "two TraceKind variants collapsed to the same wire string"
        );
    }
}

#[test]
fn sequence_is_strictly_monotonic() {
    let sink = Arc::new(RecordingTraceSink::new());
    let collector = TraceCollector::new("run-seq", RedactionMode::default(), sink.clone(), None);
    collector.prepare().unwrap();
    for kind in [
        TraceKind::RunStarted,
        TraceKind::TurnStarted,
        TraceKind::ProviderRequest,
    ] {
        collector.record(SOURCE_AGENT, kind).emit();
    }
    let seqs: Vec<u64> = sink.snapshot().iter().map(|r| r.sequence).collect();
    assert_eq!(seqs, vec![0, 1, 2], "sequences must be strictly monotonic");
}

// ---------------------------------------------------------------------------
// run / turn / provider / tool / diagnostic-linked record structure
// ---------------------------------------------------------------------------

#[test]
fn run_records_have_no_turn_id_turn_records_do() {
    let sink = Arc::new(RecordingTraceSink::new());
    let collector = TraceCollector::new("run-rt", RedactionMode::default(), sink.clone(), None);
    collector.prepare().unwrap();

    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();
    collector
        .record(SOURCE_AGENT, TraceKind::TurnStarted)
        .turn("turn-1")
        .emit();

    let snap = sink.snapshot();
    // Run record carries no turn id and omits the field on the wire.
    assert!(snap[0].turn_id.is_none());
    let run_json = serde_json::to_string(&snap[0]).unwrap();
    assert!(
        !run_json.contains("turn_id"),
        "run record must omit turn_id, got: {run_json}"
    );
    // Turn record carries the turn id on the wire.
    assert_eq!(snap[1].turn_id.as_deref(), Some("turn-1"));
    let turn_json = serde_json::to_string(&snap[1]).unwrap();
    assert!(
        turn_json.contains("\"turn_id\":\"turn-1\""),
        "turn record must serialize turn_id, got: {turn_json}"
    );
}

#[test]
fn provider_and_tool_records_carry_their_sources() {
    let sink = Arc::new(RecordingTraceSink::new());
    let collector = TraceCollector::new("run-pt", RedactionMode::default(), sink.clone(), None);
    collector.prepare().unwrap();
    collector
        .record(SOURCE_PROVIDER, TraceKind::ProviderRequest)
        .emit();
    collector
        .record(SOURCE_TOOL, TraceKind::ToolCallStarted)
        .emit();

    let snap = sink.snapshot();
    assert_eq!(snap[0].source, SOURCE_PROVIDER);
    assert_eq!(snap[0].kind, TraceKind::ProviderRequest);
    assert_eq!(snap[1].source, SOURCE_TOOL);
    assert_eq!(snap[1].kind, TraceKind::ToolCallStarted);
}

#[test]
fn diagnostic_linked_record_carries_severity_and_code() {
    let sink = Arc::new(RecordingTraceSink::new());
    let collector = TraceCollector::new("run-dl", RedactionMode::default(), sink.clone(), None);
    collector.prepare().unwrap();
    collector
        .record(SOURCE_PROVIDER, TraceKind::DiagnosticLinked)
        .severity(Severity::Error)
        .diagnostic_code(CODE_PROVIDER_TIMEOUT)
        .emit();

    let rec = &sink.snapshot()[0];
    assert_eq!(rec.kind, TraceKind::DiagnosticLinked);
    assert_eq!(rec.severity, Some(Severity::Error));
    assert_eq!(rec.diagnostic_code, Some(CODE_PROVIDER_TIMEOUT));
}

// ---------------------------------------------------------------------------
// Redaction modes
// ---------------------------------------------------------------------------

#[test]
fn summary_mode_redacts_secret_and_prompt() {
    let sink = Arc::new(RecordingTraceSink::new());
    let collector = TraceCollector::new("run-sum", RedactionMode::Summary, sink.clone(), None);
    collector.prepare().unwrap();
    collector
        .record(SOURCE_AGENT, TraceKind::TurnStarted)
        .details(serde_json::json!({
            "prompt": "the user's secret plan",
            "api_key": SCRUBBED_KEY,
            "model": "gpt-4o",
            // A benign key whose VALUE is an absolute path: Summary's
            // ABSOLUTE_PATH_RE branch must redact it, which is Summary-specific.
            "workspace": "/home/alice/project",
        }))
        .emit();

    let snapshot = sink.snapshot();
    let details = snapshot[0].details.as_ref().expect("details present");
    assert_eq!(
        details["prompt"], "[REDACTED]",
        "summary redacts prompt content"
    );
    assert_eq!(details["api_key"], "[REDACTED]", "summary redacts api_key");
    assert_eq!(
        details["workspace"], "[REDACTED]",
        "summary redacts absolute-path values"
    );
    assert_eq!(details["model"], "gpt-4o", "summary keeps benign fields");
}

#[test]
fn verbose_mode_keeps_prompt_redacts_secret() {
    let sink = Arc::new(RecordingTraceSink::new());
    let collector = TraceCollector::new("run-vrb", RedactionMode::Verbose, sink.clone(), None);
    collector.prepare().unwrap();
    collector
        .record(SOURCE_AGENT, TraceKind::TurnStarted)
        .details(serde_json::json!({
            "prompt": "the user's secret plan",
            "api_key": SCRUBBED_KEY,
            "model": "gpt-4o",
            "workspace": "/home/alice/project",
        }))
        .emit();

    let snapshot = sink.snapshot();
    let details = snapshot[0].details.as_ref().expect("details present");
    assert_eq!(
        details["prompt"], "the user's secret plan",
        "verbose keeps benign prompt content"
    );
    assert_eq!(
        details["api_key"], "[REDACTED]",
        "verbose still redacts secrets"
    );
    assert_eq!(
        details["workspace"], "/home/alice/project",
        "verbose keeps absolute-path values (Summary-only redaction)"
    );
    assert_eq!(details["model"], "gpt-4o");
}

#[test]
fn redaction_recurses_into_nested_structures() {
    // Summary and Verbose redaction both recurse into nested objects/arrays;
    // a bug that only scrubbed top-level keys would leak nested secrets.
    let nested = serde_json::json!({
        "outer": {
            "api_key": SCRUBBED_KEY,
            "prompt": "nested plan",
            "workspace": "/home/alice/project",
        },
        "items": [
            { "token": "eyJabcdefghij.aaa.aaa" },
        ],
    });

    let summary_sink = Arc::new(RecordingTraceSink::new());
    let summary = TraceCollector::new(
        "run-nest-sum",
        RedactionMode::Summary,
        summary_sink.clone(),
        None,
    );
    summary.prepare().unwrap();
    summary
        .record(SOURCE_AGENT, TraceKind::TurnStarted)
        .details(nested.clone())
        .emit();
    let summary_snap = summary_sink.snapshot();
    let sum = summary_snap[0].details.as_ref().unwrap();
    assert_eq!(
        sum["outer"]["api_key"], "[REDACTED]",
        "nested api_key (summary)"
    );
    assert_eq!(
        sum["outer"]["prompt"], "[REDACTED]",
        "nested prompt (summary)"
    );
    assert_eq!(
        sum["outer"]["workspace"], "[REDACTED]",
        "nested absolute path (summary)"
    );
    assert_eq!(
        sum["items"][0]["token"], "[REDACTED]",
        "array token (summary)"
    );

    let verbose_sink = Arc::new(RecordingTraceSink::new());
    let verbose = TraceCollector::new(
        "run-nest-vrb",
        RedactionMode::Verbose,
        verbose_sink.clone(),
        None,
    );
    verbose.prepare().unwrap();
    verbose
        .record(SOURCE_AGENT, TraceKind::TurnStarted)
        .details(nested)
        .emit();
    let verbose_snap = verbose_sink.snapshot();
    let vrb = verbose_snap[0].details.as_ref().unwrap();
    assert_eq!(
        vrb["outer"]["api_key"], "[REDACTED]",
        "nested api_key (verbose)"
    );
    assert_eq!(
        vrb["outer"]["prompt"], "nested plan",
        "verbose keeps nested prompt content"
    );
    assert_eq!(
        vrb["outer"]["workspace"], "/home/alice/project",
        "verbose keeps nested absolute path"
    );
    assert_eq!(
        vrb["items"][0]["token"], "[REDACTED]",
        "array token (verbose)"
    );
}

#[test]
fn redaction_mode_default_is_summary() {
    // Safe by default: RedactionMode::default() must be Summary.
    assert_eq!(RedactionMode::default(), RedactionMode::Summary);
}

// ---------------------------------------------------------------------------
// Trace sink failure contract
// ---------------------------------------------------------------------------

/// A sink whose `prepare` always fails, to exercise fail-closed behavior.
struct FailingPrepareSink;

impl TraceSink for FailingPrepareSink {
    fn prepare(&self) -> Result<(), TraceError> {
        Err(TraceError::Prepare(std::io::Error::other(
            "permission denied",
        )))
    }
    fn write(&self, _record: &TraceRecord) -> Result<(), TraceError> {
        Ok(())
    }
    fn finish(&self) -> Result<(), TraceError> {
        Ok(())
    }
}

/// A sink whose `write` always fails and counts attempts, to exercise fail-open.
struct FailingWriteSink {
    writes: AtomicU64,
}

impl FailingWriteSink {
    fn new() -> Self {
        Self {
            writes: AtomicU64::new(0),
        }
    }
    fn write_attempts(&self) -> u64 {
        self.writes.load(Ordering::SeqCst)
    }
}

impl TraceSink for FailingWriteSink {
    fn prepare(&self) -> Result<(), TraceError> {
        Ok(())
    }
    fn write(&self, _record: &TraceRecord) -> Result<(), TraceError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Err(TraceError::Write(std::io::Error::other("disk full")))
    }
    fn finish(&self) -> Result<(), TraceError> {
        Ok(())
    }
}

/// A sink whose `finish` always fails and counts writes, to confirm finish
/// errors are swallowed (best-effort) and the collector still disables.
struct FailingFinishSink {
    writes: AtomicU64,
}

impl FailingFinishSink {
    fn new() -> Self {
        Self {
            writes: AtomicU64::new(0),
        }
    }
    fn write_attempts(&self) -> u64 {
        self.writes.load(Ordering::SeqCst)
    }
}

impl TraceSink for FailingFinishSink {
    fn prepare(&self) -> Result<(), TraceError> {
        Ok(())
    }
    fn write(&self, _record: &TraceRecord) -> Result<(), TraceError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn finish(&self) -> Result<(), TraceError> {
        Err(TraceError::Finish(std::io::Error::other("flush failed")))
    }
}

#[test]
fn prepare_failure_aborts_before_run_fail_closed() {
    let collector = TraceCollector::new(
        "run-fc",
        RedactionMode::default(),
        Arc::new(FailingPrepareSink),
        None,
    );
    // Fail closed: a prepare error surfaces to the caller so the run aborts.
    let result = collector.prepare();
    assert!(
        result.is_err(),
        "prepare failure must be surfaced (fail closed), got {result:?}"
    );
}

#[test]
fn file_sink_prepare_fails_on_missing_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A path whose parent does not exist cannot be created by OpenOptions.
    let missing = dir.path().join("does_not_exist").join("trace.jsonl");
    let sink = FileTraceSink::new(&missing);
    assert!(
        sink.prepare().is_err(),
        "prepare must fail when the parent dir is missing"
    );
}

#[test]
fn write_failure_emits_diagnostic_and_disables_sink_fail_open() {
    let trace_sink = Arc::new(FailingWriteSink::new());
    let diag_sink = Arc::new(RecordingSink::new());
    let collector = TraceCollector::new(
        "run-fo",
        RedactionMode::default(),
        trace_sink.clone(),
        Some(diag_sink.clone()),
    );
    collector.prepare().unwrap();

    // First emit: write fails -> emit a diagnostic and disable the sink.
    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();

    assert_eq!(
        trace_sink.write_attempts(),
        1,
        "the failing write was attempted exactly once"
    );
    let diags = diag_sink.snapshot();
    assert_eq!(diags.len(), 1, "exactly one diagnostic is emitted");
    let d = &diags[0];
    assert_eq!(d.code, CODE_TRACE_SINK_FAILED);
    assert_eq!(d.source, SOURCE_AGENT);
    assert_eq!(d.severity, Severity::Warning);
}

#[test]
fn fail_open_subsequent_emits_are_noops() {
    let trace_sink = Arc::new(FailingWriteSink::new());
    let diag_sink = Arc::new(RecordingSink::new());
    let collector = TraceCollector::new(
        "run-fo2",
        RedactionMode::default(),
        trace_sink.clone(),
        Some(diag_sink.clone()),
    );
    collector.prepare().unwrap();

    // First emit disables the sink; the second must be a no-op.
    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();
    collector
        .record(SOURCE_PROVIDER, TraceKind::ProviderRequest)
        .emit();
    collector
        .record(SOURCE_TOOL, TraceKind::ToolCallStarted)
        .emit();

    assert_eq!(
        trace_sink.write_attempts(),
        1,
        "subsequent emits must not call write after the sink is disabled"
    );
    assert_eq!(
        diag_sink.snapshot().len(),
        1,
        "no second diagnostic after the sink is disabled"
    );
}

#[test]
fn file_sink_writes_versioned_jsonl_in_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("trace.jsonl");
    let sink = Arc::new(FileTraceSink::new(&path));
    let collector = TraceCollector::new("run-file", RedactionMode::default(), sink.clone(), None);
    collector.prepare().expect("prepare creates the file");
    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();
    collector
        .record(SOURCE_PROVIDER, TraceKind::ProviderRequest)
        .turn("turn-1")
        .emit();
    collector
        .record(SOURCE_TOOL, TraceKind::ToolCallStarted)
        .turn("turn-1")
        .emit();
    collector.finish();

    let contents = fs::read_to_string(&path).expect("trace file written");
    let records: Vec<serde_json::Value> = contents
        .trim_end()
        .split('\n')
        .map(|line| serde_json::from_str(line).expect("each line is a JSON record"))
        .collect();
    assert_eq!(records.len(), 3, "three records written in order");
    // Every record carries the unstable schema version.
    for r in &records {
        assert_eq!(r["schema_version"], TRACE_SCHEMA_VERSION);
        assert_eq!(r["run_id"], "run-file");
    }
    assert_eq!(records[0]["sequence"], 0);
    assert_eq!(records[1]["sequence"], 1);
    assert_eq!(records[2]["sequence"], 2);
    assert_eq!(records[0]["kind"], "run_started");
    assert_eq!(records[1]["kind"], "provider_request");
    assert_eq!(records[2]["kind"], "tool_call_started");
}

#[test]
fn file_not_created_until_prepared_opt_in() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("not_yet.jsonl");
    let sink = FileTraceSink::new(&path);
    // Constructing the sink must not touch the filesystem.
    assert!(!path.exists(), "file must not exist until prepare runs");
    sink.prepare().expect("prepare creates the file");
    assert!(path.exists(), "file exists after prepare");
}

#[test]
fn fail_open_works_without_diagnostic_sink() {
    // No DiagnosticSink attached: a write failure must still disable the sink
    // and must not panic.
    let trace_sink = Arc::new(FailingWriteSink::new());
    let collector = TraceCollector::new(
        "run-fo-none",
        RedactionMode::default(),
        trace_sink.clone(),
        None,
    );
    collector.prepare().unwrap();
    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();
    assert_eq!(trace_sink.write_attempts(), 1, "write attempted once");
    // Subsequent emit is a no-op (disabled), proving disable happened even with
    // no diagnostic sink to observe it.
    collector
        .record(SOURCE_PROVIDER, TraceKind::ProviderRequest)
        .emit();
    assert_eq!(
        trace_sink.write_attempts(),
        1,
        "disabled: no further writes without a diagnostic sink"
    );
}

#[test]
fn emit_after_finish_is_a_noop() {
    let trace_sink = Arc::new(RecordingTraceSink::new());
    let collector = TraceCollector::new(
        "run-fin",
        RedactionMode::default(),
        trace_sink.clone(),
        None,
    );
    collector.prepare().unwrap();
    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();
    let before = trace_sink.len();
    collector.finish();
    // Emitting after finish() must not reach the sink.
    collector.record(SOURCE_AGENT, TraceKind::RunEnded).emit();
    assert_eq!(trace_sink.len(), before, "emit after finish is a no-op");
}

#[test]
fn finish_error_is_swallowed_and_disables() {
    // finish() is best-effort: a finish error must not propagate and the sink
    // is disabled so no further records are written.
    let trace_sink = Arc::new(FailingFinishSink::new());
    let collector = TraceCollector::new(
        "run-finerr",
        RedactionMode::default(),
        trace_sink.clone(),
        None,
    );
    collector.prepare().unwrap();
    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();
    assert_eq!(trace_sink.write_attempts(), 1, "first emit written");
    // finish() returns () and swallows the Finish error.
    collector.finish();
    // After finish, emit is a no-op even though FailingFinishSink::write would
    // succeed — the unchanged counter proves disable.
    collector.record(SOURCE_AGENT, TraceKind::RunEnded).emit();
    assert_eq!(
        trace_sink.write_attempts(),
        1,
        "finish disables the collector even when finish() errored"
    );
}

#[test]
fn file_sink_fail_open_when_written_before_prepare() {
    // Misuse guard: writing through a FileTraceSink before prepare returns a
    // Write error, which the collector treats as fail-open (disable + diagnostic).
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("unprepared.jsonl");
    let trace_sink = Arc::new(FileTraceSink::new(&path));
    let diag_sink = Arc::new(RecordingSink::new());
    let collector = TraceCollector::new(
        "run-noprep",
        RedactionMode::default(),
        trace_sink.clone(),
        Some(diag_sink.clone()),
    );

    // Deliberately no prepare(): the emit hits the not-prepared write guard.
    collector.record(SOURCE_AGENT, TraceKind::RunStarted).emit();

    let diags = diag_sink.snapshot();
    assert_eq!(
        diags.len(),
        1,
        "write-before-prepare emits trace_sink_failed"
    );
    assert_eq!(diags[0].code, CODE_TRACE_SINK_FAILED);
    assert!(!path.exists(), "no file created without prepare");
}
