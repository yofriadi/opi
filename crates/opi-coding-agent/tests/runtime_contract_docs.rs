//! Phase 8 runtime-contract documentation and non-goal guard tests (task 8.7).
//!
//! These guards implement the Phase 8 design Success Criteria 6 and 8:
//!
//! - **SC 6** — public `opi-agent` runtime, extension, event, session, SDK/RPC,
//!   and streaming-proxy surfaces are classified in the docs (English and
//!   Chinese) as supported 0.x, unstable internal, or candidate removal
//!   (`phase8_api_surface_classification`). Each classified surface is pinned
//!   to its exact `pub use` re-export line in `crates/opi-agent/src/lib.rs`
//!   (renaming or removing a re-export fails the test), and `SessionEntry` is
//!   confirmed module-path-only.
//! - **SC 8** — Phase 8 did not claim or implement a stable 1.0 API, a
//!   TypeScript extension API, package ecosystem expansion, a new adapter kind,
//!   web UI, provider OAuth, in-core workflow tools, an MCP runtime, a shared
//!   `opi-types` crate, unjustified public type migration, or a whole-loop
//!   rewrite (`phase8_non_goals_not_claimed_or_implemented`).
//!
//! The Phase 8 non-goal set is disjoint from the Phase 6 guards in
//! `productized_packages_docs.rs` (npm/marketplace/OAuth-parity) and the
//! Phase 7 guards in `observability_docs.rs` (telemetry/analytics); this file
//! owns only the Phase 8 runtime-stabilization non-goals.

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

/// Helper: the repository root (two levels up from the test crate).
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Helper: assert that no line positively claims the forbidden `needle`.
/// Legitimate negation contexts (the guard itself saying "must not claim X")
/// are allowed.
///
/// Limitation: this is a per-line substring model shared with the Phase 6/7
/// guards. A line is treated as negated if any negation token co-occurs on it,
/// so a deliberately deceptive line that places a negation word next to a
/// positive claim (e.g. "Web UI is not optional") can leak through. The token
/// set is kept to the narrow Phase 7 baseline (no overly-common broadeners),
/// and the meta-guard pins the reliable behavior; fully closing co-occurrence
/// leaks would require structural clause parsing across all three phase guards
/// and is out of scope for this task.
fn no_positive_claim(haystack: &str, needle: &str) -> bool {
    let lower = haystack.to_lowercase();
    let needle_lower = needle.to_lowercase();
    for line in lower.lines() {
        if line.contains(&needle_lower) {
            if line.contains("reject")
                || line.contains("must not")
                || line.contains("do not")
                || line.contains("does not")
                || line.contains("not claim")
                || line.contains("no ")
                || line.contains("without")
                || line.contains("never ")
                || line.contains("不声明")
                || line.contains("不得")
                || line.contains("未实现")
                || line.contains("不会")
                || line.contains("从不")
                || line.contains("不收集")
                || line.contains("不传输")
                || line.contains("并未")
                || line.contains("并不")
            {
                continue;
            }
            return false;
        }
    }
    true
}

/// Helper: assert no file in `files` positively claims the forbidden `needle`.
fn assert_docs_reject_claim(files: &[&str], needle: &str, what: &str) {
    for path in files {
        let content = read_repo_file(path);
        assert!(
            no_positive_claim(&content, needle),
            "{path} must not positively claim {what}; forbidden phrase {needle:?} appeared outside a negation context"
        );
    }
}

// ===========================================================================
// SC 6: public opi-agent surfaces are classified (EN + ZH)
// ===========================================================================

#[test]
fn phase8_api_surface_classification() {
    let en = read_repo_file("crates/opi-agent/README.md");
    let zh = read_repo_file("crates/opi-agent/README.zh.md");

    // The classification section exists in both languages.
    assert!(
        contains_ci(&en, "API Surface Classification"),
        "EN opi-agent README must have an API Surface Classification section"
    );
    assert!(
        zh.contains("API 表面分类"),
        "ZH opi-agent README must have an API 表面分类 section"
    );

    // All three classification tiers are documented in both languages.
    for (tier_en, tier_zh) in [
        ("supported 0.x", "支持的 0.x"),
        ("unstable internal", "不稳定内部"),
        ("candidate removal", "候选移除"),
    ] {
        assert!(
            contains_ci(&en, tier_en),
            "EN README must document the {tier_en:?} classification tier"
        );
        assert!(
            zh.contains(tier_zh),
            "ZH README must document the {tier_zh:?} classification tier"
        );
    }

    // Every named surface is bound to its classification tier on a single doc
    // line (the table row), in both languages. Asserting surface + tier
    // co-occurrence prevents a silent misclassification (e.g. promoting an
    // unstable-internal surface to supported 0.x) from passing the guard.
    let classification: &[(&str, &str, &str)] = &[
        // (surface, tier EN, tier ZH)
        ("Agent", "supported 0.x", "支持的 0.x"),
        ("agent_loop", "supported 0.x", "支持的 0.x"),
        ("AgentHooks", "supported 0.x", "支持的 0.x"),
        ("Tool", "supported 0.x", "支持的 0.x"),
        ("AgentEvent", "supported 0.x", "支持的 0.x"),
        ("AgentSessionEvent", "unstable internal", "不稳定内部"),
        ("SessionEntry", "unstable internal", "不稳定内部"),
        ("Extension", "unstable internal", "不稳定内部"),
        ("ExtensionRegistry", "unstable internal", "不稳定内部"),
        ("SdkCommand", "unstable internal", "不稳定内部"),
        ("SdkResponse", "unstable internal", "不稳定内部"),
        ("StreamingProxy", "unstable internal", "不稳定内部"),
    ];
    for (surface, tier_en, tier_zh) in classification {
        assert!(
            en.lines()
                .any(|l| l.contains(surface) && contains_ci(l, tier_en)),
            "EN README must classify {surface} as {tier_en} on one line"
        );
        assert!(
            zh.lines()
                .any(|l| l.contains(surface) && l.contains(tier_zh)),
            "ZH README must classify {surface} as {tier_zh} on one line"
        );
    }

    // Honesty: the docs state there is no stable 1.0 promise and name the real
    // stability mechanism (#[non_exhaustive] plus module "# Unstable" prose;
    // no #[doc(hidden)] / #[unstable] feature gate).
    assert!(
        en.contains("no stable 1.0") && en.contains("#[non_exhaustive]"),
        "EN README must state no stable 1.0 promise and document #[non_exhaustive]"
    );
    assert!(
        zh.contains("不会给出稳定 1.0") && zh.contains("#[non_exhaustive]"),
        "ZH README must state 不会给出稳定 1.0 and document #[non_exhaustive]"
    );

    // The three wire schema versions are documented alongside the classification.
    for version in [
        "SDK_SCHEMA_VERSION = 3",
        "NDJSON_SCHEMA_VERSION = 2",
        "TRACE_SCHEMA_VERSION = 1",
    ] {
        assert!(
            en.contains(version),
            "EN README must document {version} with the classification"
        );
        assert!(
            zh.contains(version),
            "ZH README must document {version} with the classification"
        );
    }

    // Production cross-check: each classified crate-root surface is pinned to
    // its exact `pub use` re-export line in lib.rs (not a bare substring), so
    // renaming or removing a re-export fails the guard rather than being masked
    // by an unrelated identifier (e.g. a bare `Agent` check would also match
    // `AgentError`). SessionEntry is confirmed module-path-only.
    let lib = read_repo_file("crates/opi-agent/src/lib.rs");
    for reexport in [
        "pub use agent::Agent;",
        "pub use agent_loop::agent_loop;",
        "pub use hooks::AgentHooks;",
        "pub use event::{AgentEvent, AgentEventSink};",
        "pub use session_event::AgentSessionEvent;",
        "pub use sdk::{SDK_SCHEMA_VERSION, SdkCommand, SdkResponse};",
        "Extension, ExtensionCommand, ExtensionError, ExtensionHookResult, ExtensionRegistry",
        "ProxyConfig, ProxyEvent, ProxyHandler, SecretRedactor, StreamingProxy, StreamingProxyError",
        "ExecutionMode, Tool, ToolError, ToolResult",
    ] {
        assert!(
            lib.contains(reexport),
            "opi-agent lib.rs must contain the classified re-export: {reexport}"
        );
    }
    let session = read_repo_file("crates/opi-agent/src/session.rs");
    assert!(
        session.contains("SessionEntry"),
        "SessionEntry must remain in the session module"
    );
    assert!(
        !lib.contains("pub use session::SessionEntry"),
        "SessionEntry must NOT be hoisted to the crate root (unstable internal, module-path-only)"
    );
}

// ===========================================================================
// SC 8 + Non-Goals: forbidden scope is neither claimed nor implemented
// ===========================================================================

#[test]
fn phase8_non_goals_not_claimed_or_implemented() {
    let doc_files = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
        "docs/pi-alignment-matrix.md",
        "docs/pi-alignment-matrix.zh.md",
        "crates/opi-agent/README.md",
        "crates/opi-agent/README.zh.md",
    ];

    // --- Doc side: Phase 8 non-goals must not be positively claimed. ---
    let forbidden_en = [
        "stable 1.0 public API",
        "stable 1.0 API promise",
        "TypeScript extension API compatibility",
        "TypeScript extension API",
        "package ecosystem expansion",
        "package marketplace",
        "new adapter kind",
        "web UI",
        "web dashboard",
        "provider OAuth login",
        "OAuth client credentials",
        "in-core plan mode",
        "in-core sub-agent",
        "MCP runtime",
        "shared opi-types crate",
        "whole agent loop rewrite",
    ];
    let forbidden_zh = [
        "稳定 1.0 公共 API",
        "稳定 1.0 API 承诺",
        "TypeScript 扩展 API 兼容",
        "package 生态扩张",
        "package 市场",
        "新 adapter 类型",
        "Web UI",
        "Web 仪表盘",
        "供应商 OAuth 登录",
        "OAuth 客户端凭据",
        "内核 plan mode",
        "MCP 运行时",
        "共享 opi-types crate",
        "整个 agent loop 重写",
    ];
    for needle in forbidden_en.iter().chain(forbidden_zh.iter()) {
        assert_docs_reject_claim(&doc_files, needle, "a Phase 8 non-goal");
    }

    // --- Code side: forbidden surfaces are not implemented. ---

    // Gather root + crate Cargo.toml files.
    let mut cargo_files: Vec<std::path::PathBuf> = vec![repo_root().join("Cargo.toml")];
    for entry in std::fs::read_dir(repo_root().join("crates")).expect("read crates directory") {
        let entry = entry.expect("dir entry");
        let path = entry.path().join("Cargo.toml");
        if path.is_file() {
            cargo_files.push(path);
        }
    }

    // No shared opi-types crate (no workspace member, no dependency).
    for path in &cargo_files {
        let cargo = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        assert!(
            !cargo.contains("opi-types"),
            "{} must not add an opi-types crate; Phase 8 forbids a shared types crate",
            path.display()
        );
    }

    // No opi-web-ui workspace member (web UI product work is out of scope).
    let root_cargo =
        std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("root Cargo.toml");
    assert!(
        !root_cargo.contains("opi-web-ui"),
        "root Cargo.toml must not list opi-web-ui; Phase 8 forbids web UI product work"
    );

    // No OAuth client/credential crates (provider OAuth login is out of scope;
    // Vertex's bearer-token usage is not an OAuth client flow).
    for path in &cargo_files {
        let cargo = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        for forbidden in ["oauth2", "openidconnect", "tame-oauth"] {
            assert!(
                !cargo.contains(forbidden),
                "{} must not depend on an OAuth client crate ({forbidden}); Phase 8 forbids provider OAuth work",
                path.display()
            );
        }
    }

    // No in-core workflow / web / oauth / mcp modules in opi-agent.
    let lib = read_repo_file("crates/opi-agent/src/lib.rs");
    for forbidden_mod in [
        "pub mod web_ui",
        "pub mod oauth",
        "pub mod mcp",
        "pub mod plan_mode",
        "pub mod sub_agent",
        "pub mod todo",
        "pub mod permission_popup",
    ] {
        assert!(
            !lib.contains(forbidden_mod),
            "opi-agent lib.rs must not declare {forbidden_mod}; Phase 8 forbids in-core workflow/web/oauth/mcp runtime"
        );
    }

    // No whole-loop rewrite: the public agent_loop entry point still exists.
    let agent_loop = read_repo_file("crates/opi-agent/src/agent_loop.rs");
    assert!(
        agent_loop.contains("pub async fn agent_loop"),
        "the public agent_loop entry point must still exist; Phase 8 forbids a whole-loop rewrite"
    );

    // No new adapter kind: the adapter protocol is still opi-extension-jsonl-v1 only.
    let adapter_protocol = read_repo_file("crates/opi-coding-agent/src/adapter_protocol.rs");
    assert!(
        adapter_protocol.contains("opi-extension-jsonl-v1"),
        "the adapter protocol must remain opi-extension-jsonl-v1"
    );
    for forbidden_proto in [
        "opi-extension-jsonl-v2",
        "opi-extension-mcp",
        "opi-extension-grpc",
        "opi-extension-websocket",
    ] {
        assert!(
            !adapter_protocol.contains(forbidden_proto),
            "adapter_protocol must not introduce a second adapter kind ({forbidden_proto})"
        );
    }
}

/// Meta-guard: the negation helper still rejects synthetic positive claims, so
/// the non-goal guards cannot be silently weakened. Pins the helper's reliable
/// behavior (positives without any negation token are rejected; clear
/// negations pass). See `no_positive_claim` for the documented substring
/// co-occurrence limitation.
#[test]
fn phase8_negation_helper_rejects_positive_claims() {
    // Positives with no negation token are rejected.
    assert!(
        !no_positive_claim("opi now ships a stable 1.0 API", "stable 1.0 API"),
        "a positive 1.0 claim must be rejected"
    );
    assert!(
        !no_positive_claim(
            "opi adds a TypeScript extension API",
            "TypeScript extension API"
        ),
        "a positive TypeScript extension API claim must be rejected"
    );
    assert!(
        !no_positive_claim("opi ships a web UI dashboard", "web UI dashboard"),
        "a positive web UI dashboard claim must be rejected"
    );
    assert!(
        !no_positive_claim("opi 提供 稳定 1.0 公共 API", "稳定 1.0 公共 API"),
        "a positive Chinese 1.0 claim must be rejected"
    );
    assert!(
        !no_positive_claim("opi 新增 共享 opi-types crate", "共享 opi-types crate"),
        "a positive Chinese opi-types claim must be rejected"
    );
    // Legitimate negation contexts pass.
    assert!(
        no_positive_claim("opi does not ship a stable 1.0 API", "stable 1.0 API"),
        "a clear English negation must pass the helper"
    );
    assert!(
        no_positive_claim("opi does not add a new adapter kind", "new adapter kind"),
        "a clear English negation must pass the helper"
    );
    assert!(
        no_positive_claim("opi 不得引入 MCP 运行时", "MCP 运行时"),
        "a clear Chinese negation must pass the helper"
    );
}
