# Phase 6 Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verify and repair the confirmed Phase 6 audit findings that affect truthful sign-off, command-only adapter state persistence, and regression guard teeth.

**Architecture:** Keep changes surgical. Documentation truth fixes stay in docs and docs guard tests; runtime persistence fixes stay in `CodingHarness` and its RPC caller; package doctor/list duplicate visibility uses the existing resolved package pipeline instead of adding a new diagnostic model.

**Tech Stack:** Rust 2024, tokio tests, existing `opi-coding-agent` integration tests, markdown docs.

---

## File Structure

- Modify: `AGENTS.md`
  - Update the live workspace version claim from `0.5.0` to `0.5.1`.
- Modify: `CLAUDE.md`
  - Update the live project summary from `v0.5.0 ships` to `v0.5.1 ships`.
- Modify: `docs/snapshots/phase6/audit-baseline.md`
  - Add final Phase 6 reconciliation and reclassify closed findings that the current audit documents show as stale.
- Modify: `docs/pi-alignment-matrix.zh.md`
  - Synchronize Phase 4 package/extension rows, add the Phase 5 package/adapter row, and align the P1 extension/package execution priority with the English matrix.
- Modify: `crates/opi-coding-agent/tests/productized_packages_docs.rs`
  - Extend version, localization, baseline, non-goal, and positive capability guards.
- Modify: `crates/opi-coding-agent/tests/session_extension_state.rs`
  - Add regression tests for command-only state restore and command-only state persistence.
- Modify: `crates/opi-coding-agent/src/harness.rs`
  - Restore pending extension state before command dispatch and persist extension state after a handled successful command.
- Modify: `crates/opi-coding-agent/src/rpc.rs`
  - Borrow the harness mutably for `extension_command` dispatch.
- Modify: `crates/opi-coding-agent/tests/extensions.rs`
  - Update harness variable mutability after `dispatch_extension_command` becomes mutable.
- Modify: `crates/opi-coding-agent/src/package_cli.rs`
  - Use `resolve_installed_packages` for list/doctor so runtime duplicate diagnostics are visible.
- Modify: `crates/opi-coding-agent/tests/package_cli.rs`
  - Add CLI-level duplicate-name diagnostic tests for list/doctor.
- Modify: `crates/opi-coding-agent/src/adapter_protocol.rs`
  - Correct failure-semantics documentation for after-tool hooks, protocol mismatch diagnostics, and drop-vs-explicit shutdown behavior.

## Task 1: Documentation Truth and Baseline Reconciliation

**Files:**
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `docs/snapshots/phase6/audit-baseline.md`
- Modify: `docs/pi-alignment-matrix.zh.md`
- Modify: `crates/opi-coding-agent/tests/productized_packages_docs.rs`

- [ ] **Step 1: Write failing guards**

Add assertions that:
- `AGENTS.md` contains `Current workspace version: \`0.5.1\``.
- `CLAUDE.md` contains `v0.5.1 ships`.
- The Phase 6 baseline no longer contains `None of those tasks is passing yet`, and names closed product-loop, four-hook, adapter command containment, RPC startup diagnostics, and adapter state persistence findings.
- The Chinese alignment matrix contains the Phase 5 row and the current P1 process-JSONL adapter bridge claim.

- [ ] **Step 2: Run the focused guards and verify RED**

Run:

```sh
cargo test -p opi-coding-agent --test productized_packages_docs phase6_current_docs_match_workspace_version
cargo test -p opi-coding-agent --test productized_packages_docs phase6_localized_docs_stay_in_sync
cargo test -p opi-coding-agent --test productized_packages_docs phase6_baseline_audit_is_complete
```

Expected: fail on stale `AGENTS.md` / `CLAUDE.md`, stale baseline language, and missing ZH matrix Phase 5/process-JSONL text.

- [ ] **Step 3: Apply the documentation fixes**

Update only the current-state claims and current audit reconciliation. Do not edit Phase 5 audit snapshots or released changelog sections.

- [ ] **Step 4: Run the focused guards and verify GREEN**

Run the same three commands. Expected: all pass.

## Task 2: Command-Only Extension State Persistence

**Files:**
- Modify: `crates/opi-coding-agent/tests/session_extension_state.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/rpc.rs`
- Modify: `crates/opi-coding-agent/tests/extensions.rs`

- [ ] **Step 1: Write failing tests**

Add tests that:
- Resume a session with persisted todo state and call `todo/list` without an intervening prompt; the command must see restored state.
- Resume a session, call `todo/add`, quit without an agent turn, and read the session JSONL; an `ExtensionState` entry must contain the new todo.

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```sh
cargo test -p opi-coding-agent --test session_extension_state command_only
```

Expected: the restore/persist command-only tests fail before the harness change.

- [ ] **Step 3: Implement minimal runtime change**

Change `CodingHarness::dispatch_extension_command` from `&self` to `&mut self`, call `restore_pending_extension_state().await` before dispatch, and call `persist_extension_state().await` after `Ok(Some(_))`. Update RPC to use `self.harness.as_mut()` and update tests that call the method.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```sh
cargo test -p opi-coding-agent --test session_extension_state command_only
cargo test -p opi-coding-agent --test rpc_jsonl rpc_adapter_backed_commands_dispatch_consistently_through_shared_abstraction
```

Expected: all pass.

## Task 3: Package Doctor/List Duplicate Visibility

**Files:**
- Modify: `crates/opi-coding-agent/tests/package_cli.rs`
- Modify: `crates/opi-coding-agent/src/package_cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add tests for two project packages with the same manifest name:
- `package doctor --json` returns a `duplicate_name` diagnostic and exit code `2`.
- `package list --json` prints one surviving package row plus one diagnostic row with `duplicate_name`.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```sh
cargo test -p opi-coding-agent --test package_cli duplicate_name
```

Expected: tests fail because package CLI uses `resolve_declared_installed_packages`.

- [ ] **Step 3: Implement minimal CLI change**

Switch `cmd_list` and `cmd_doctor` to `resolve_installed_packages`.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```sh
cargo test -p opi-coding-agent --test package_cli duplicate_name
```

Expected: tests pass.

## Task 4: Guard Teeth and Protocol Documentation

**Files:**
- Modify: `crates/opi-coding-agent/tests/productized_packages_docs.rs`
- Modify: `crates/opi-coding-agent/src/adapter_protocol.rs`

- [ ] **Step 1: Add guard helper tests**

Add tests that prove positive non-goal claims are rejected:
- `opi package add supports npm sources`
- `opi now bundles Node without external deps`
- `TypeScript extension API compatibility is complete`

- [ ] **Step 2: Run helper tests and verify RED**

Run:

```sh
cargo test -p opi-coding-agent --test productized_packages_docs positive_non_goal_claims_are_rejected_by_helpers
```

Expected: fail under the current helper logic.

- [ ] **Step 3: Tighten helpers and guards**

Remove the broad `without` and `pi` bypasses, scan every `Cargo.toml` for JS/TS runtime crates, expand marketplace/update/runtime needles, and require adapter-specific positive Phase 5 phrases.

- [ ] **Step 4: Correct protocol docs**

Update `adapter_protocol.rs` failure table:
- protocol mismatch diagnostics come from runtime startup, not doctor;
- after-tool hook timeout fails open and continues without a diagnostic claim;
- explicit `AdapterHost::shutdown` is graceful, but ordinary drop is best-effort kill.

- [ ] **Step 5: Run focused guard tests and verify GREEN**

Run:

```sh
cargo test -p opi-coding-agent --test productized_packages_docs
```

Expected: all productized package docs guards pass.

## Task 5: Final Verification

**Files:**
- No new files.

- [ ] **Step 1: Run focused suites**

Run:

```sh
cargo test -p opi-coding-agent --test productized_packages_docs
cargo test -p opi-coding-agent --test session_extension_state
cargo test -p opi-coding-agent --test package_cli
cargo test -p opi-coding-agent --test rpc_jsonl rpc_adapter_backed_commands_dispatch_consistently_through_shared_abstraction
```

- [ ] **Step 2: Run required lint gate**

Run:

```sh
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: exit 0.

- [ ] **Step 3: Do not commit unless requested**

This repository's `AGENTS.md` says never commit unless the user asks. Leave changes unstaged unless explicitly asked to stage or commit.
