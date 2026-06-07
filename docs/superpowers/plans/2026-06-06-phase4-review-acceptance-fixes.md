# Phase 4 Review Acceptance Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use
> checkbox status because later agents may resume from this file.

**Goal:** Close the three review findings blocking Phase 4 acceptance: extension
event callbacks must observe real `CodingHarness` agent events, the Phase 4
ledger must record the current spec hash, and public README crate descriptions
must stop describing the removed transport abstraction as current API.

**Architecture:** Keep the implementation minimal. Use the existing agent event
subscriber path to dispatch `AgentEvent`s to the `ExtensionRegistry`; do not add
a second event protocol. Add regression tests before implementation for the
missing event wiring and stale documentation. Update only the ledger hash value
required by the current spec file contents.

**Tech Stack:** Rust 2024, Tokio tests, existing `opi-agent` / `opi-coding-agent`
test helpers, workspace dependency `sha2`, Cargo workspace verification.

---

## Findings Covered

1. `Extension::on_event` is documented and implemented on the registry, but
   `CodingHarnessBuilder` does not wire extension event dispatch into the real
   `Agent` event sink. A harness-installed extension can add tools and hooks but
   cannot observe `AgentStart`, `TurnStart`, tool, compaction, or terminal
   events.
2. `docs/snapshots/phase4/opi-impl-state.json` records a stale
   `spec_files_sha256["docs/opi-spec.md"]` value. Current expected value from
   the review is:
   `5ef729fae53b478794921af26a135bc98245f190d26bf05421f6c692444495f6`.
3. `README.md` and `README.zh.md` still describe `opi-agent` as exposing a
   transport abstraction even though the transport stub was removed.

## Files To Change

- `crates/opi-agent/src/extension.rs`
- `crates/opi-agent/tests/transport.rs`
- `crates/opi-coding-agent/Cargo.toml`
- `crates/opi-coding-agent/src/harness.rs`
- `crates/opi-coding-agent/tests/extensions.rs`
- `crates/opi-coding-agent/tests/phase4_ledger.rs`
- `README.md`
- `README.zh.md`
- `docs/snapshots/phase4/opi-impl-state.json`

Do not commit unless explicitly requested. If a commit is later requested, stage
only the files listed above with explicit `git add <path>` commands.

---

## Task 1: Add Regression Test For Harness Extension Events

- [ ] Read these files in full before editing:
  - `crates/opi-agent/src/extension.rs`
  - `crates/opi-coding-agent/src/harness.rs`
  - `crates/opi-coding-agent/tests/extensions.rs`
- [ ] In `crates/opi-coding-agent/tests/extensions.rs`, add
  `use opi_agent::event::AgentEvent;` if it is not already imported.
- [ ] Add a new test named
  `harness_builder_extension_observes_agent_events`.
- [ ] Define a local `EventRecorderExtension` in that test:
  - Store `events: Arc<Mutex<Vec<&'static str>>>`.
  - Implement `Extension` with `name() -> "event-recorder"`.
  - Implement `on_event(&self, event: &AgentEvent)` and push stable labels for
    at least `AgentStart`, `TurnStart`, `MessageStart`, and `AgentEnd`; ignore
    unrelated events.
- [ ] Build a harness using the same pattern as
  `harness_builder_wraps_extension_registry_hooks_and_tools`:
  - `MockProvider::new("mock", vec![text_response("Done")])`
  - `CodingHarness::builder(Box::new(provider), "mock:mock-model".into(),
    OpiConfig::default(), workspace.path().to_path_buf())`
  - `.extension_registry(registry)`
  - `.tool_selection(ToolSelection::Disabled)`
  - `.build()`
- [ ] Run `harness.prompt("hello").await.unwrap();`.
- [ ] Assert the recorded event list contains `AgentStart`, `TurnStart`, and
  `AgentEnd`. Also assert `MessageStart` if the existing agent loop emits it for
  a text-only mock response; if that assertion is flaky, keep the test focused
  on lifecycle events.
- [ ] Run the targeted test before implementing the fix:

```powershell
cargo test -p opi-coding-agent --test extensions harness_builder_extension_observes_agent_events -- --nocapture
```

Expected before fix: the test fails because the recorded event list is empty or
missing lifecycle events.

## Task 2: Wire Extension Event Dispatch Through CodingHarness

- [ ] In `crates/opi-agent/src/extension.rs`, make `ExtensionRegistry` clonable
  without cloning extension objects:

```rust
impl Clone for ExtensionRegistry {
    fn clone(&self) -> Self {
        Self {
            extensions: self.extensions.clone(),
        }
    }
}
```

- [ ] Keep the existing `RegistryLocked` behavior intact. Cloning the registry
  must share the same `Arc<Vec<Box<dyn Extension>>>`; it must not reopen
  registration after hooks or event dispatch have been wrapped.
- [ ] In `crates/opi-coding-agent/src/harness.rs`, inside the harness build path
  that currently consumes `extension_registry` for tools, providers, and
  `wrap_hooks`, retain a clone for event dispatch before moving the registry
  into `wrap_hooks`.
- [ ] After constructing the `Agent`, call `agent.subscribe(...)` with a closure
  that invokes `registry.dispatch_event(event)`.
- [ ] Ensure this subscription is installed before returning `CodingHarness` and
  before user code can call `CodingHarness::subscribe`.
- [ ] Do not change `Agent::build_event_sink` unless the subscriber approach
  cannot preserve ordering or lifetime requirements.
- [ ] Run:

```powershell
cargo test -p opi-coding-agent --test extensions harness_builder_extension_observes_agent_events -- --nocapture
cargo test -p opi-agent --test extensions register_after_wrap_hooks_returns_error_instead_of_panicking wrap_event_sink_dispatches_to_extensions -- --nocapture
```

Expected after fix: the new harness test passes, and the existing extension
registry tests continue to pass.

## Task 3: Add A Ledger Hash Regression Test

- [ ] In `crates/opi-coding-agent/Cargo.toml`, add `sha2 = { workspace = true }`
  under `[dev-dependencies]`.
- [ ] Create `crates/opi-coding-agent/tests/phase4_ledger.rs`.
- [ ] Add a test named `phase4_ledger_spec_hash_matches_current_spec`.
- [ ] The test should:
  - Resolve the repo root with
    `Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")`.
  - Read `docs/opi-spec.md`.
  - Normalize CRLF to LF and compute SHA-256 with
    `sha2::{Digest, Sha256}`.
  - Read `docs/snapshots/phase4/opi-impl-state.json` as
    `serde_json::Value`.
  - Compare the computed hex string to
    `value["spec_files_sha256"]["docs/opi-spec.md"].as_str().unwrap()`.
- [ ] Run the test before editing the ledger:

```powershell
cargo test -p opi-coding-agent --test phase4_ledger -- --nocapture
```

Expected before fix: the test fails with the stale hash currently recorded in
the ledger.

## Task 4: Update The Phase 4 Ledger Hash

- [ ] In `docs/snapshots/phase4/opi-impl-state.json`, update only
  `spec_files_sha256["docs/opi-spec.md"]` to:

```text
5ef729fae53b478794921af26a135bc98245f190d26bf05421f6c692444495f6
```

- [ ] Do not edit task evidence, commit hashes, evaluator fields, or the current
  phase value unless a new mismatch is found during implementation.
- [ ] Run:

```powershell
cargo test -p opi-coding-agent --test phase4_ledger -- --nocapture
$spec = (Get-Content -Raw docs\opi-spec.md) -replace "`r`n", "`n"
$bytes = [System.Text.Encoding]::UTF8.GetBytes($spec)
$sha = [System.Security.Cryptography.SHA256]::Create()
$actual = ([System.BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLower()
$ledger = (Get-Content -Raw docs\snapshots\phase4\opi-impl-state.json | ConvertFrom-Json).spec_files_sha256.'docs/opi-spec.md'
if ($actual -ne $ledger) { throw "phase4 ledger hash mismatch: actual=$actual ledger=$ledger" }
```

Expected after fix: the Rust test passes and the PowerShell hash check exits
without throwing.

## Task 5: Add README Transport Description Regression Coverage

- [ ] In `crates/opi-agent/tests/transport.rs`, extend
  `public_specs_do_not_describe_removed_transport_stub_as_current` or add a new
  test named `public_readmes_do_not_claim_transport_abstraction`.
- [ ] Check these files:
  - `README.md`
  - `README.zh.md`
- [ ] Assert that `README.md` does not contain the exact phrase
  `transport abstraction`.
- [ ] Assert that `README.zh.md` does not contain the exact phrase
  `transport 抽象`.
- [ ] Keep the existing spec-file stale phrase checks unchanged.
- [ ] Run before editing the READMEs:

```powershell
cargo test -p opi-agent --test transport public_readmes_do_not_claim_transport_abstraction -- --nocapture
```

If the checks are folded into the existing test instead of a new test, run:

```powershell
cargo test -p opi-agent --test transport public_specs_do_not_describe_removed_transport_stub_as_current -- --nocapture
```

Expected before fix: the README assertion fails.

## Task 6: Update README And Localized README Descriptions

- [ ] In `README.md`, replace the `opi-agent` crate table description with:

```text
Agent loop, tool execution, hooks, events, queues, sessions, compaction, SDK types, extension API, and streaming proxy primitives
```

- [ ] In `README.zh.md`, replace the corresponding Chinese description with:

```text
Agent 主循环、工具执行、hooks、事件、队列、会话、压缩、SDK 类型、extension API 和 streaming proxy 原语
```

- [ ] Run:

```powershell
cargo test -p opi-agent --test transport -- --nocapture
rg -n "transport abstraction|transport 抽象" README.md README.zh.md
```

Expected after fix: transport tests pass; `rg` returns no matches in the two
README files.

## Task 7: Run Acceptance Gates

- [ ] Run focused tests:

```powershell
cargo test -p opi-coding-agent --test extensions -- --nocapture
cargo test -p opi-coding-agent --test phase4_ledger -- --nocapture
cargo test -p opi-agent --test transport -- --nocapture
```

- [ ] Run workspace gates:

```powershell
cargo fmt --check --all
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps
git diff --check
```

- [ ] Run a final scoped audit search:

```powershell
rg -n "Extension::on_event|wrap_event_sink|dispatch_event|transport abstraction|transport 抽象|spec_files_sha256" crates README.md README.zh.md docs\snapshots\phase4\opi-impl-state.json
```

Expected final state:

- `Extension::on_event` is exercised through `CodingHarness::builder`.
- Phase 4 ledger hash matches the current `docs/opi-spec.md` hash.
- README and README.zh no longer describe a removed transport abstraction.
- All focused tests and workspace gates pass.

## Risk Notes

- Event dispatch must not execute extension callbacks under an internal agent
  mutex. The subscriber closure should only call `ExtensionRegistry::dispatch_event`
  through the normal event fan-out path.
- Do not replace or remove `wrap_event_sink`; it is still useful for embedders
  that explicitly compose event sinks outside `CodingHarness`.
- Do not add backward compatibility shims for the removed transport stub. The
  acceptance finding is stale documentation, not a request to restore API.
- Keep ledger changes surgical. If `docs/opi-spec.md` changes during the repair,
  recompute the hash and update the expected value in both the ledger and the
  new test expectation through computed comparison, not a hard-coded assertion.
