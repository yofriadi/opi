# Phase 4 Residual Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three residual issues found during Phase 4 review: stale transport wording in paired specs, unused `StreamingProxyError::Cancelled`, and panic-based late extension registration.

**Architecture:** Keep the Phase 4 settled API shape: no public `Transport` stub, streaming proxy cancellation remains a clean shutdown/event path, and extension registry lifecycle violations return typed errors instead of panicking.

**Tech Stack:** Rust 2024, Cargo workspace tests, `thiserror`, Markdown specs.

---

## Preconditions

- [ ] Run `git status --short` and confirm the working tree is already dirty from Phase 4 remediation.
- [ ] Do not revert or stage unrelated files.
- [ ] Read the affected files before editing:
  - `crates/opi-agent/src/extension.rs`
  - `crates/opi-agent/src/streaming_proxy.rs`
  - `crates/opi-agent/tests/extensions.rs`
  - `crates/opi-agent/tests/streaming_proxy.rs`
  - `crates/opi-agent/tests/transport.rs`
  - `docs/opi-spec.md`
  - `docs/opi-spec.zh.md`

## Task 1: Guard And Fix Stale Transport Spec Wording

**Problem:** `docs/opi-spec.md` and `docs/opi-spec.zh.md` still describe the current `transport` stub as reserved for Phase 4 even though the public stub was removed.

**Red check:**

- [ ] Add this regression test to `crates/opi-agent/tests/transport.rs`:

```rust
#[test]
fn public_specs_do_not_describe_removed_transport_stub_as_current() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let stale_phrases = [
        "current `transport` stub is reserved for the Phase 4 RPC/proxy transport",
        "当前的 `transport` 存根保留给第 4 阶段 RPC/proxy transport",
    ];

    for rel in ["docs/opi-spec.md", "docs/opi-spec.zh.md"] {
        let doc = std::fs::read_to_string(repo_root.join(rel)).expect(rel);
        for phrase in stale_phrases {
            assert!(
                !doc.contains(phrase),
                "{rel} still describes the removed transport stub as current"
            );
        }
    }
}
```

- [ ] Run `cargo test -p opi-agent --test transport public_specs_do_not_describe_removed_transport_stub_as_current -- --nocapture`.
- [ ] Expected result before the doc edit: failure because both specs still contain stale wording.

**Green change:**

- [ ] In `docs/opi-spec.md`, replace the stale sentence near the public API exposure constraints with wording that says `opi_agent::Transport` was removed in Phase 4 and that RPC/proxy surfaces now live in `opi-coding-agent::rpc`, `opi-agent::sdk`, and `opi-agent::streaming_proxy`.
- [ ] In `docs/opi-spec.zh.md`, make the same localized update in the corresponding section.
- [ ] Preserve the later Phase 4 history sections that already describe removal; do not duplicate them.
- [ ] Re-run the target test and confirm it passes.

## Task 2: Remove Dead `StreamingProxyError::Cancelled`

**Problem:** `StreamingProxyError::Cancelled` is public but never constructed. Cancellation currently emits `proxy_cancelled` and exits cleanly, so introducing an error path would be a behavior regression.

**Behavior lock:**

- [ ] Tighten `crates/opi-agent/tests/streaming_proxy.rs::cancellation_stops_proxy_cleanly` so it requires `StreamingProxy::run()` to return `Ok(writer)` on pre-cancelled input.
- [ ] Parse the output and assert a `proxy_cancelled` frame is emitted.
- [ ] Run `cargo test -p opi-agent --test streaming_proxy cancellation_stops_proxy_cleanly -- --nocapture`.
- [ ] Expected result before the source edit: pass. This is a behavior lock, not the red test.

**Green change:**

- [ ] In `crates/opi-agent/src/streaming_proxy.rs`, delete the unused `Cancelled` variant from `StreamingProxyError`.
- [ ] Remove any stale doc comment attached to that variant.
- [ ] Do not change `proxy_cancelled` event emission or clean-shutdown behavior.
- [ ] Run:
  - `cargo test -p opi-agent --test streaming_proxy cancellation_stops_proxy_cleanly cancellation_emits_proxy_cancelled_event -- --nocapture`
  - `rg -n "StreamingProxyError::Cancelled|Cancelled," crates/opi-agent/src crates/opi-agent/tests`
- [ ] Expected search result: no `StreamingProxyError::Cancelled` and no enum variant line; `proxy_cancelled` strings may remain.

## Task 3: Replace Late Extension Registration Panic With Typed Error

**Problem:** `ExtensionRegistry::register()` panics after `wrap_hooks()` or `wrap_event_sink()` because `Arc::get_mut()` returns `None`. Public registry mutation should report a typed error.

**Red check:**

- [ ] Add this test to the registration section of `crates/opi-agent/tests/extensions.rs`:

```rust
#[test]
fn register_after_wrap_hooks_returns_error_instead_of_panicking() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Box::new(RecordingExtension::new("first")))
        .unwrap();

    let _composite = registry.wrap_hooks(Box::new(TestHooks));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        registry.register(Box::new(RecordingExtension::new("late")))
    }));

    assert!(result.is_ok(), "late registration should not panic");
    assert!(matches!(
        result.unwrap(),
        Err(ExtensionError::RegistryLocked)
    ));
}
```

- [ ] Add the same shape of test for `wrap_event_sink()` if `register()` uses a shared path that is not fully covered by the `wrap_hooks()` test.
- [ ] Run `cargo test -p opi-agent --test extensions register_after_wrap_hooks_returns_error_instead_of_panicking -- --nocapture`.
- [ ] Expected result before the source edit: failure because the call panics or `RegistryLocked` does not exist.

**Green change:**

- [ ] In `crates/opi-agent/src/extension.rs`, add a `thiserror` variant:

```rust
#[error("cannot register extensions after registry has been shared")]
RegistryLocked,
```

- [ ] Change `ExtensionRegistry::register()` so the `Arc::get_mut()` `None` branch returns `Err(ExtensionError::RegistryLocked)` instead of panicking.
- [ ] Update the `register()` rustdoc to say late registration returns `ExtensionError::RegistryLocked`.
- [ ] Do not change duplicate-name behavior or lifecycle ordering.
- [ ] Re-run `cargo test -p opi-agent --test extensions register_after_wrap_hooks_returns_error_instead_of_panicking -- --nocapture`.

## Task 4: Targeted Regression Suite

- [ ] Run `cargo test -p opi-agent --test transport -- --nocapture`.
- [ ] Run `cargo test -p opi-agent --test streaming_proxy -- --nocapture`.
- [ ] Run `cargo test -p opi-agent --test extensions -- --nocapture`.
- [ ] Run `rg -n "current ``transport`` stub|当前的 ``transport`` 存根|StreamingProxyError::Cancelled|Cancelled," docs crates/opi-agent/src crates/opi-agent/tests`.
- [ ] Expected result: no stale transport sentence, no `StreamingProxyError::Cancelled`; only valid cancellation event wording may remain.

## Task 5: Workspace Gates

- [ ] Run `cargo fmt --check --all`.
- [ ] Run `cargo test --workspace --all-targets`.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Run `$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps`.
- [ ] Run `git diff --check`.

## Acceptance Criteria

- [ ] English and Chinese specs no longer claim that a current `transport` stub is reserved for Phase 4.
- [ ] `StreamingProxyError::Cancelled` is removed while cancellation still emits `proxy_cancelled` and exits cleanly.
- [ ] `ExtensionRegistry::register()` no longer panics after registry sharing; it returns `ExtensionError::RegistryLocked`.
- [ ] Targeted tests and full workspace gates pass.
- [ ] Final review notes include exact verification commands and any remaining risk.
