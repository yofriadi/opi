//! Test that --list-models --json output is valid JSON even with special
//! characters in model/display names.

use std::process::Command;

#[test]
fn list_models_json_output_is_valid_json() {
    let output = Command::new("cargo")
        .args(["run", "-p", "opi-coding-agent", "--", "--list-models", "--json"])
        .output()
        .expect("failed to run opi");
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "invalid JSON in --list-models --json output: {line:?}"
        );
    }
}
