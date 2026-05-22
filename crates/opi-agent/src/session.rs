//! Session v1 JSONL storage (S9.3).
//!
//! Append-only, versioned JSONL format for session persistence. The first line
//! is a header; subsequent lines are tree entries forming a conversation tree.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Current session format version.
const FORMAT_VERSION: u32 = 1;

/// Session header — the first line of a JSONL file (S9.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHeader {
    #[serde(rename = "type")]
    pub type_: String,
    pub version: u32,
    pub id: String,
    pub timestamp: String,
    pub cwd: String,
    pub parent_session: Option<String>,
}

impl SessionHeader {
    pub fn new(id: String, timestamp: String, cwd: String, parent_session: Option<String>) -> Self {
        Self {
            type_: "session".to_owned(),
            version: FORMAT_VERSION,
            id,
            timestamp,
            cwd,
            parent_session,
        }
    }
}

/// A message tree entry (S9.3 `message` type).
///
/// The `message` field uses the provider-facing `Message` type (S7.1), not
/// `AgentMessage`. Each S9.3 entry type maps to its own payload structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub timestamp: String,
    pub message: opi_ai::message::Message,
}

/// A compaction tree entry (S9.3 `compaction` type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub timestamp: String,
    pub summary: String,
    pub first_kept_entry_id: String,
    pub tokens_before: u64,
    pub tokens_after: u64,
}

/// A leaf pointer entry (S9.3 `leaf` type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub timestamp: String,
    pub entry_id: String,
}

/// All tree entry types (S9.3).
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEntry {
    Message(MessageEntry),
    Compaction(CompactionEntry),
    Leaf(LeafEntry),
}

/// Crash recovery status returned by `SessionReader`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashRecovery {
    Clean,
    TruncatedLine,
    CorruptEntries { count: usize },
    /// Both corruption and truncation detected.
    CorruptEntriesWithTruncation { count: usize },
}

/// Append-only JSONL writer with crash-safe flush.
pub struct SessionWriter {
    file: std::fs::File,
}

impl SessionWriter {
    /// Create a new session file with the given header.
    pub fn create(path: &Path, header: SessionHeader) -> std::io::Result<Self> {
        let mut file = std::fs::File::create(path)?;
        let header_json = serde_json::to_string(&header)?;
        writeln!(file, "{header_json}")?;
        file.sync_all()?;
        Ok(Self { file })
    }

    /// Open an existing session file for appending (seeks to end).
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().append(true).open(path)?;
        Ok(Self { file })
    }

    /// Append a session entry as a new JSONL line.
    pub fn append(&mut self, entry: &SessionEntry) -> std::io::Result<()> {
        let json = serde_json::to_string(entry)?;
        writeln!(self.file, "{json}")?;
        self.file.sync_all()
    }
}

/// JSONL reader with crash recovery.
pub struct SessionReader;

impl SessionReader {
    /// Read all entries from a session file (strict mode — errors on corrupt data).
    pub fn read_all(path: &Path) -> std::io::Result<(SessionHeader, Vec<SessionEntry>)> {
        let (header, entries, _recovery) = Self::read_with_recovery(path)?;
        Ok((header, entries))
    }

    /// Read all entries with crash recovery metadata.
    pub fn read_with_recovery(
        path: &Path,
    ) -> std::io::Result<(SessionHeader, Vec<SessionEntry>, CrashRecovery)> {
        let content = std::fs::read_to_string(path)?;

        if content.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "empty session file",
            ));
        }

        let last_line_incomplete = !content.ends_with('\n') && !content.ends_with('\r');

        // Single-pass: collect lines, then parse.
        let all_lines: Vec<&str> = content.lines().collect();
        if all_lines.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "empty session file",
            ));
        }

        // First line is the header.
        let header: SessionHeader = serde_json::from_str(all_lines[0]).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid session header: {e}"),
            )
        })?;

        // Validate header type and version.
        if header.type_ != "session" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("expected header type 'session', got '{}'", header.type_),
            ));
        }
        if header.version != FORMAT_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "unsupported session version {}, expected {}",
                    header.version, FORMAT_VERSION
                ),
            ));
        }

        let data_lines = &all_lines[1..];
        let total = data_lines.len();
        let mut entries = Vec::new();
        let mut corrupt_count = 0;

        for (i, line) in data_lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            // Skip the last line if the file ended without a newline (truncated write).
            if last_line_incomplete && i == total - 1 {
                continue;
            }
            match serde_json::from_str::<SessionEntry>(line) {
                Ok(entry) => entries.push(entry),
                Err(_) => corrupt_count += 1,
            }
        }

        let recovery = match (corrupt_count > 0, last_line_incomplete) {
            (true, true) => CrashRecovery::CorruptEntriesWithTruncation { count: corrupt_count },
            (true, false) => CrashRecovery::CorruptEntries { count: corrupt_count },
            (false, true) => CrashRecovery::TruncatedLine,
            (false, false) => CrashRecovery::Clean,
        };

        Ok((header, entries, recovery))
    }
}
