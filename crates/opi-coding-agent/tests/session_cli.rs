//! Session CLI integration tests (task 2.7).
//!
//! DoD: "--resume, --list-sessions, --delete-session CLI flags with
//! stdout/stderr/exit-code E2E tests"
//!
//! Tests cover session_dir(), list/resume/delete operations, and CLI flag
//! dispatch through the session_cli module.

use std::io::Write;
use std::path::PathBuf;

use opi_agent::session::{MessageEntry, SessionEntry, SessionHeader, SessionWriter};
use opi_ai::message::{InputContent, Message, UserMessage};
use opi_coding_agent::session_cli::{
    self, SessionInfo, delete_session, list_sessions, resume_session,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_header(id: &str, cwd: &str) -> SessionHeader {
    SessionHeader::new(id.into(), "2026-05-22T12:00:00Z".into(), cwd.into(), None)
}

fn make_header_with_parent(id: &str, cwd: &str, parent: &str) -> SessionHeader {
    SessionHeader::new(
        id.into(),
        "2026-05-22T12:00:00Z".into(),
        cwd.into(),
        Some(parent.into()),
    )
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

fn create_session_file(dir: &std::path::Path, header: &SessionHeader) -> PathBuf {
    let path = dir.join(format!("{}.jsonl", header.id));
    let mut writer = SessionWriter::create(&path, header.clone()).unwrap();
    writer.append(&test_message_entry("e1", "Hello")).unwrap();
    path
}

fn create_session_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

// ---------------------------------------------------------------------------
// session_dir tests
// ---------------------------------------------------------------------------

#[test]
fn session_dir_returns_path_with_sessions_component() {
    let dir = session_cli::session_dir();
    assert!(
        dir.to_string_lossy().contains("sessions"),
        "session_dir should contain 'sessions': got {:?}",
        dir
    );
}

#[test]
fn session_dir_is_consistent_across_calls() {
    let a = session_cli::session_dir();
    let b = session_cli::session_dir();
    assert_eq!(a, b, "session_dir should return the same path each time");
}

// ---------------------------------------------------------------------------
// list_sessions tests
// ---------------------------------------------------------------------------

#[test]
fn list_sessions_empty_dir_returns_empty() {
    let dir = create_session_dir();
    let sessions = list_sessions(dir.path()).unwrap();
    assert!(
        sessions.is_empty(),
        "empty directory should return no sessions"
    );
}

#[test]
fn list_sessions_nonexistent_dir_returns_empty() {
    let dir = create_session_dir();
    let nonexistent = dir.path().join("no_such_dir");
    let sessions = list_sessions(&nonexistent).unwrap();
    assert!(
        sessions.is_empty(),
        "nonexistent directory should return no sessions"
    );
}

#[test]
fn list_sessions_finds_single_session() {
    let dir = create_session_dir();
    let header = make_header("sess-001", "/repo");
    create_session_file(dir.path(), &header);

    let sessions = list_sessions(dir.path()).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "sess-001");
    assert_eq!(sessions[0].cwd, "/repo");
}

#[test]
fn list_sessions_finds_multiple_sessions() {
    let dir = create_session_dir();
    create_session_file(dir.path(), &make_header("sess-001", "/repo1"));
    create_session_file(dir.path(), &make_header("sess-002", "/repo2"));
    create_session_file(dir.path(), &make_header("sess-003", "/repo3"));

    let sessions = list_sessions(dir.path()).unwrap();
    assert_eq!(sessions.len(), 3);

    let ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"sess-001"));
    assert!(ids.contains(&"sess-002"));
    assert!(ids.contains(&"sess-003"));
}

#[test]
fn list_sessions_skips_non_jsonl_files() {
    let dir = create_session_dir();
    create_session_file(dir.path(), &make_header("sess-001", "/repo"));

    // Create a non-JSONL file
    let other = dir.path().join("notes.txt");
    std::fs::write(&other, "not a session").unwrap();

    let sessions = list_sessions(dir.path()).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "sess-001");
}

#[test]
fn list_sessions_skips_corrupt_jsonl_files() {
    let dir = create_session_dir();
    create_session_file(dir.path(), &make_header("sess-001", "/repo"));

    // Create a corrupt JSONL file
    let corrupt = dir.path().join("corrupt.jsonl");
    let mut f = std::fs::File::create(&corrupt).unwrap();
    writeln!(f, "NOT VALID JSON").unwrap();

    let sessions = list_sessions(dir.path()).unwrap();
    assert_eq!(sessions.len(), 1, "corrupt file should be skipped");
    assert_eq!(sessions[0].id, "sess-001");
}

#[test]
fn list_sessions_extracts_timestamp() {
    let dir = create_session_dir();
    let header = make_header("sess-001", "/repo");
    create_session_file(dir.path(), &header);

    let sessions = list_sessions(dir.path()).unwrap();
    assert_eq!(sessions[0].timestamp, "2026-05-22T12:00:00Z");
}

#[test]
fn list_sessions_extracts_parent_session() {
    let dir = create_session_dir();
    let header = make_header_with_parent("sess-002", "/repo", "sess-001");
    create_session_file(dir.path(), &header);

    let sessions = list_sessions(dir.path()).unwrap();
    assert_eq!(sessions[0].parent_session.as_deref(), Some("sess-001"));
}

// ---------------------------------------------------------------------------
// resume_session tests
// ---------------------------------------------------------------------------

#[test]
fn resume_session_reads_existing_session() {
    let dir = create_session_dir();
    let header = make_header("sess-001", "/repo");
    create_session_file(dir.path(), &header);

    let result = resume_session(dir.path(), "sess-001").unwrap();
    assert_eq!(result.header.id, "sess-001");
    assert_eq!(result.header.cwd, "/repo");
    assert_eq!(result.entries.len(), 1, "should have one entry");
}

#[test]
fn resume_session_returns_entries() {
    let dir = create_session_dir();
    let header = make_header("sess-001", "/repo");
    let path = dir.path().join("sess-001.jsonl");
    let mut writer = SessionWriter::create(&path, header.clone()).unwrap();
    writer.append(&test_message_entry("e1", "Hello")).unwrap();
    writer.append(&test_message_entry("e2", "World")).unwrap();

    let result = resume_session(dir.path(), "sess-001").unwrap();
    assert_eq!(result.entries.len(), 2);
}

#[test]
fn resume_session_missing_returns_error() {
    let dir = create_session_dir();
    let result = resume_session(dir.path(), "nonexistent");
    assert!(
        result.is_err(),
        "resuming a nonexistent session should fail"
    );
}

// ---------------------------------------------------------------------------
// delete_session tests
// ---------------------------------------------------------------------------

#[test]
fn delete_session_removes_file() {
    let dir = create_session_dir();
    let header = make_header("sess-001", "/repo");
    create_session_file(dir.path(), &header);

    let path = dir.path().join("sess-001.jsonl");
    assert!(path.exists(), "session file should exist before delete");

    delete_session(dir.path(), "sess-001").unwrap();
    assert!(
        !path.exists(),
        "session file should be removed after delete"
    );
}

#[test]
fn delete_session_missing_returns_error() {
    let dir = create_session_dir();
    let result = delete_session(dir.path(), "nonexistent");
    assert!(
        result.is_err(),
        "deleting a nonexistent session should fail"
    );
}

#[test]
fn delete_session_does_not_affect_other_sessions() {
    let dir = create_session_dir();
    create_session_file(dir.path(), &make_header("sess-001", "/repo1"));
    create_session_file(dir.path(), &make_header("sess-002", "/repo2"));

    delete_session(dir.path(), "sess-001").unwrap();

    let sessions = list_sessions(dir.path()).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "sess-002");
}

// ---------------------------------------------------------------------------
// format_sessions_for_display tests
// ---------------------------------------------------------------------------

#[test]
fn format_sessions_empty_list() {
    let output = session_cli::format_sessions(&[]);
    assert!(
        output.is_empty(),
        "empty session list should produce empty output"
    );
}

#[test]
fn format_sessions_single_entry() {
    let info = SessionInfo {
        id: "sess-001".into(),
        timestamp: "2026-05-22T12:00:00Z".into(),
        cwd: "/repo".into(),
        parent_session: None,
    };
    let output = session_cli::format_sessions(&[info]);
    assert!(
        output.contains("sess-001"),
        "output should contain session id"
    );
    assert!(output.contains("/repo"), "output should contain cwd");
}

#[test]
fn format_sessions_shows_parent_when_present() {
    let info = SessionInfo {
        id: "sess-002".into(),
        timestamp: "2026-05-22T12:00:00Z".into(),
        cwd: "/repo".into(),
        parent_session: Some("sess-001".into()),
    };
    let output = session_cli::format_sessions(&[info]);
    assert!(
        output.contains("sess-001"),
        "output should show parent session id"
    );
}

// ---------------------------------------------------------------------------
// CLI flag parsing tests
// ---------------------------------------------------------------------------

#[test]
fn cli_parse_list_sessions() {
    use clap::Parser;
    use opi_coding_agent::cli::Cli;

    let cli = Cli::try_parse_from(["opi", "--list-sessions"]);
    assert!(cli.is_ok(), "--list-sessions should parse");
    assert!(cli.unwrap().list_sessions);
}

#[test]
fn cli_parse_resume() {
    use clap::Parser;
    use opi_coding_agent::cli::Cli;

    let cli = Cli::try_parse_from(["opi", "--resume", "sess-001"]);
    assert!(cli.is_ok(), "--resume should parse");
    assert_eq!(cli.unwrap().resume.as_deref(), Some("sess-001"));
}

#[test]
fn cli_parse_delete_session() {
    use clap::Parser;
    use opi_coding_agent::cli::Cli;

    let cli = Cli::try_parse_from(["opi", "--delete-session", "sess-001"]);
    assert!(cli.is_ok(), "--delete-session should parse");
    assert_eq!(cli.unwrap().delete_session.as_deref(), Some("sess-001"));
}

#[test]
fn cli_session_flags_are_independent() {
    use clap::Parser;
    use opi_coding_agent::cli::Cli;

    // Only --list-sessions
    let cli = Cli::try_parse_from(["opi", "--list-sessions"]).unwrap();
    assert!(cli.list_sessions);
    assert!(cli.resume.is_none());
    assert!(cli.delete_session.is_none());
}

// ---------------------------------------------------------------------------
// Path traversal validation tests
// ---------------------------------------------------------------------------

#[test]
fn resume_session_rejects_path_traversal() {
    let dir = create_session_dir();
    assert!(resume_session(dir.path(), "../etc/passwd").is_err());
    assert!(resume_session(dir.path(), "..\\windows\\system32").is_err());
    assert!(resume_session(dir.path(), "../../secret").is_err());
    assert!(resume_session(dir.path(), "").is_err());
}

#[test]
fn delete_session_rejects_path_traversal() {
    let dir = create_session_dir();
    assert!(delete_session(dir.path(), "../etc/passwd").is_err());
    assert!(delete_session(dir.path(), "..\\windows\\system32").is_err());
    assert!(delete_session(dir.path(), "../../secret").is_err());
    assert!(delete_session(dir.path(), "").is_err());
}

#[test]
fn valid_session_ids_accepted() {
    let dir = create_session_dir();
    // These should NOT fail validation (they fail on "not found" instead)
    let r1 = resume_session(dir.path(), "sess-001");
    assert!(matches!(r1, Err(session_cli::SessionCliError::NotFound(_))));

    let r2 = resume_session(dir.path(), "abc123");
    assert!(matches!(r2, Err(session_cli::SessionCliError::NotFound(_))));
}

#[test]
fn resume_session_reports_skipped_corrupt_entries() {
    let dir = create_session_dir();
    let header = make_header("corrupt-sess", "/repo");
    let path = dir.path().join("corrupt-sess.jsonl");
    let mut writer = SessionWriter::create(&path, header.clone()).unwrap();

    // Write a valid entry
    writer.append(&test_message_entry("e1", "good")).unwrap();
    drop(writer);

    // Inject a corrupt line directly into the JSONL file.
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"{not valid json}\n").unwrap();
    }

    // Write another valid entry
    let mut writer = SessionWriter::open(&path).unwrap();
    writer
        .append(&test_message_entry("e2", "also-good"))
        .unwrap();

    let result = resume_session(dir.path(), "corrupt-sess").unwrap();
    assert_eq!(result.entries.len(), 2, "should have 2 valid entries");
    assert_eq!(
        result.skipped_entries, 1,
        "should report 1 corrupt entry skipped"
    );
}

// ---------------------------------------------------------------------------
// Subprocess E2E tests
// ---------------------------------------------------------------------------

fn opi_binary() -> std::path::PathBuf {
    // Tests run in the crate directory; the binary is at workspace_root/target/debug/opi
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let workspace_root = std::path::PathBuf::from(&crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("crate should be in crates/opi-coding-agent")
        .to_path_buf();
    let bin_name = if cfg!(windows) { "opi.exe" } else { "opi" };
    let path = workspace_root.join("target").join("debug").join(bin_name);
    assert!(
        path.exists(),
        "opi binary must be built: run `cargo build -p opi-coding-agent`"
    );
    path
}

fn build_opi_if_needed() {
    let bin = opi_binary();
    if !bin.exists() {
        let status = std::process::Command::new("cargo")
            .args(["build", "-p", "opi-coding-agent"])
            .status()
            .expect("failed to run cargo build");
        assert!(status.success(), "cargo build failed");
    }
}

#[test]
fn e2e_list_sessions_empty_exits_zero() {
    build_opi_if_needed();

    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(opi_binary())
        .env("OPI_SESSIONS_DIR", dir.path())
        .arg("--list-sessions")
        .output()
        .expect("failed to run opi");

    // --list-sessions with no sessions should succeed with empty stdout
    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "stdout should be empty, got: {stdout}");
}

#[test]
fn e2e_delete_nonexistent_exits_nonzero() {
    build_opi_if_needed();

    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(opi_binary())
        .env("OPI_SESSIONS_DIR", dir.path())
        .arg("--delete-session")
        .arg("nonexistent-session")
        .output()
        .expect("failed to run opi");

    assert!(!output.status.success(), "exit code should be non-zero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "stderr should mention 'not found', got: {stderr}"
    );
}

#[test]
fn e2e_resume_nonexistent_exits_nonzero() {
    build_opi_if_needed();

    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(opi_binary())
        .env("OPI_SESSIONS_DIR", dir.path())
        .arg("--resume")
        .arg("nonexistent-session")
        .output()
        .expect("failed to run opi");

    assert!(!output.status.success(), "exit code should be non-zero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "stderr should mention 'not found', got: {stderr}"
    );
}
