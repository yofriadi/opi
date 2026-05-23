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

    let (header, entries) = opi_agent::session::SessionReader::read_all(&path)
        .map_err(|e| SessionCliError::Corrupt(format!("{}: {e}", path.display())))?;

    Ok(ResumedSession { header, entries })
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
                println!(
                    "Resuming session {} ({} entries, cwd: {})",
                    session.header.id,
                    session.entries.len(),
                    session.header.cwd,
                );
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
pub fn reconstruct_context(
    entries: &[opi_agent::session::SessionEntry],
) -> Vec<opi_agent::message::AgentMessage> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            opi_agent::session::SessionEntry::Message(msg_entry) => Some(
                opi_agent::message::AgentMessage::Llm(msg_entry.message.clone()),
            ),
            _ => None,
        })
        .collect()
}
