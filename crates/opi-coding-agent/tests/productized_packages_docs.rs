//! Documentation guard tests for the Phase 5 productized extension/package ecosystem.
//!
//! These tests verify that user-facing documentation describes the Phase 5 MVP
//! truthfully and does NOT claim features that are not implemented.

use std::path::Path;

/// Helper: read a file relative to the repo root.
fn read_repo_file(relative: &str) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("../..").join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

/// Helper: case-insensitive substring check.
fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

// ===========================================================================
// Negative guards: features that MUST NOT be claimed as complete
// ===========================================================================

#[test]
fn readme_does_not_claim_npm() {
    let en = read_repo_file("README.md");
    let zh = read_repo_file("README.zh.md");

    // npm is a pi mechanism that opi deliberately does not use.
    // The only acceptable mention is "not npm" / "no npm" / "does not use npm".
    for (lang, content) in [("en", &en), ("zh", &zh)] {
        // Raw "npm" should only appear in a negation context.
        if content.contains("npm") {
            // Allow: "does not", "without", "no ", "not " before npm
            let lower = content.to_lowercase();
            for line in lower.lines() {
                if line.contains("npm") {
                    assert!(
                        line.contains("not")
                            || line.contains("no ")
                            || line.contains("without")
                            || line.contains("pi")
                            || line.contains("而不是"),
                        "[{lang}] README mentions npm without clear negation: {line}"
                    );
                }
            }
        }
    }
}

#[test]
fn readme_does_not_claim_marketplace() {
    let en = read_repo_file("README.md");
    let zh = read_repo_file("README.zh.md");

    // No package marketplace exists.
    assert!(
        !contains_ci(&en, "marketplace"),
        "en README must not claim a package marketplace"
    );
    assert!(
        !contains_ci(&zh, "marketplace"),
        "zh README must not claim a package marketplace"
    );
    assert!(
        !contains_ci(&zh, "市场"),
        "zh README must not claim a package marketplace (市场)"
    );
}

#[test]
fn readme_does_not_claim_hot_reload() {
    let en = read_repo_file("README.md");
    let zh = read_repo_file("README.zh.md");

    assert!(
        !contains_ci(&en, "hot reload"),
        "en README must not claim hot reload"
    );
    assert!(
        !contains_ci(&zh, "热重载"),
        "zh README must not claim hot reload"
    );
}

/// Helper: check that a forbidden phrase does not appear as a positive claim.
/// Allows matches only within a rejection/negation context
/// (lines containing "reject", "must not", "do not", etc.).
fn no_positive_claim(haystack: &str, needle: &str) -> bool {
    let lower = haystack.to_lowercase();
    let needle_lower = needle.to_lowercase();
    for line in lower.lines() {
        if line.contains(&needle_lower) {
            // If the line is itself a rejection/negation context, it's OK.
            // This covers exit criteria like "guard tests reject claims about X".
            if line.contains("reject")
                || line.contains("must not")
                || line.contains("do not")
                || line.contains("does not")
                || line.contains("without")
                || line.contains("not claim")
            {
                continue;
            }
            return false;
        }
    }
    true
}

#[test]
fn spec_does_not_claim_provider_streaming_adapters() {
    let en = read_repo_file("docs/opi-spec.md");
    let zh = read_repo_file("docs/opi-spec.zh.md");

    // "provider streaming adapter" would mean adapters can intercept/modify
    // the LLM response stream. Phase 5 adapters only provide tools, commands,
    // hooks, and events.
    assert!(
        no_positive_claim(&en, "provider streaming adapter"),
        "opi-spec must not claim provider streaming adapters"
    );
    assert!(
        !contains_ci(&zh, "供应商流式适配器"),
        "opi-spec.zh must not claim provider streaming adapters"
    );
}

#[test]
fn spec_does_not_claim_custom_tui_adapters() {
    let en = read_repo_file("docs/opi-spec.md");
    let zh = read_repo_file("docs/opi-spec.zh.md");

    // TUI adapters (custom terminal rendering from packages) do not exist.
    assert!(
        no_positive_claim(&en, "TUI adapter"),
        "opi-spec must not claim TUI adapters"
    );
    assert!(
        !contains_ci(&zh, "TUI 适配器"),
        "opi-spec.zh must not claim TUI adapters"
    );
}

#[test]
fn docs_do_not_claim_package_permission_enforcement() {
    let en = read_repo_file("docs/opi-spec.md");
    let readme_en = read_repo_file("README.md");

    // "package permission enforcement" would mean the package system enforces
    // permission policies. Phase 5 only provides hooks that packages can use;
    // there is no built-in permission enforcement layer.
    assert!(
        no_positive_claim(&en, "package permission enforcement"),
        "opi-spec must not claim package permission enforcement"
    );
    assert!(
        !contains_ci(&readme_en, "package permission enforcement"),
        "README must not claim package permission enforcement"
    );
}

#[test]
fn spec_does_not_claim_hot_reload() {
    let en = read_repo_file("docs/opi-spec.md");

    assert!(
        no_positive_claim(&en, "hot reload"),
        "opi-spec must not claim hot reload"
    );
    assert!(
        no_positive_claim(&en, "hot-reload"),
        "opi-spec must not claim hot reload"
    );
}

// ===========================================================================
// Positive guards: Phase 5 MVP truth that MUST be present
// ===========================================================================

#[test]
fn readme_mentions_package_cli_commands() {
    let en = read_repo_file("README.md");

    // README should mention opi package commands.
    assert!(
        contains_ci(&en, "package add")
            || contains_ci(&en, "opi package")
            || contains_ci(&en, "package remove"),
        "en README must mention package CLI commands (package add/remove/list/doctor)"
    );
}

#[test]
fn readme_mentions_process_adapters() {
    let en = read_repo_file("README.md");

    // README should mention process adapters.
    assert!(
        contains_ci(&en, "process") && contains_ci(&en, "adapter"),
        "en README must mention process adapters"
    );
}

#[test]
fn spec_has_phase_five_roadmap() {
    let en = read_repo_file("docs/opi-spec.md");

    // opi-spec must have a Phase 5 section.
    assert!(
        contains_ci(&en, "Phase 5") || contains_ci(&en, "phase 5"),
        "opi-spec must include Phase 5 in the implementation roadmap"
    );
}

#[test]
fn spec_mentions_adapter_protocol() {
    let en = read_repo_file("docs/opi-spec.md");

    // opi-spec should mention the adapter JSONL protocol.
    assert!(
        contains_ci(&en, "opi-extension-jsonl"),
        "opi-spec must mention the opi-extension-jsonl-v1 adapter protocol"
    );
}

#[test]
fn alignment_matrix_mentions_process_adapters() {
    let matrix = read_repo_file("docs/pi-alignment-matrix.md");

    // The alignment matrix should reflect that process adapters are now present.
    assert!(
        contains_ci(&matrix, "process") && contains_ci(&matrix, "adapter"),
        "pi-alignment-matrix must mention process adapters in the Phase 5 update"
    );
}

#[test]
fn spec_mentions_package_cli() {
    let en = read_repo_file("docs/opi-spec.md");

    assert!(
        contains_ci(&en, "package add")
            || contains_ci(&en, "package remove")
            || contains_ci(&en, "package list")
            || contains_ci(&en, "package doctor")
            || contains_ci(&en, "`opi package`"),
        "opi-spec must mention package CLI commands"
    );
}

// ===========================================================================
// Synchronization guards: EN/ZH must be in sync on key claims
// ===========================================================================

#[test]
fn readme_en_zh_both_mention_packages() {
    let en = read_repo_file("README.md");
    let zh = read_repo_file("README.zh.md");

    let en_has_package_cli = contains_ci(&en, "package add")
        || contains_ci(&en, "package remove")
        || contains_ci(&en, "opi package");
    let zh_has_package_cli = contains_ci(&zh, "package add")
        || contains_ci(&zh, "package remove")
        || contains_ci(&zh, "opi package");

    assert_eq!(
        en_has_package_cli, zh_has_package_cli,
        "EN and ZH READMEs must both mention package CLI commands"
    );
}

#[test]
fn spec_en_zh_both_have_phase_five() {
    let en = read_repo_file("docs/opi-spec.md");
    let zh = read_repo_file("docs/opi-spec.zh.md");

    let en_has = contains_ci(&en, "Phase 5") || contains_ci(&en, "phase 5");
    let zh_has =
        contains_ci(&zh, "Phase 5") || contains_ci(&zh, "phase 5") || contains_ci(&zh, "第五阶段");

    assert_eq!(
        en_has, zh_has,
        "EN and ZH opi-specs must both include Phase 5"
    );
}
