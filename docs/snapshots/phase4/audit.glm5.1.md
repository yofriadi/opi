# Phase 4 Audit (GLM-5.1)

**Date**: 2026-06-05
**Auditor**: GLM-5.1 via Claude Code
**Scope**: All 18 tasks (4.1 through 4.11, including subtasks)
**Source**: `docs/snapshots/phase4/opi-impl-state.json`
**Head commit at audit**: `7a903c0`
**Methodology**: Full source and test read of all 18 tasks across 6 dimensions (spec compliance, test coverage, API stability, dependency hygiene, security, code quality) with live gate verification.

---

## 1. Executive Summary

**Overall assessment: CONDITIONAL PASS**

Phase 4 is substantively complete. All 18 tasks meet their definition of done, all gate checks pass, and the extension/RPC/discovery substrate is functional. The phase can close after addressing one critical finding and reviewing the high-priority warnings.

| Metric | Value |
|--------|-------|
| Tasks audited | 18 |
| Total findings | 72 |
| Critical | 1 |
| Warning | 20 |
| Info | 51 |

**Phase readiness verdict**: Close after fixing the critical finding (blocking I/O in async context) and addressing security warnings (path traversal, secret redaction false positives, misleading error types). Warnings in code-quality and api-stability are deferrable.

---

## 2. Gate Verification (Live)

All four gates verified against the current HEAD (`7a903c0`):

| Gate | Result |
|------|--------|
| `cargo fmt --check --all` | PASS (no output) |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS (1523 tests) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | PASS |

No regressions since the last task verification.

---

## 3. Per-Task Findings

### 3.1 Task 4.1 -- RPC JSONL mode

**Spec compliance**: PASS WITH WARNINGS
**Test coverage**: 27 tests (15 parsing, 8 subprocess, 4 unit). Adequate for parsing and framing, but no subprocess test drives prompt/continue end-to-end.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.1-01 | WARNING | `set_thinking_level` returns success but is a no-op -- the `level` value is discarded. A lying success is worse than an honest error. (`rpc.rs:284-294`) |
| 4.1-02 | WARNING | No subprocess test exercises prompt/continue with a live mock provider. Subprocess tests cover framing but not agent-driving flows. (`tests/rpc_jsonl.rs`) |
| 4.1-03 | WARNING | RPC subprocess tests set `ANTHROPIC_API_KEY` env var with a dummy value. Pattern is fragile. (`tests/rpc_jsonl.rs:158`) |
| 4.1-07 | WARNING | RPC runner uses blocking `stdin.read_line()` inside an async context. Same issue as 4.10-001 but in the CLI entry point. (`rpc.rs:176`) |
| 4.1-04 | INFO | Subprocess tests silently skip if binary is not pre-built. |
| 4.1-05 | INFO | `handle_agent_result` is a no-op for all branches -- dead code or placeholder. |
| 4.1-06 | INFO | RPC module docs correctly mark protocol as unstable 0.x. (Positive) |

### 3.2 Task 4.2 -- SDK embedding surface

**Spec compliance**: PASS WITH WARNINGS
**Test coverage**: 38 tests (33 opi-agent + 5 opi-coding-agent). Thorough coverage of all command variants and agent flows.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.2-01 | WARNING | `SdkResponse` derives `Serialize` but not `Deserialize`, preventing round-trip deserialization. Breaks "shared types without duplication" goal. (`sdk.rs:177-178`) |
| 4.2-02 | WARNING | `agent_event_to_value` fallback emits `SessionPersistError` type on serialization failure -- misleading event classification. (`sdk.rs:246-251`) |
| 4.2-06 | WARNING | `SdkCommand::id()` repeats identical match arms 10 times instead of using a helper or macro. (`sdk.rs:119-133`) |

### 3.3 Task 4.3 -- settle opi-agent::Transport

**Spec compliance**: PASS
**Test coverage**: 2 transport tests + 144 opi-agent tests. Verified Transport module is fully removed.
**Key findings**: No actionable findings. Clean removal with all references updated across root docs.

### 3.4 Task 4.4 -- Extension trait, lifecycle hooks, custom tools/commands/messages/state

**Spec compliance**: PASS
**Test coverage**: 26 tests (22 opi-agent + 4 opi-coding-agent). Covers registration, execution, blocking/error, state isolation, serialization.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.4-04 | WARNING | Extension trait async hooks require verbose `Pin<Box<dyn Future>>` boilerplate. Consider `async-trait` or `impl Future` return types. (`extension.rs:204-238`) |

### 3.5 Task 4.5 -- Extension/resource loading strategy

**Spec compliance**: PASS
**Test coverage**: 18 tests (discovery, precedence, normalization, duplicates, errors, minimal manifests).
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.5-03 | WARNING | `canonicalize()` falls back to uncanonicalized path on error -- potential path traversal bypass. (`resource.rs:209`) |
| 4.5-04 | WARNING | Resource discovery module exists but is not wired into any production code path. Consumers must call it manually. (`resource.rs`) |

### 3.6 Task 4.6 -- Custom provider/model registration

**Spec compliance**: PASS
**Test coverage**: 28 tests (20 opi-ai + 8 opi-coding-agent). Covers registration, resolution, capabilities, streaming, overrides, dedup, validation, precedence, listing.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.6-03 | WARNING | `RegistrationError` and `RegistryError` not re-exported from crate root. Consumers must import from submodule. (`opi-ai/src/lib.rs:27-31`) |
| 4.6-07 | WARNING | `RegistrationError` and `RegistryError` not marked `#[non_exhaustive]`. Adding variants is a breaking change in 0.x. (`registry.rs:73-95`) |

### 3.7 Task 4.7.1 -- Skills with progressive discovery

**Spec compliance**: PASS
**Test coverage**: 29 tests (14 parsing, 8 discovery, 4 registry, 2 progressive disclosure, 1 body loading).
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.7.1-01 | WARNING | Duplicated helper functions (`extract_frontmatter`, `parse_field`, `strip_yaml_quotes`, `extract_body`, `validate_name`, `validate_description`) between `skill.rs` and `prompt_fragment.rs`. Should be factored into shared module. (`skill.rs:150-252`, `prompt_fragment.rs:180-342`) |

### 3.8 Task 4.7.2 -- Prompt fragments/templates

**Spec compliance**: PASS
**Test coverage**: 40 tests. Full coverage of parsing, expansion, discovery, precedence.
**Key findings**: No warnings. Clean implementation.

### 3.9 Task 4.7.3 -- Themes with progressive discovery

**Spec compliance**: PASS
**Test coverage**: 57 tests (36 theme_discovery + 21 theme_snapshots). Cross-crate dependency is clean.
**Key findings**: No warnings. Clean separation between opi-tui (token schema, color parsing) and opi-coding-agent (TOML discovery).

### 3.10 Task 4.7.4 -- Packages with progressive resource composition

**Spec compliance**: PASS
**Test coverage**: 36 tests. Covers manifest parsing, discovery, precedence, composition, disabled resources, duplicates, missing assets, security.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.7.4-01 | WARNING | Path traversal security check uses `canonicalize() + starts_with()` with fallback to uncanonicalized path on error. Edge case on Windows (UNC paths). (`package_discovery.rs:406-418`) |
| 4.7.4-03 | WARNING | No test for actual symlink-based path traversal in package composition. Only the error variant is tested, not runtime detection. (`tests/package_discovery.rs:794-828`) |

### 3.11 Task 4.8.1 -- Permission gate extension example

**Spec compliance**: PASS WITH WARNINGS
**Test coverage**: 11 tests.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.8.1-A | WARNING | `package.toml` uses bare top-level format while later examples use `[package]` + `[package.extensions]`. Inconsistent manifest schema. |

### 3.12 Task 4.8.2 -- Protected paths extension example

**Spec compliance**: PASS
**Test coverage**: 14 tests. Covers allow, deny, edit, bash cwd, normalization, symlink, non-file tools, audit.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.8.2-A | WARNING | `resolve_components` silently swallows excessive `..` traversal without error or clamping. Path like `/workspace/../../../../../../etc/passwd` escapes workspace. (`tests/protected_paths_example.rs:149-161`) |

### 3.13 Task 4.8.3 -- Sub-agent extension example

**Spec compliance**: PASS
**Test coverage**: 10 tests. Covers completion, error propagation, cancellation, event routing, isolated state, session visibility.
**Key findings**: INFO only (no concurrent child run test; single `active_child_cancel` field limitation).

### 3.14 Task 4.8.4 -- Plan mode extension example

**Spec compliance**: PASS
**Test coverage**: 12 tests. Thorough coverage of mode transitions, tool gating, agent integration, state round-trips.
**Key findings**: None.

### 3.15 Task 4.8.5 -- Todo extension example

**Spec compliance**: PASS
**Test coverage**: 16 tests. Most thorough example test suite. All CRUD, validation, serialization, failure recovery.
**Key findings**: None. Positive reference for other examples.

### 3.16 Task 4.8.6 -- MCP adapter extension example

**Spec compliance**: PASS
**Test coverage**: 20 tests. Covers tool discovery, schema exposure, argument validation, execution success/error, resource metadata, cancellation.
**Key findings**: INFO only (cancellation test pattern could be simplified).

### 3.17 Task 4.9 -- Session branching UI

**Spec compliance**: PASS
**Test coverage**: 31 tests (22 session_branching + 9 branch_picker snapshots).
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.9-001 | WARNING | `BranchPicker::unicode_display_width` treats all non-control chars as width 1. CJK/emoji characters render with incorrect alignment. Use `unicode-width` crate instead. (`branch_picker.rs:232-234`) |

### 3.18 Task 4.10 -- Streaming proxy

**Spec compliance**: PASS
**Test coverage**: 25 tests. Comprehensive coverage of success, malformed frames, cancellation, redaction, backpressure, disconnect.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| **4.10-001** | **CRITICAL** | `StreamingProxy::run` is `pub async fn` but performs blocking `BufRead::read_line` synchronously on the tokio runtime. Uses `std::sync::mpsc` instead of `tokio::sync`. Either remove `async` or convert to proper async I/O. (`streaming_proxy.rs:147-182`) |
| 4.10-002 | WARNING | `SecretRedactor::simple_pattern_match` has high false positive rate. `sk-` prefix matches benign strings like `task-based`, `desk-top`. Needs regex or length validation. (`streaming_proxy.rs:424-436`) |

### 3.19 Task 4.11 -- Web UI consuming RPC/SDK events

**Spec compliance**: PASS
**Test coverage**: 62 tests (53 web_ui + 9 RPC integration). Covers event parsing, state machine, components, HTML rendering, XSS prevention, end-to-end flows.
**Key findings**:

| ID | Sev | Title |
|----|-----|-------|
| 4.11-001 | WARNING | `opi-web-ui` depends on `opi-agent` in regular dependencies, but only uses it in tests. Should be a dev-dependency to match CLAUDE.md layout (`opi-web-ui -> opi-ai`). (`Cargo.toml:14`) |

---

## 4. Cross-Cutting Analysis

### 4.1 API Surface Stability

- All public RPC, SDK, and extension types are documented as unstable 0.x in module docs. Good.
- `RegistrationError` and `RegistryError` in `opi-ai` lack `#[non_exhaustive]`, meaning adding error variants is technically a semver break even in 0.x. Low risk but should be added.
- `SdkResponse` lacking `Deserialize` is a practical gap for SDK embedders who need to parse responses.
- Extension trait's `Pin<Box<dyn Future>>` return types are ergonomic friction but not a stability issue.

### 4.2 Dependency Hygiene

- All crates use workspace dependencies correctly. No version pinning in individual `Cargo.toml` files.
- `opi-web-ui` incorrectly lists `opi-agent` as a regular dependency when only tests use it. Should be dev-only.
- Dependency graph matches the documented layout with the one exception above.
- No circular dependencies detected.

### 4.3 Security

Seven security-related findings across the phase:

1. **Path traversal fallback** (4.5-03, 4.7.4-01): `canonicalize()` failure falls back to uncanonicalized path in resource and package discovery. On error, the security check is effectively bypassed.
2. **Excessive `..` traversal** (4.8.2-A): Protected paths example silently swallows parent traversal past root without clamping or error.
3. **Secret redaction false positives** (4.10-002): `simple_pattern_match` matches benign strings containing `sk-` prefix.
4. **Misleading error event** (4.2-02): Serialization failure injected as `SessionPersistError` type.
5. **API key in test env** (4.1-03): Fragile pattern of passing env var for API key in subprocess tests.
6. **No symlink traversal test** (4.7.4-03): Path traversal security exists in code but is not tested with actual symlinks.
7. **`resolve_components` underflow** (4.8.2-A): Path past workspace root not detected as escape.

### 4.4 Code Quality

**Strengths**:
- Consistent use of `thiserror` for library error types across all modules.
- Module-level documentation is thorough and includes unstable 0.x warnings.
- Test naming is descriptive and follows consistent patterns.
- Cross-crate boundaries are clean (with one exception).

**Weaknesses**:
- Significant code duplication in discovery modules (~250 lines of near-identical frontmatter parsing and discovery loop logic across skill.rs, prompt_fragment.rs, theme_discovery.rs, package_discovery.rs).
- Blocking I/O in async contexts (4.10-001, 4.1-07).
- Dead code in `handle_agent_result` (4.1-05).

---

## 5. Finding Index

### Critical (must fix before Phase 4 close)

| ID | Task | Dimension | Title | Location |
|----|------|-----------|-------|----------|
| 4.10-001 | 4.10 | code-quality | StreamingProxy::run is async but performs blocking I/O on the tokio runtime | `streaming_proxy.rs:147-182` |

### Warning (should fix, deferrable)

| ID | Task | Dimension | Title | Location |
|----|------|-----------|-------|----------|
| 4.1-01 | 4.1 | spec-compliance | set_thinking_level is a no-op returning success | `rpc.rs:284-294` |
| 4.1-02 | 4.1 | test-coverage | No subprocess prompt/continue end-to-end test | `tests/rpc_jsonl.rs` |
| 4.1-03 | 4.1 | security | RPC tests pass API key via env var | `tests/rpc_jsonl.rs:158` |
| 4.1-07 | 4.1 | code-quality | RPC runner uses blocking stdin read in async context | `rpc.rs:176` |
| 4.2-01 | 4.2 | spec-compliance | SdkResponse lacks Deserialize | `sdk.rs:177-178` |
| 4.2-02 | 4.2 | security | agent_event_to_value fallback uses wrong event type | `sdk.rs:246-251` |
| 4.2-06 | 4.2 | code-quality | SdkCommand::id() repeats match arms | `sdk.rs:119-133` |
| 4.4-04 | 4.4 | api-stability | Extension async hooks require Pin<Box> boilerplate | `extension.rs:204-238` |
| 4.5-03 | 4.5 | security | canonicalize() falls back to uncanonicalized path | `resource.rs:209` |
| 4.5-04 | 4.5 | api-stability | Resource discovery not wired into production code | `resource.rs` |
| 4.6-03 | 4.6 | api-stability | RegistrationError/RegistryError not re-exported | `lib.rs:27-31` |
| 4.6-07 | 4.6 | api-stability | Error types not #[non_exhaustive] | `registry.rs:73-95` |
| 4.7.1-01 | 4.7.1 | code-quality | Duplicated frontmatter helpers across skill/fragment | `skill.rs:150-252` |
| 4.7.4-01 | 4.7.4 | security | Path traversal check bypass on canonicalize failure | `package_discovery.rs:406-418` |
| 4.7.4-03 | 4.7.4 | test-coverage | No symlink path traversal test | `tests/package_discovery.rs:794` |
| 4.8.1-A | 4.8.1 | spec-compliance | package.toml format inconsistency | `examples/permission-gate/` |
| 4.8.2-A | 4.8.2 | security | Excessive `..` traversal not detected | `tests/protected_paths_example.rs:149-161` |
| 4.9-001 | 4.9 | code-quality | Unicode display width incorrect for CJK/emoji | `branch_picker.rs:232-234` |
| 4.10-002 | 4.10 | security | SecretRedactor high false positive rate | `streaming_proxy.rs:424-436` |
| 4.11-001 | 4.11 | dependency-hygiene | opi-web-ui depends on opi-agent in regular deps | `Cargo.toml:14` |

### Info (51 findings -- not listed individually)

All info findings are positive observations (correct patterns, clean implementations, thorough test suites) or low-priority improvement suggestions. No action required.

---

## 6. Recommendations

### Before Phase 4 close

1. **Fix 4.10-001** (critical): Either remove `async` from `StreamingProxy::run` and make it synchronous (simpler, honest), or convert to proper async I/O with `tokio::io::BufReader`, `tokio::sync::mpsc`, and `select!`. The same pattern exists in 4.1-07 (RPC runner) and should be addressed consistently.

2. **Review security findings** (4.5-03, 4.7.4-01, 4.8.2-A): The `canonicalize()` fallback and excessive `..` traversal are real security edges. At minimum, log warnings when canonicalize fails and add underflow detection to `resolve_components`.

### Deferrable (post-close)

3. **Add `#[non_exhaustive]` to public error types** (4.6-07) -- prevents accidental semver breaks.
4. **Add `Deserialize` to `SdkResponse`** (4.2-01) -- enables round-trip SDK usage.
5. **Factor shared discovery infrastructure** (4.7.1-01) -- ~250 lines of duplicated frontmatter/discovery logic.
6. **Fix `agent_event_to_value` fallback** (4.2-02) -- use a dedicated error type, not `SessionPersistError`.
7. **Move opi-agent to dev-dependencies in opi-web-ui** (4.11-001).
8. **Fix unicode display width** (4.9-001) -- use `unicode-width` crate.
9. **Standardize package.toml format** (4.8.1-A).
10. **Improve SecretRedactor** (4.10-002) -- use regex or length validation.
