use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

#[test]
fn phase4_ledger_spec_hash_matches_current_spec() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let spec_path = repo_root.join("docs/opi-spec.md");
    let ledger_path = repo_root.join("docs/snapshots/phase4/opi-impl-state.json");

    let spec = fs::read_to_string(&spec_path).expect("read docs/opi-spec.md");
    let normalized_spec = spec.replace("\r\n", "\n");
    let actual = format!("{:x}", Sha256::digest(normalized_spec.as_bytes()));

    let ledger_bytes =
        fs::read(&ledger_path).expect("read docs/snapshots/phase4/opi-impl-state.json");
    let ledger: serde_json::Value =
        serde_json::from_slice(&ledger_bytes).expect("parse phase4 ledger");
    let recorded = ledger["spec_files_sha256"]["docs/opi-spec.md"]
        .as_str()
        .expect("phase4 ledger records docs/opi-spec.md hash");

    assert_eq!(
        recorded, actual,
        "phase4 ledger spec hash for docs/opi-spec.md is stale"
    );
}
