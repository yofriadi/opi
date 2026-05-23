//! Session management CLI operations (task 2.7).
//!
//! Provides list/resume/delete operations for session JSONL files stored in
//! the platform-specific sessions directory (S9.2).

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Errors from session CLI operations.
#[derive(Debug, Error)]
pub enum SessionCliError {
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("session directory error: {0}")]
    Io(#[from] std::io::Error),
    #[error("session file corrupt: {0}")]
    Corrupt(String),
}

/// Summary of a session for display purposes.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub timestamp: String,
    pub cwd: String,
    pub parent_session: Option<String>,
}

/// Result of a resume operation.
pub struct ResumedSession {
    pub header: opi_agent::session::SessionHeader,
    pub entries: Vec<opi_agent::session::SessionEntry>,
    /// Filesystem path of the resumed session JSONL file. Used by the harness
    /// to open the file in append mode instead of creating a new session.
    pub path: PathBuf,
    /// Number of corrupt/unparseable entries skipped during load.
    pub skipped_entries: usize,
}

/// Return the platform-specific session storage directory (S9.2).
///
/// Checks `OPI_SESSIONS_DIR` env var first (for testing), then falls back to
/// the platform default.
///
/// Unix: `~/.local/share/opi/sessions/`
/// Windows: `%LOCALAPPDATA%\opi\sessions\`
pub fn session_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OPI_SESSIONS_DIR") {
        return PathBuf::from(dir);
    }
    if cfg!(windows) {
        std::env::var("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("opi").join("sessions"))
            .unwrap_or_else(|_| PathBuf::from(".opi").join("sessions"))
    } else {
        dirs_home()
            .map(|h| h.join(".local").join("share").join("opi").join("sessions"))
            .unwrap_or_else(|| PathBuf::from(".opi").join("sessions"))
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Validate that a session ID is safe to use as a filename component.
/// Rejects empty strings, path separators, and `..` traversal.
fn validate_session_id(id: &str) -> Result<(), SessionCliError> {
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(SessionCliError::NotFound(id.into()));
    }
    Ok(())
}

/// List all sessions in the given directory.
///
/// Returns session metadata parsed from the first line (header) of each `.jsonl`
/// file. Corrupt or unreadable files are silently skipped.
pub fn list_sessions(dir: &Path) -> Result<Vec<SessionInfo>, SessionCliError> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut sessions = Vec::new();
    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let first_line = match content.lines().next() {
            Some(line) => line,
            None => continue,
        };

        let header: opi_agent::session::SessionHeader = match serde_json::from_str(first_line) {
            Ok(h) => h,
            Err(_) => continue,
        };

        sessions.push(SessionInfo {
            id: header.id,
            timestamp: header.timestamp,
            cwd: header.cwd,
            parent_session: header.parent_session,
        });
    }

    Ok(sessions)
}

/// Resume a session by reading its header and entries.
pub fn resume_session(dir: &Path, session_id: &str) -> Result<ResumedSession, SessionCliError> {
    validate_session_id(session_id)?;
    let path = dir.join(format!("{session_id}.jsonl"));
    if !path.exists() {
        return Err(SessionCliError::NotFound(session_id.into()));
    }

    let (header, entries, recovery) = opi_agent::session::SessionReader::read_with_recovery(&path)
        .map_err(|e| SessionCliError::Corrupt(format!("{}: {e}", path.display())))?;

    let skipped_entries = recovery.corrupt_count();

    Ok(ResumedSession {
        header,
        entries,
        path,
        skipped_entries,
    })
}

/// Delete a session file by ID.
pub fn delete_session(dir: &Path, session_id: &str) -> Result<(), SessionCliError> {
    validate_session_id(session_id)?;
    let path = dir.join(format!("{session_id}.jsonl"));
    if !path.exists() {
        return Err(SessionCliError::NotFound(session_id.into()));
    }
    std::fs::remove_file(&path)?;
    Ok(())
}

/// Format a list of session info for stdout display.
pub fn format_sessions(sessions: &[SessionInfo]) -> String {
    if sessions.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    for s in sessions {
        let mut line = format!("{}  {}  {}", s.id, s.timestamp, s.cwd);
        if let Some(parent) = &s.parent_session {
            line.push_str(&format!("  (parent: {parent})"));
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Handle session CLI dispatch.
///
/// Returns `(handled, Some(ResumedSession))` for `--resume`,
/// `(true, None)` for list/delete (caller should exit),
/// `(false, None)` if no session command was given (normal execution continues).
pub fn handle_session_cli(
    list: bool,
    resume: Option<&str>,
    delete: Option<&str>,
) -> Result<(bool, Option<ResumedSession>), i32> {
    let dir = session_dir();

    if list {
        match list_sessions(&dir) {
            Ok(sessions) => {
                let output = format_sessions(&sessions);
                if !output.is_empty() {
                    println!("{output}");
                }
                Ok((true, None))
            }
            Err(e) => {
                eprintln!("opi: {e}");
                Err(1)
            }
        }
    } else if let Some(id) = resume {
        match resume_session(&dir, id) {
            Ok(session) => {
                // Print to stderr so it doesn't corrupt NDJSON stdout in --json mode.
                eprintln!(
                    "Resuming session {} ({} entries, cwd: {})",
                    session.header.id,
                    session.entries.len(),
                    session.header.cwd,
                );
                if session.skipped_entries > 0 {
                    eprintln!(
                        "opi: warning: {} corrupt entry/entries skipped in session {}",
                        session.skipped_entries, session.header.id,
                    );
                }
                Ok((true, Some(session)))
            }
            Err(e) => {
                eprintln!("opi: {e}");
                Err(1)
            }
        }
    } else if let Some(id) = delete {
        match delete_session(&dir, id) {
            Ok(()) => {
                println!("Deleted session {id}");
                Ok((true, None))
            }
            Err(e) => {
                eprintln!("opi: {e}");
                Err(1)
            }
        }
    } else {
        Ok((false, None))
    }
}

/// Reconstruct agent messages from session entries for resume.
///
/// Two modes:
///
/// 1. **Active-branch mode (with `Leaf` entries).** The session file holds
///    `leaf` pointer entries that record the active branch tip. When one or
///    more `Leaf` entries are present, this function uses the last `Leaf`'s
///    `entry_id` as the tip and walks the parent chain backward via
///    `parent_id`, collecting only the entries on that branch. This is
///    required for any session that contains branches — file-order replay
///    would otherwise interleave messages from sibling branches into the
///    reconstructed context.
///
/// 2. **Legacy linear mode (no `Leaf` entries).** Sessions written by the
///    current runtime do not yet emit `Leaf` markers; for those the entire
///    file is one linear branch and we replay every `Message`/`Compaction`
///    entry in file order.
///
/// Compaction entries are honored in both modes by replaying their
/// semantics: when a `Compaction` entry is encountered, every previously
/// collected message that precedes the entry whose id equals
/// `first_kept_entry_id` is dropped, the compaction summary is inserted in
/// its place, and the kept tail (already persisted before the marker) is
/// preserved. Messages written after the compaction marker are then
/// appended as usual.
///
/// Defensive fallback: if a `Compaction` entry's `first_kept_entry_id` does
/// not match any collected entry (corrupt or forward-incompatible session),
/// the pre-summary buffer is dropped entirely so the summary still appears
/// and post-compaction entries continue to accumulate.
pub fn reconstruct_context(
    entries: &[opi_agent::session::SessionEntry],
) -> Vec<opi_agent::message::AgentMessage> {
    let ordered = select_ordered_entries(entries);
    apply_entries(&ordered)
}

/// Return session entries ordered by the active branch.
///
/// When the session contains `Leaf` pointer entries, the last Leaf's
/// `entry_id` is used as the branch tip and the parent chain is walked
/// backward to collect only the active-branch entries (root to tip).
/// Without Leaves, all non-Leaf entries are returned in file order (legacy
/// linear sessions). This is the shared ordering logic used by both
/// `reconstruct_context` (Agent message buffer) and
/// `SessionCoordinator::open_existing` (compaction buffer).
pub(crate) fn select_ordered_entries(
    entries: &[opi_agent::session::SessionEntry],
) -> Vec<&opi_agent::session::SessionEntry> {
    use opi_agent::session::SessionEntry;

    let last_leaf_tip: Option<&str> = entries.iter().rev().find_map(|e| match e {
        SessionEntry::Leaf(l) => Some(l.entry_id.as_str()),
        _ => None,
    });

    match last_leaf_tip {
        Some(tip) => walk_active_branch(entries, tip),
        None => entries
            .iter()
            .filter(|e| !matches!(e, SessionEntry::Leaf(_)))
            .collect(),
    }
}

/// Walk the active branch backward from `tip_entry_id`, returning entries
/// from root to tip (ancestors first). `Leaf` entries themselves are
/// excluded from the result — they are pointers, not content.
///
/// If the tip id is not found, returns an empty vector; callers fall back
/// to legacy behavior or treat the resume as empty depending on context.
fn walk_active_branch<'a>(
    entries: &'a [opi_agent::session::SessionEntry],
    tip_entry_id: &str,
) -> Vec<&'a opi_agent::session::SessionEntry> {
    use std::collections::HashMap;

    use opi_agent::session::SessionEntry;

    let mut by_id: HashMap<&str, &SessionEntry> = HashMap::new();
    for entry in entries {
        let id = match entry {
            SessionEntry::Message(m) => Some(m.id.as_str()),
            SessionEntry::Compaction(c) => Some(c.id.as_str()),
            // Leaf pointers are excluded from the chain; the tip references
            // a Message/Compaction directly.
            SessionEntry::Leaf(_) => None,
            _ => None,
        };
        if let Some(id) = id {
            by_id.insert(id, entry);
        }
    }

    let mut chain: Vec<&SessionEntry> = Vec::new();
    let mut cursor: Option<&str> = Some(tip_entry_id);
    let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
    while let Some(id) = cursor {
        if !visited.insert(id) {
            // Cycle in parent_id graph: stop walking to avoid an infinite
            // loop on a corrupt file.
            break;
        }
        let Some(entry) = by_id.get(id).copied() else {
            break;
        };
        chain.push(entry);
        cursor = match entry {
            SessionEntry::Message(m) => m.parent_id.as_deref(),
            SessionEntry::Compaction(c) => c.parent_id.as_deref(),
            _ => None,
        };
    }
    chain.reverse();
    chain
}

/// Apply a sequence of message/compaction entries (in order) into the
/// runtime `AgentMessage` buffer, honoring compaction summary semantics.
fn apply_entries(
    entries: &[&opi_agent::session::SessionEntry],
) -> Vec<opi_agent::message::AgentMessage> {
    use opi_agent::message::{AgentMessage, CompactionSummaryMessage};
    use opi_agent::session::SessionEntry;

    let mut messages: Vec<AgentMessage> = Vec::new();
    let mut entry_ids: Vec<Option<String>> = Vec::new();

    for entry in entries {
        match entry {
            SessionEntry::Message(m) => {
                messages.push(AgentMessage::Llm(m.message.clone()));
                entry_ids.push(Some(m.id.clone()));
            }
            SessionEntry::Compaction(c) => {
                let kept_start = entry_ids
                    .iter()
                    .position(|id| id.as_deref() == Some(c.first_kept_entry_id.as_str()));

                let (kept_messages, kept_ids): (Vec<_>, Vec<_>) = match kept_start {
                    Some(idx) => (messages.split_off(idx), entry_ids.split_off(idx)),
                    None => (Vec::new(), Vec::new()),
                };

                messages.clear();
                entry_ids.clear();
                messages.push(AgentMessage::CompactionSummary(CompactionSummaryMessage {
                    summary: c.summary.clone(),
                    first_kept_entry_id: c.first_kept_entry_id.clone(),
                    tokens_before: c.tokens_before,
                    tokens_after: c.tokens_after,
                }));
                entry_ids.push(None);
                messages.extend(kept_messages);
                entry_ids.extend(kept_ids);
            }
            SessionEntry::Leaf(_) => {}
            _ => {}
        }
    }

    messages
}
