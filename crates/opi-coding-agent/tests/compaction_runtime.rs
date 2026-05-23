use opi_agent::compaction::{CompactionConfig, CompactionEngine};
use opi_agent::session_event::CompactionReason;

#[test]
fn compaction_triggers_at_threshold() {
    let config = CompactionConfig {
        enabled: true,
        threshold_tokens: 100,
    };
    let engine = CompactionEngine::new(config);
    assert!(engine.should_compact(150, CompactionReason::Threshold));
    assert!(!engine.should_compact(50, CompactionReason::Threshold));
}

#[test]
fn compaction_disabled() {
    let config = CompactionConfig {
        enabled: false,
        threshold_tokens: 100,
    };
    let engine = CompactionEngine::new(config);
    assert!(!engine.should_compact(150, CompactionReason::Threshold));
}
