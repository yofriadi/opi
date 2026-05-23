//! Session lifecycle coordinator bridging harness, session writer,
//! compaction engine, and usage accumulation.

use std::path::Path;

use opi_agent::compaction::CompactionConfig;
use opi_agent::compaction::CompactionEngine;
use opi_agent::message::AgentMessage;
use opi_agent::session::SessionEntry;
use opi_agent::session::SessionHeader;
use opi_agent::session::SessionWriter;
use opi_ai::stream::CumulativeUsage;
use opi_ai::stream::Usage;

pub struct SessionCoordinator {
    writer: SessionWriter,
    compaction: CompactionEngine,
    usage: CumulativeUsage,
    session_id: String,
}

impl SessionCoordinator {
    pub fn new(
        dir: &Path,
        cwd: &str,
        compaction_config: CompactionConfig,
    ) -> std::io::Result<Self> {
        let id = generate_session_id();
        let timestamp = now_iso();
        let header = SessionHeader::new(id.clone(), timestamp, cwd.into(), None);
        let path = dir.join(format!("{id}.jsonl"));
        std::fs::create_dir_all(dir)?;
        let writer = SessionWriter::create(&path, header)?;
        Ok(Self {
            writer,
            compaction: CompactionEngine::new(compaction_config),
            usage: CumulativeUsage::default(),
            session_id: id,
        })
    }

    pub fn on_turn_end(&mut self, messages: &[AgentMessage], usage: &Usage) {
        self.usage.accumulate(usage);
        for msg in messages {
            if let AgentMessage::Llm(m) = msg {
                let entry = SessionEntry::Message(opi_agent::session::MessageEntry {
                    id: format!("msg-{}", self.usage.turn_count()),
                    parent_id: None,
                    timestamp: now_iso(),
                    message: m.clone(),
                });
                let _ = self.writer.append(&entry);
            }
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn usage(&self) -> &CumulativeUsage {
        &self.usage
    }

    pub fn compaction_engine(&self) -> &CompactionEngine {
        &self.compaction
    }
}

fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{ts:x}")
}

fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let tod = secs % 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;
    let s = tod % 60;
    let (y, mo, d) = days_to_ymd(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let diy = if is_leap(year) { 366 } else { 365 };
        if days < diy {
            break;
        }
        days -= diy;
        year += 1;
    }
    let md = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0u64;
    for &d in &md {
        if days < d {
            break;
        }
        days -= d;
        month += 1;
    }
    (year, month + 1, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}
