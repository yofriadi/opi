//! Documentation guard tests for the productized extension/package ecosystem
//! and the Phase 6 documentation-truth and version-synchronization workstreams.
//!
//! The Phase 5 tests verify that user-facing documentation describes the Phase 5
//! MVP truthfully and does NOT claim features that are not implemented. The
//! Phase 6 tests verify that current-state documentation identifies the
//! workspace/crate state at the current released version (matching the
//! workspace version) while historical release rows stay historical, and that
//! English and Chinese counterparts carry the same current-version and
//! opi-web-ui scope claims.

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

fn readme_npm_line_has_clear_negation(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("not npm")
        || lower.contains("no npm")
        || lower.contains("does not use npm")
        || lower.contains("without npm")
        || lower.contains("without node, npm")
        || lower.contains("无需 node、npm")
        || lower.contains("不是 npm")
        || lower.contains("而不是 npm")
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
                        readme_npm_line_has_clear_negation(line),
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
fn docs_do_not_claim_package_marketplace_or_gallery() {
    let files = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
        "docs/pi-alignment-matrix.md",
        "docs/pi-alignment-matrix.zh.md",
    ];
    for needle in [
        "package marketplace",
        "package gallery",
        "marketplace/gallery",
        "marketplace/registry",
        "registry-backed package sources",
        "package 市场",
        "package 画廊",
    ] {
        assert_docs_reject_claim(&files, needle, "a package marketplace or gallery");
    }
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
                || line.contains("not claim")
                || line.contains("不声明")
                || line.contains("不得")
                || line.contains("未实现")
            {
                continue;
            }
            return false;
        }
    }
    true
}

#[test]
fn positive_non_goal_claims_are_rejected_by_helpers() {
    assert!(
        !readme_npm_line_has_clear_negation("opi package add supports npm sources"),
        "an opi/npm positive claim must not be treated as negated just because it contains 'opi'"
    );
    assert!(
        !no_positive_claim(
            "opi now bundles Node without external dependencies",
            "bundles Node"
        ),
        "a positive bundled-runtime claim must not be treated as negated just because it says without"
    );
    assert!(
        !no_positive_claim(
            "TypeScript extension API compatibility is complete",
            "TypeScript extension API"
        ),
        "positive TypeScript extension API compatibility claims must be rejected"
    );
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

#[test]
fn spec_documents_shutdown_contract_without_harness_overclaim() {
    let en = read_repo_file("docs/opi-spec.md");
    let zh = read_repo_file("docs/opi-spec.zh.md");

    assert!(
        !en.contains("On shutdown, the harness sends a `shutdown` message"),
        "opi-spec must not claim ordinary harness teardown sends adapter shutdown"
    );
    assert!(
        !zh.lines().any(|line| line.contains("harness")
            && line.contains("`shutdown`")
            && line.contains("发送")),
        "opi-spec.zh must not claim ordinary harness teardown sends adapter shutdown"
    );
    assert!(
        en.contains("Explicit `AdapterHost::shutdown`")
            && contains_ci(&en, "ordinary registry teardown")
            && en.contains("best-effort kill-only"),
        "opi-spec must document explicit shutdown separately from ordinary registry teardown"
    );
    assert!(
        zh.contains("`AdapterHost::shutdown`")
            && zh.contains("registry")
            && zh.contains("best-effort kill-only"),
        "opi-spec.zh must document explicit shutdown separately from ordinary registry teardown"
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

#[test]
fn docs_warn_packages_are_trusted_code() {
    let readme = read_repo_file("README.md");
    let readme_zh = read_repo_file("README.zh.md");
    let spec = read_repo_file("docs/opi-spec.md");
    let spec_zh = read_repo_file("docs/opi-spec.zh.md");

    assert!(
        readme.contains("Packages are trusted code"),
        "README must warn that packages are trusted code"
    );
    assert!(
        readme.contains("not sandboxed"),
        "README must warn that package code is not sandboxed"
    );
    assert!(
        readme_zh.contains("Package 是受信任代码"),
        "README.zh must warn that packages are trusted code"
    );
    assert!(
        readme_zh.contains("不会被 sandbox"),
        "README.zh must warn that package code is not sandboxed"
    );
    assert!(
        spec.contains("Packages are trusted code"),
        "opi-spec must warn that packages are trusted code"
    );
    assert!(
        spec.contains("not sandboxed"),
        "opi-spec must warn that package code is not sandboxed"
    );
    assert!(
        spec_zh.contains("Package 是受信任代码"),
        "opi-spec.zh must warn that packages are trusted code"
    );
    assert!(
        spec_zh.contains("不会被 sandbox"),
        "opi-spec.zh must warn that package code is not sandboxed"
    );
}

#[test]
fn changelog_mentions_phase_five_package_loop() {
    let changelog = read_repo_file("CHANGELOG.md");

    assert!(
        changelog.contains("opi package add/remove/list/doctor"),
        "CHANGELOG must mention package CLI lifecycle coverage"
    );
    assert!(
        changelog.contains("opi-extension-jsonl-v1"),
        "CHANGELOG must mention the adapter JSONL protocol"
    );
    assert!(
        changelog.contains("Adapter state snapshots"),
        "CHANGELOG must mention adapter state persistence"
    );
}

#[test]
fn docs_guard_package_lifecycle_claims() {
    let readme = read_repo_file("README.md");
    let readme_zh = read_repo_file("README.zh.md");
    let spec = read_repo_file("docs/opi-spec.md");
    let spec_zh = read_repo_file("docs/opi-spec.zh.md");

    for (name, content) in [("README", readme.as_str()), ("opi-spec", spec.as_str())] {
        assert!(
            content.contains("validates the manifest")
                || content.contains("validates the package manifest"),
            "{name} must say package add validates manifests"
        );
        assert!(
            content.contains("writes a lock entry"),
            "{name} must say package add writes lock entries"
        );
        assert!(
            content.contains("reads installed declarations and lock state"),
            "{name} must say runtime startup reads installed declarations and lock state"
        );
    }

    for (name, content) in [
        ("README.zh", readme_zh.as_str()),
        ("opi-spec.zh", spec_zh.as_str()),
    ] {
        assert!(
            content.contains("验证 manifest") || content.contains("验证 package manifest"),
            "{name} must say package add validates manifests"
        );
        assert!(
            content.contains("写入 lock 条目"),
            "{name} must say package add writes lock entries"
        );
        assert!(
            content.contains("读取已安装声明和 lock 状态"),
            "{name} must say runtime startup reads installed declarations and lock state"
        );
    }

    for phrase in [
        "source availability",
        "lock consistency",
        "manifest V2",
        "resource containment",
        "opi version constraints",
        "adapter command resolution",
    ] {
        assert!(
            spec.contains(phrase),
            "opi-spec must say doctor validates {phrase}"
        );
    }
    for phrase in [
        "来源可用性",
        "lock 一致性",
        "manifest V2",
        "资源路径包含关系",
        "opi 版本约束",
        "adapter 命令解析",
    ] {
        assert!(
            spec_zh.contains(phrase),
            "opi-spec.zh must say doctor validates {phrase}"
        );
    }
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

// ===========================================================================
// Phase 6 guards: documentation truth and version synchronization
//
// Phase 6 Success Criteria 1 and 2 require current-state documentation to
// identify the workspace/crate state at the current released version (matching
// the workspace version) while historical release rows stay historical, and
// require English and Chinese counterparts to carry the same current-version
// and opi-web-ui scope claims. Lockstep versioning makes the compiled crate
// version the single source of truth, so both tests read it from
// `env!("CARGO_PKG_VERSION")` rather than hardcoding a number.
// ===========================================================================

#[test]
fn phase6_current_docs_match_workspace_version() {
    // Lockstep versioning: every crate shares the workspace version, so the
    // compiled crate version is the authoritative current version the docs that
    // describe the *current* implementation must match. Historical release
    // records (CHANGELOG sections, roadmap rows) are exempt and stay historical.
    let version = env!("CARGO_PKG_VERSION");

    // Root README names the current workspace version.
    let readme = read_repo_file("README.md");
    assert!(
        readme.contains(&format!("Current workspace version: `{version}`")),
        "README must name the current workspace version `{version}`"
    );
    assert!(
        read_repo_file("AGENTS.md").contains(&format!("Current workspace version: `{version}`")),
        "AGENTS.md is live agent context and must name the current workspace version `{version}`"
    );
    assert!(
        read_repo_file("CLAUDE.md").contains(&format!("v{version} ships")),
        "CLAUDE.md is live agent context and must summarize the current workspace as v{version}"
    );

    // Each publishable crate README names its current crate version.
    for crate_name in [
        "opi-ai",
        "opi-agent",
        "opi-tui",
        "opi-coding-agent",
        "opi-web-ui",
    ] {
        let crate_readme = read_repo_file(&format!("crates/{crate_name}/README.md"));
        assert!(
            crate_readme.contains(&format!("Current crate version: `{version}`")),
            "{crate_name} README must name the current crate version `{version}`"
        );
    }

    // opi-spec describes the current workspace, not a historical release, in its
    // Document Control "Current implementation" row, its Current Baseline
    // versioning row, and its Phase 4/5 status lines.
    let spec = read_repo_file("docs/opi-spec.md");
    let current_impl = spec
        .lines()
        .find(|line| line.contains("Current implementation"))
        .expect("opi-spec must have a Current implementation row");
    assert!(
        current_impl.contains(&format!("{version} workspace")),
        "opi-spec Current implementation row must describe the {version} workspace, got: {current_impl}"
    );
    assert!(
        spec.contains(&format!("| Versioning | lockstep `{version}` |")),
        "opi-spec Current Baseline versioning must be lockstep `{version}`"
    );
    assert!(
        spec.contains(&format!("current `{version}` workspace")),
        "opi-spec Phase 4/5 status lines must reference the current `{version}` workspace"
    );

    // The alignment matrix P0 row advances the current version.
    assert!(
        read_repo_file("docs/pi-alignment-matrix.md")
            .contains(&format!("Current docs describe the `{version}` workspace")),
        "pi-alignment-matrix P0 row must describe the current `{version}` workspace"
    );

    // Historical 0.5.0 release row is preserved, not rewritten to the current version.
    assert!(
        read_repo_file("CHANGELOG.md").contains("## [0.5.0]"),
        "CHANGELOG must preserve the historical 0.5.0 release section"
    );
}

#[test]
fn phase6_localized_docs_stay_in_sync() {
    // Every Phase 6 current-version / opi-web-ui-scope claim made in English
    // documentation must be carried by its Chinese counterpart in the same form.
    // Assertions are per-language and positive, so a stale-but-matched pair
    // (both EN and ZH wrong) cannot satisfy the sync requirement.
    let version = env!("CARGO_PKG_VERSION");

    // Root README.
    assert!(
        read_repo_file("README.zh.md").contains(&format!("当前 workspace 版本：`{version}`")),
        "README.zh must name the current workspace version `{version}`"
    );

    // Publishable crate READMEs.
    for crate_name in [
        "opi-ai",
        "opi-agent",
        "opi-tui",
        "opi-coding-agent",
        "opi-web-ui",
    ] {
        let crate_readme_zh = read_repo_file(&format!("crates/{crate_name}/README.zh.md"));
        assert!(
            crate_readme_zh.contains(&format!("当前 crate 版本：`{version}`")),
            "{crate_name} README.zh must name the current crate version `{version}`"
        );
    }

    // opi-spec Document Control "Current implementation" row.
    let spec_zh = read_repo_file("docs/opi-spec.zh.md");
    assert!(
        spec_zh
            .lines()
            .any(|line| line.contains("当前实现") && line.contains(&format!("{version} workspace"))),
        "opi-spec.zh Current implementation row must reference the {version} workspace"
    );

    // Alignment matrix P0 row.
    assert!(
        read_repo_file("docs/pi-alignment-matrix.zh.md")
            .contains(&format!("当前文档描述 `{version}` workspace")),
        "pi-alignment-matrix.zh P0 row must describe the current `{version}` workspace"
    );
    let matrix_zh = read_repo_file("docs/pi-alignment-matrix.zh.md");
    assert!(
        matrix_zh.contains("| 5 | Package store")
            && matrix_zh.contains("process-JSONL adapter hosting")
            && matrix_zh.contains("opi-extension-jsonl-v1"),
        "pi-alignment-matrix.zh must include the Phase 5 package/adapter capability row"
    );
    assert!(
        matrix_zh.contains("通过 `opi-extension-jsonl-v1` 运行的 process-JSONL adapter 会把 package command、tool、hook、event、state 和 cancellation 桥接进 runtime。"),
        "pi-alignment-matrix.zh P1 extension/package execution row must match the current process-JSONL bridge claim"
    );

    // opi-web-ui scope: English and Chinese both describe it only as an
    // unpublished reusable Rust component/state/rendering crate, never a
    // standalone browser app or pi-web-ui parity surface.
    assert!(
        read_repo_file("crates/opi-web-ui/README.md").contains("not a standalone browser app"),
        "opi-web-ui README must deny it is a standalone browser app"
    );
    assert!(
        read_repo_file("crates/opi-web-ui/README.zh.md").contains("不是独立浏览器应用"),
        "opi-web-ui README.zh must deny it is a standalone browser app"
    );
}

// ===========================================================================
// Phase 6 task 6.6: alignment guards and final Phase 6 gates.
//
// These guards implement the Phase 6 design Workstream 6 (Alignment Guards),
// Success Criteria 7 (guards prevent overclaiming deferred pi ecosystem
// features), Success Criteria 9 (no npm/marketplace/OAuth-parity/pi-web-ui
// parity/permission-enforcement/TS-extension-compat/new-shared-type-crate is
// added in Phase 6), and the DoD's expanded coverage of every Phase 6 non-goal.
//
// Negative doc guards reject current-phase claims for each non-goal. Because
// the docs are already clean, these guards also serve as regression guards;
// each is written so a hypothetical positive claim (e.g. "opi supports OAuth
// parity") would fail while legitimate negations ("OAuth remains a separate
// product decision") pass. Negative code guards assert that no non-goal
// implementation exists in the workspace. A positive guard keeps the Phase 5
// MVP adapter capability surface documented.
// ===========================================================================

/// Helper: the repository root (two levels up from the test crate).
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Helper: assert that no file in `files` positively claims the forbidden
/// `needle`. Uses [`no_positive_claim`] so legitimate negations pass.
fn assert_docs_reject_claim(files: &[&str], needle: &str, what: &str) {
    for path in files {
        let content = read_repo_file(path);
        assert!(
            no_positive_claim(&content, needle),
            "{path} must not positively claim {what}; forbidden phrase {needle:?} appeared outside a negation context"
        );
    }
}

// ---------------------------------------------------------------------------
// Negative doc guards: deferred ecosystem features must not be claimed complete
// ---------------------------------------------------------------------------

#[test]
fn docs_do_not_claim_package_update_enable_disable() {
    // Phase 5 ships add/remove/list/doctor only. update/enable/disable are
    // deferred ecosystem candidates and must not be claimed as commands.
    let files = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
    ];
    for needle in [
        "opi package update",
        "opi package enable",
        "opi package disable",
        "update a package",
        "enable a package",
        "disable a package",
        "package 更新",
        "package 启用",
        "package 禁用",
    ] {
        assert_docs_reject_claim(&files, needle, "a package update/enable/disable command");
    }
}

#[test]
fn docs_do_not_claim_bundled_js_ts_runtime() {
    // The core binary must not bundle a Node.js/TypeScript/jiti runtime.
    let files = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
    ];
    for needle in [
        "bundled Node",
        "bundled TypeScript",
        "bundled JavaScript",
        "bundled Node.js",
        "bundles Node",
        "bundles TypeScript",
        "jiti runtime",
        "Node.js runtime",
        "TypeScript runtime",
        "JavaScript runtime",
        "ships Node",
        "includes Node.js",
        "embeds Node",
        "内置 Node",
        "内置 TypeScript",
        "内置 JavaScript",
    ] {
        assert_docs_reject_claim(&files, needle, "a bundled Node.js/TypeScript/jiti runtime");
    }
}

#[test]
fn docs_do_not_claim_ts_extension_api_compat() {
    // opi is not TypeScript-extension-API compatible with pi.
    let files = [
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
        "docs/pi-alignment-matrix.md",
        "docs/pi-alignment-matrix.zh.md",
    ];
    // "TypeScript extension API" / "TypeScript-compatible" scope the claim to the
    // extension surface; the bare "TypeScript API compatibility" phrase is
    // intentionally avoided because docs legitimately disclaim it ("It is not a
    // TypeScript API compatibility checklist").
    for needle in [
        "TypeScript extension API",
        "TypeScript extension compatibility",
        "TypeScript-compatible",
    ] {
        assert_docs_reject_claim(&files, needle, "TypeScript extension API compatibility");
    }
}

#[test]
fn docs_do_not_claim_pi_session_v3_compat() {
    // opi session JSONL is Rust-native and does not promise pi session v3
    // read/write file compatibility.
    let files = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
        "docs/pi-alignment-matrix.md",
        "docs/pi-alignment-matrix.zh.md",
    ];
    for needle in [
        "pi session v3 compatibility",
        "session v3 read/write compatibility",
        "supports pi session v3",
        "reads pi session v3",
        "writes pi session v3",
        "兼容 pi session v3",
    ] {
        assert_docs_reject_claim(&files, needle, "pi session v3 file compatibility");
    }
}

#[test]
fn docs_do_not_claim_pi_web_ui_parity() {
    // opi-web-ui is an unpublished Rust component crate, not pi-web-ui parity.
    let files = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
        "docs/pi-alignment-matrix.md",
        "docs/pi-alignment-matrix.zh.md",
    ];
    for needle in [
        "pi-web-ui parity",
        "web-ui parity",
        "pi web ui parity",
        "parity with pi-web-ui",
    ] {
        assert_docs_reject_claim(&files, needle, "pi-web-ui parity");
    }
}

#[test]
fn docs_do_not_claim_broad_oauth_provider_parity() {
    // OAuth and broad provider coverage are deferred/separate product decisions.
    let files = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
        "docs/pi-alignment-matrix.md",
        "docs/pi-alignment-matrix.zh.md",
    ];
    for needle in [
        "OAuth parity",
        "provider parity",
        "parity with OAuth",
        "broad OAuth",
        "provider coverage parity",
        "OAuth 对等",
        "provider 对等",
    ] {
        assert_docs_reject_claim(&files, needle, "broad OAuth or provider parity");
    }
}

#[test]
fn docs_do_not_claim_opi_types_or_protocol_migration() {
    // Adapter protocol types must stay in opi-coding-agent. Needles are scoped
    // to positive migration/creation claims so the legitimate "Why There Is No
    // opi-types" section is not tripped.
    let files = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
        "docs/pi-alignment-matrix.md",
        "docs/pi-alignment-matrix.zh.md",
    ];
    for needle in [
        "moved to opi-types",
        "migrated to opi-types",
        "introduced opi-types",
        "opi-types crate",
        "extracted into opi-types",
        "shared opi-types crate",
        "adapter protocol types now live in opi-types",
    ] {
        assert_docs_reject_claim(
            &files,
            needle,
            "migration of adapter protocol types to an opi-types crate",
        );
    }
}

// ---------------------------------------------------------------------------
// Negative code guards: no Phase 6 non-goal implementation exists (SC 9)
// ---------------------------------------------------------------------------

#[test]
fn workspace_has_no_opi_types_crate() {
    // Phase 6 forbids a shared types crate.
    let cargo = read_repo_file("Cargo.toml");
    assert!(
        !cargo.contains("opi-types") && !cargo.contains("opi_types"),
        "root Cargo.toml must not declare an opi-types workspace member or dependency"
    );
    assert!(
        !repo_root().join("crates/opi-types").exists(),
        "no crates/opi-types directory may exist (Phase 6 forbids a shared types crate)"
    );
}

#[test]
fn workspace_has_no_bundled_js_ts_runtime() {
    // A bundled JS/TS runtime would pull in one of these crates. Phase 6 forbids it.
    let cargo_files: Vec<_> = std::iter::once(repo_root().join("Cargo.toml"))
        .chain(
            std::fs::read_dir(repo_root().join("crates"))
                .expect("read crates directory")
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path().join("Cargo.toml");
                    path.is_file().then_some(path)
                }),
        )
        .collect();
    for forbidden in [
        "jiti",
        "deno_core",
        "deno_runtime",
        "boa_engine",
        "rquickjs",
        "rusty_v8",
        "swc",
        "oxc",
        "neon",
        "napi",
    ] {
        for path in &cargo_files {
            let cargo = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            assert!(
                !cargo.contains(forbidden),
                "{} must not depend on a JS/TS runtime crate ({forbidden}); Phase 6 forbids a bundled Node.js/TypeScript/jiti runtime",
                path.display()
            );
        }
    }
}

#[test]
fn first_class_provider_set_is_unchanged() {
    // First-class providers arrive only as the known nine. Any additional
    // provider module under opi-ai/src would indicate core provider broadening
    // (a Phase 6 non-goal); custom providers must use runtime registration.
    let providers_dir = repo_root().join("crates/opi-ai/src");
    let known_providers: std::collections::BTreeSet<&str> = [
        "anthropic",
        "azure_openai",
        "bedrock",
        "gemini",
        "mistral",
        "openai_chat",
        "openai_responses",
        "openrouter",
        "vertex",
    ]
    .into_iter()
    .collect();
    let infra: std::collections::BTreeSet<&str> = [
        "lib",
        "config",
        "http",
        "message",
        "model",
        "provider",
        "registry",
        "retry",
        "stream",
        "test_support",
    ]
    .into_iter()
    .collect();

    let mut actual_providers = std::collections::BTreeSet::<String>::new();
    let entries = std::fs::read_dir(&providers_dir)
        .unwrap_or_else(|e| panic!("failed to read opi-ai/src: {e}"));
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = name.strip_suffix(".rs").map(str::to_owned).unwrap_or(name);
        if infra.contains(stem.as_str()) {
            continue;
        }
        assert!(
            known_providers.contains(stem.as_str()),
            "opi-ai/src has an unexpected module '{stem}'; the first-class provider set must stay at the nine known providers (a new first-class provider is a Phase 6 non-goal)"
        );
        actual_providers.insert(stem);
    }
    for provider in known_providers {
        assert!(
            actual_providers.contains(provider),
            "expected first-class provider module '{provider}' is missing from opi-ai/src"
        );
    }
}

#[test]
fn adapter_protocol_types_stay_in_coding_agent() {
    // The adapter protocol module remains a coding-agent product surface and
    // must not migrate into opi-agent or a shared crate during Phase 6.
    let root = repo_root();
    assert!(
        root.join("crates/opi-coding-agent/src/adapter_protocol.rs")
            .exists(),
        "adapter_protocol.rs must remain in opi-coding-agent"
    );
    assert!(
        !root
            .join("crates/opi-agent/src/adapter_protocol.rs")
            .exists(),
        "opi-agent must not host adapter_protocol.rs; protocol types stay in opi-coding-agent"
    );
    let agent_cargo = read_repo_file("crates/opi-agent/Cargo.toml");
    assert!(
        !agent_cargo.contains("adapter_protocol"),
        "opi-agent Cargo.toml must not reference adapter_protocol"
    );
}

// ---------------------------------------------------------------------------
// Positive guard: Phase 5 MVP adapter capability surface stays documented
// ---------------------------------------------------------------------------

#[test]
fn docs_describe_phase5_adapter_capability_surface() {
    let spec = read_repo_file("docs/opi-spec.md");
    // Phase 5 adapters bridge the full capability surface; docs must keep
    // stating each capability so the MVP claim remains truthful.
    for phrase in [
        "`opi package add/remove/list/doctor` works",
        "packages with `[adapter]` sections start as child processes using `opi-extension-jsonl-v1`",
        "tools, commands, hooks, and events through child process adapters",
        "adapter tools, commands, hooks, state, and cancellation bridge into the existing extension API",
        "before_tool_call",
        "after_tool_call",
        "transform_context",
        "prepare_next_turn",
    ] {
        assert!(
            spec.to_lowercase().contains(&phrase.to_lowercase()),
            "opi-spec must describe the Phase 5 adapter capability surface (missing: {phrase})"
        );
    }
}
