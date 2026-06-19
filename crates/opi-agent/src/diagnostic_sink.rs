//! Minimal in-process diagnostic sink (Phase 7 task 7.2).
//!
//! This module provides the emission substrate that runtime failure paths
//! (provider, retry, cancellation, tool execution, compaction, session
//! recovery, package/adapter, config, RPC startup) record [`Diagnostic`]s
//! into. It is intentionally minimal: a [`DiagnosticSink`] trait plus two
//! implementations.
//!
//! Scope note: this is NOT the durable local trace envelope or trace sink.
//! That structured, schema-versioned, redacted trace store is a later task
//! (Phase 7 "trace envelope and trace sink"). Here we only need a place
//! failure paths can push a [`Diagnostic`] and tests can observe the sequence.
//!
//! The trait is [`Send`] + [`Sync`] and object-safe so the runtime can hold it
//! behind an `Arc<dyn DiagnosticSink>`; the concrete implementation can then
//! vary between tests ([`RecordingSink`]) and production (a [`NullSink`] today,
//! a trace sink later) without changing call sites.

use std::sync::Mutex;

use crate::Diagnostic;

/// Receives diagnostic observations emitted from runtime failure paths.
///
/// Implementations must be cheap to call from failure paths: `record` must not
/// perform network I/O, block on locks held across `.await`s, or panic on
/// normal inputs.
pub trait DiagnosticSink: Send + Sync {
    /// Record a single diagnostic observation.
    fn record(&self, diagnostic: Diagnostic);
}

/// A sink that records every diagnostic in emission order.
///
/// Used by tests and by in-process captures that want the full ordered
/// sequence. Records are guarded by a [`Mutex`]; lock poisoning is treated as
/// unreachable (consistent with the rest of the crate) and recovered by
/// reading the inner data anyway.
#[derive(Default)]
pub struct RecordingSink {
    records: Mutex<Vec<Diagnostic>>,
}

impl RecordingSink {
    /// Create an empty recording sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lock the inner record buffer, recovering the data even if a prior holder
    /// poisoned the mutex.
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Diagnostic>> {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Number of diagnostics recorded so far.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether no diagnostics have been recorded.
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Return a snapshot (clone) of the recorded diagnostics in emission order.
    ///
    /// Returning a `Vec` clone keeps the lock short and lets callers inspect the
    /// sequence without holding the mutex.
    pub fn snapshot(&self) -> Vec<Diagnostic> {
        self.lock().clone()
    }

    /// Clear all recorded diagnostics (Phase 7 task 7.5).
    ///
    /// Used by the coding harness to scope run-summary diagnostic counts to the
    /// current run when a single sink is shared across runs (e.g. an RPC
    /// session issues multiple prompt runs against one harness).
    pub fn clear(&self) {
        self.lock().clear();
    }
}

impl DiagnosticSink for RecordingSink {
    fn record(&self, diagnostic: Diagnostic) {
        self.lock().push(diagnostic);
    }
}

/// A sink that discards every diagnostic.
///
/// The default production sink until a durable trace sink is wired in: failure
/// paths still call `record`, so the diagnostic is classified and the call site
/// exists, but nothing is retained in-process.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl DiagnosticSink for NullSink {
    fn record(&self, _diagnostic: Diagnostic) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    #[test]
    fn recording_sink_round_trip_and_clear() {
        let sink = RecordingSink::new();
        assert!(sink.is_empty());
        sink.record(Diagnostic::new(Severity::Info, "a", "agent", "one"));
        sink.record(Diagnostic::new(Severity::Error, "b", "agent", "two"));
        let snap = sink.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].code, "a");
        assert_eq!(snap[1].code, "b");
        assert_eq!(sink.len(), 2);
        assert!(!sink.is_empty());
    }

    #[test]
    fn null_sink_records_nothing() {
        let sink = NullSink;
        sink.record(Diagnostic::new(Severity::Info, "x", "agent", "ignored"));
        // Nothing to observe; the contract is silent discard.
        let _ = sink;
    }
}
