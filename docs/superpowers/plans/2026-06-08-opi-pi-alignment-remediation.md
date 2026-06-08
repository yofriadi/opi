# Opi Pi Alignment Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair the concrete drift between `opi` and `pi@0.75.3` while preserving the Rust-native crate architecture and keeping workflow-heavy features outside the core runtime.

**Architecture:** Treat this as a sequence of independently verifiable workstreams, not one large refactor. The target is semantic and product-path alignment with `pi`, not TypeScript package/API compatibility. Keep provider, session, extension, package, TUI, and web-ui decisions behind explicit Rust interfaces.

**Tech Stack:** Rust 2024, Cargo workspace, Tokio, serde/serde_json, TOML config, ratatui/crossterm, schemars/jsonschema, existing `opi-ai`, `opi-agent`, `opi-coding-agent`, `opi-tui`, and unpublished `opi-web-ui` crates.

---

## Scope Check

This remediation spans multiple subsystems. Execute it as separate slices in the order below. Do not merge workflow-heavy features such as MCP, sub-agents, plan mode, todos, or permission popups into core; they remain extension/package examples unless a separate product decision changes the scope.

The work has three alignment targets:

1. Documentation truth: version, phase status, and `pi` relationship must match the implemented code.
2. Product parity path: session tree, extension/package execution, provider/profile breadth, and RPC/SDK integration must have working user paths.
3. Rust architecture quality: crate boundaries stay Rust-native and internal modules become deeper where the current files are too broad.

## Current Evidence

- `Cargo.toml` and `CHANGELOG.md` identify the workspace as `0.5.0`.
- `README.md`, `README.zh.md`, `AGENTS.md`, `CLAUDE.md`, `docs/opi-spec.md`, and `docs/opi-spec.zh.md` still describe the current workspace as `0.4.0`.
- `opi-coding-agent::interactive` has `/model`, `/session`, and `/branch`, but not the fuller `pi` session tree command surface.
- `opi-coding-agent::package_discovery` discovers and composes resources, but does not yet provide a full package manager workflow like `pi`.
- `opi-agent::extension` supports hooks, tools, commands, state, providers, and message preparation, but does not represent dynamic TypeScript-style extension loading. This is an intentional Rust-native difference that needs a process/RPC execution story.
- `opi-web-ui` is an unpublished event/state/rendering crate, not the browser component product shipped by `pi-web-ui`.

## File Boundary Map

### Documentation and roadmap truth

- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `CHANGELOG.md`
- Modify: `.opi-impl-state.json`
- Modify: `docs/snapshots/phase4/opi-impl-state.json`
- Create: `docs/pi-alignment-matrix.md`
- Create localized counterpart only if the matrix becomes user-facing documentation: `docs/pi-alignment-matrix.zh.md`

### Session tree and command parity

- Modify: `crates/opi-agent/src/session.rs`
- Modify: `crates/opi-agent/src/session_branch.rs`
- Modify: `crates/opi-coding-agent/src/session_cli.rs`
- Modify: `crates/opi-coding-agent/src/session_coordinator.rs`
- Modify: `crates/opi-coding-agent/src/interactive.rs`
- Modify: `crates/opi-tui/src/branch_picker.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Test: `crates/opi-agent/tests/session_branching.rs`
- Test: `crates/opi-agent/tests/session_storage.rs`
- Test: `crates/opi-coding-agent/tests/session_cli.rs`
- Test: `crates/opi-coding-agent/tests/session_runtime.rs`
- Test: `crates/opi-tui/tests/session_branching_snapshots.rs`

### Extension, package, and RPC execution path

- Modify: `crates/opi-agent/src/extension.rs`
- Modify: `crates/opi-agent/src/sdk.rs`
- Modify: `crates/opi-agent/src/streaming_proxy.rs`
- Modify: `crates/opi-coding-agent/src/rpc.rs`
- Modify: `crates/opi-coding-agent/src/resource.rs`
- Modify: `crates/opi-coding-agent/src/package_discovery.rs`
- Modify: `crates/opi-coding-agent/src/config.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `examples/*/package.toml`
- Test: `crates/opi-agent/tests/extensions.rs`
- Test: `crates/opi-coding-agent/tests/extensions.rs`
- Test: `crates/opi-coding-agent/tests/extension_resources.rs`
- Test: `crates/opi-coding-agent/tests/package_discovery.rs`
- Test: `crates/opi-coding-agent/tests/harness_resource_integration.rs`
- Test: `crates/opi-coding-agent/tests/rpc_jsonl.rs`

### Provider and model registry breadth

- Modify: `crates/opi-ai/src/registry.rs`
- Modify: `crates/opi-ai/src/provider.rs`
- Modify: `crates/opi-ai/src/lib.rs`
- Modify: `crates/opi-coding-agent/src/config.rs`
- Modify: `crates/opi-coding-agent/src/provider_factory.rs`
- Modify: `crates/opi-coding-agent/src/model_listing.rs`
- Test: `crates/opi-ai/tests/custom_provider_registration.rs`
- Test: `crates/opi-ai/tests/provider_trait.rs`
- Test: `crates/opi-coding-agent/tests/custom_provider_registration.rs`
- Test: `crates/opi-coding-agent/tests/provider_factory.rs`

### Web-ui scope and state fidelity

- Modify: `crates/opi-web-ui/src/lib.rs`
- Modify: `crates/opi-web-ui/src/event.rs`
- Modify: `crates/opi-web-ui/src/state.rs`
- Modify: `crates/opi-web-ui/src/render.rs`
- Modify: `crates/opi-web-ui/Cargo.toml`
- Test: `crates/opi-web-ui/tests/web_ui.rs`
- Test: `crates/opi-coding-agent/tests/web_ui_rpc.rs`

### Rust architecture hardening

- Modify: `crates/opi-agent/src/lib.rs`
- Create: `crates/opi-agent/src/agent_loop.rs`
- Modify only if needed: `crates/opi-agent/src/agent.rs`
- Modify only if needed: `crates/opi-agent/src/loop_types.rs`
- Test: `crates/opi-agent/tests/session_contract.rs`
- Test: `crates/opi-agent/tests/extensions.rs`

## Execution Order

### Task 1: Fix version and phase truth before feature work

**Files:**
- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `.opi-impl-state.json`
- Modify: `docs/snapshots/phase4/opi-impl-state.json`

- [x] **Step 1: Confirm the single source of version truth**

Run:

```powershell
rg -n 'version = "0\.5\.0"|0\.4\.0|0\.5\.0' Cargo.toml CHANGELOG.md README.md README.zh.md AGENTS.md CLAUDE.md docs/opi-spec.md docs/opi-spec.zh.md .opi-impl-state.json docs/snapshots/phase4/opi-impl-state.json
```

Expected: `Cargo.toml` and `CHANGELOG.md` establish `0.5.0`; docs still contain stale `0.4.0` references.

- [x] **Step 2: Update docs to describe the current state as `0.5.0`**

Change stale current-version prose from `0.4.0` to `0.5.0`. Preserve historical release rows that correctly refer to released `0.4.0` Phase 3 work.

- [x] **Step 3: Separate historical releases from current workspace status**

In `docs/opi-spec.md` and `docs/opi-spec.zh.md`, keep the release table historically accurate and add or update a current-workspace sentence that says Phase 4 substrate is implemented in `0.5.0`.

- [x] **Step 4: Verify no stale current-version claim remains**

Run:

```powershell
rg -n 'Current workspace version: `0\.4\.0`|当前 workspace 版本：`0\.4\.0`|current `0\.4\.0` workspace|当前 `0\.4\.0` workspace|v0\.4\.0 ships' README.md README.zh.md AGENTS.md CLAUDE.md docs/opi-spec.md docs/opi-spec.zh.md
```

Expected: no output.

- [x] **Step 5: Run documentation-safe formatting gate**

Run:

```powershell
cargo fmt --check --all
```

Expected: pass. This should pass because the task does not change Rust code.

### Task 2: Add a durable `pi` alignment matrix

**Files:**
- Create: `docs/pi-alignment-matrix.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`

- [x] **Step 1: Create the matrix document**

Create `docs/pi-alignment-matrix.md` with these sections:

```markdown
# pi Alignment Matrix

## Scope

This document compares `opi` against `.repo/pi-0.75.3` by semantic behavior and product workflow. It is not a TypeScript API compatibility checklist.

## Alignment Levels

| Level | Meaning |
|---|---|
| Full | Implemented with equivalent user-visible or library-visible behavior. |
| Partial | Implemented as a substrate or narrower Rust-native equivalent. |
| Deliberate Divergence | Intentionally different because Rust architecture or project scope differs. |
| Missing | Present in `pi` and in scope for `opi`, but not implemented. |
| Out of Scope | Present in `pi`, but excluded from the current `opi` scope. |
```

- [x] **Step 2: Add package rows**

Include rows for `pi-ai`, `pi-agent-core`, `pi-coding-agent`, `pi-tui`, and `pi-web-ui`. For each row, list the matching `opi` crate, current level, and next action.

- [x] **Step 3: Add phase rows**

Include rows for Phase 1 through Phase 4 grouped by crate. Every feature family from the phase summaries must appear at least once: providers, tools, agent loop, TUI, sessions, compaction, JSON/RPC, images, provider registry, resources, skills, prompt fragments, themes, packages, extensions, branch selection, streaming proxy, and web-ui.

- [x] **Step 4: Link the matrix from the spec**

Add a short reference in `docs/opi-spec.md` and `docs/opi-spec.zh.md` pointing to the matrix as the maintained drift ledger.

- [x] **Step 5: Verify the matrix covers every crate**

Run:

```powershell
rg -n 'opi-ai|opi-agent|opi-coding-agent|opi-tui|opi-web-ui|pi-ai|pi-agent-core|pi-coding-agent|pi-tui|pi-web-ui' docs/pi-alignment-matrix.md
```

Expected: every listed package and crate appears.

### Task 3: Bring session tree behavior closer to `pi`

**Files:**
- Modify: `crates/opi-agent/src/session_branch.rs`
- Modify: `crates/opi-coding-agent/src/session_cli.rs`
- Modify: `crates/opi-coding-agent/src/interactive.rs`
- Modify: `crates/opi-tui/src/branch_picker.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Test: `crates/opi-agent/tests/session_branching.rs`
- Test: `crates/opi-coding-agent/tests/session_cli.rs`
- Test: `crates/opi-tui/tests/session_branching_snapshots.rs`

- [x] **Step 1: Define the minimal parity command set**

Implement or expose these user paths first: `/tree`, `/fork`, `/clone`, `--fork`, and branch resume by tip id. Keep `/branch` as a compatibility alias for branch selection.

- [x] **Step 2: Add tests before implementation**

Add tests for:

- `/tree` displays branch tips and active branch marker.
- `/fork` creates a new branch tip without mutating prior entries.
- `/clone` creates a new session with `parent_session` set.
- `--fork <SESSION_ID>` starts from an existing session and writes a new session file.
- Branch reconstruction still uses the last `Leaf` pointer.

Progress note: `--fork`, `/tree`, `/fork`, and `/clone` have tested user paths for new parented sessions. Runtime session entries now also carry meaningful `parent_id` links, turn/compaction completion writes `Leaf` pointers, and continuing from a selected active branch appends new messages under that branch tip in the same JSONL session.

- [x] **Step 3: Implement only the tested session operations**

Use existing append-only JSONL semantics. Do not rewrite existing session files except for explicit delete commands that already exist.

- [x] **Step 4: Update TUI snapshot coverage**

Run:

```powershell
cargo test -p opi-tui session_branching_snapshots
```

Expected: snapshots either pass or produce intentional updates for the new tree labels. Review snapshot diffs before accepting.

- [x] **Step 5: Run session tests**

Run:

```powershell
cargo test -p opi-agent session_branching
cargo test -p opi-coding-agent session_cli
cargo test -p opi-coding-agent session_runtime
```

Expected: pass.

### Task 4: Turn packages and extensions from discovery into an executable composition path

**Files:**
- Modify: `crates/opi-agent/src/extension.rs`
- Modify: `crates/opi-agent/src/sdk.rs`
- Modify: `crates/opi-coding-agent/src/rpc.rs`
- Modify: `crates/opi-coding-agent/src/resource.rs`
- Modify: `crates/opi-coding-agent/src/package_discovery.rs`
- Modify: `crates/opi-coding-agent/src/config.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Test: `crates/opi-agent/tests/extensions.rs`
- Test: `crates/opi-coding-agent/tests/extensions.rs`
- Test: `crates/opi-coding-agent/tests/extension_resources.rs`
- Test: `crates/opi-coding-agent/tests/package_discovery.rs`
- Test: `crates/opi-coding-agent/tests/harness_resource_integration.rs`
- Test: `crates/opi-coding-agent/tests/rpc_jsonl.rs`

- [x] **Step 1: Choose process/RPC as the default plugin execution strategy**

Document in `docs/opi-spec.md` that Rust `opi` will not load arbitrary dynamic Rust libraries by default. Extension execution should flow through in-process registered Rust extensions for embedders and process/RPC adapters for external packages.

- [x] **Step 2: Add tests that distinguish metadata discovery from executable registration**

Tests must prove:

- package discovery lists packages without loading code.
- a package can contribute skills, fragments, and themes through composed resource layers.
- an extension command can be invoked through the registry.
- an RPC command can dispatch to extension commands with correlated response ids.

- [x] **Step 3: Wire resource layers into harness construction**

Ensure config/project/user/explicit package and extension layers affect the harness used by interactive, non-interactive, and RPC modes consistently.

- [x] **Step 4: Keep workflow-heavy examples outside core**

Examples for MCP, plan mode, sub-agent, todo, and permission gate must remain examples or packages. They must not introduce built-in commands unless the command is routed through extension/package registration.

- [x] **Step 5: Run extension and package tests**

Run:

```powershell
cargo test -p opi-agent extensions
cargo test -p opi-coding-agent extensions
cargo test -p opi-coding-agent extension_resources
cargo test -p opi-coding-agent package_discovery
cargo test -p opi-coding-agent harness_resource_integration
cargo test -p opi-coding-agent rpc_jsonl
```

Expected: pass.

Progress note: SDK/RPC now has an `extension_command` command with correlated response IDs, `CodingHarness` can dispatch commands through an injected `ExtensionRegistry`, and RPC tests prove handled/unhandled extension command behavior. Existing package/resource tests continue to prove discovery and composed resource layers are metadata/resource paths rather than implicit code loading. External process/RPC package adapters remain a productization slice, not a core dynamic-loading requirement.

### Task 5: Expand provider breadth through profiles, not copied provider modules

**Files:**
- Modify: `crates/opi-ai/src/registry.rs`
- Modify: `crates/opi-ai/src/provider.rs`
- Modify: `crates/opi-coding-agent/src/config.rs`
- Modify: `crates/opi-coding-agent/src/provider_factory.rs`
- Modify: `crates/opi-coding-agent/src/model_listing.rs`
- Test: `crates/opi-ai/tests/custom_provider_registration.rs`
- Test: `crates/opi-coding-agent/tests/provider_factory.rs`
- Test: `crates/opi-coding-agent/tests/custom_provider_registration.rs`

- [x] **Step 1: Define provider expansion policy**

Add spec text that provider breadth should primarily arrive through:

- built-in first-class providers only when wire format or auth is materially different.
- OpenAI-compatible profiles when provider behavior can be configured.
- custom provider registration for embedders and extension adapters.

- [x] **Step 2: Add config tests for OpenAI-compatible profiles**

Tests must cover a profile with provider id, base URL, API key env var, model list, model capabilities, and optional compatibility flags.

- [x] **Step 3: Route model listing through the registry**

`--list-models` should enumerate built-in providers plus configured profiles and registered custom model overrides.

- [x] **Step 4: Keep OAuth as an explicit decision**

Do not silently add OAuth flows in this task. Add a documented decision point listing Anthropic OAuth, OpenAI Codex OAuth, and GitHub Copilot OAuth as separate future slices because they introduce credential storage and login commands.

- [x] **Step 5: Run provider tests**

Run:

```powershell
cargo test -p opi-ai custom_provider_registration
cargo test -p opi-coding-agent provider_factory
cargo test -p opi-coding-agent custom_provider_registration
```

Expected: pass.

Progress note: Config now supports `[providers.openai_compatible.<id>]` profiles with base URL, API key env var, model metadata, proxy config, and compatibility flags. Configured profiles build through the existing OpenAI-compatible adapter, participate in runtime provider construction, and appear in registry-backed `--list-models` output. OAuth provider flows remain documented as separate future product decisions.

### Task 6: Make `opi-web-ui` claims match the actual crate

**Files:**
- Modify: `crates/opi-web-ui/src/event.rs`
- Modify: `crates/opi-web-ui/src/state.rs`
- Modify: `crates/opi-web-ui/src/render.rs`
- Modify: `crates/opi-web-ui/src/lib.rs`
- Modify: `crates/opi-web-ui/Cargo.toml`
- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Test: `crates/opi-web-ui/tests/web_ui.rs`
- Test: `crates/opi-coding-agent/tests/web_ui_rpc.rs`

- [x] **Step 1: Pick the web-ui product stance**

Use this stance for Phase 4 remediation: `opi-web-ui` is an unpublished Rust event/state/rendering crate for consumers of RPC/SDK events. It is not a standalone browser app and not equivalent to `pi-web-ui`.

- [x] **Step 2: Preserve RPC response data in state**

Ensure response payloads such as session info, model info, resources, and compaction status can update `ConversationState`.

- [x] **Step 3: Add render safety tests**

Tests must cover text escaping, tool-call rendering, session/model state updates, and unknown event tolerance.

- [x] **Step 4: Run web-ui tests**

Run:

```powershell
cargo test -p opi-web-ui
cargo test -p opi-coding-agent web_ui_rpc
```

Expected: pass.

Progress note: `opi-web-ui` remains an unpublished Rust event/state/rendering crate, not a standalone browser app. `ConversationState` now exposes resource metadata from `session_info` responses and the last successful compaction response payload. The `opi-web-ui` test suite covers parsing, unknown event tolerance, response data preservation, text escaping, tool-call rendering, session/model/resource state, compaction state, and SDK event round trips. The planned `crates/opi-coding-agent/tests/web_ui_rpc.rs` target does not exist; keeping the coverage inside `opi-web-ui` avoids introducing a reverse test dependency from `opi-coding-agent` to the unpublished web crate.

### Task 7: Deepen `opi-agent` internals without changing crate boundaries

**Files:**
- Modify: `crates/opi-agent/src/lib.rs`
- Create: `crates/opi-agent/src/agent_loop.rs`
- Modify: `crates/opi-agent/src/loop_types.rs`
- Test: `crates/opi-agent/tests/session_contract.rs`
- Test: `crates/opi-agent/tests/extensions.rs`

- [x] **Step 1: Move the loop implementation into `agent_loop.rs`**

Keep the public `opi_agent::agent_loop` export stable by re-exporting the function from `lib.rs`.

- [x] **Step 2: Keep crate boundaries unchanged**

Do not introduce a shared types crate. The current crate split remains correct: provider abstractions in `opi-ai`, generic runtime in `opi-agent`, CLI/harness/tools in `opi-coding-agent`, TUI widgets in `opi-tui`, and unpublished web-facing state in `opi-web-ui`.

- [x] **Step 3: Run agent tests**

Run:

```powershell
cargo test -p opi-agent
```

Expected: pass.

Progress note: `opi-agent` now keeps `lib.rs` as a public module/re-export surface while the core `agent_loop` implementation and private helpers live in `crates/opi-agent/src/agent_loop.rs`. The public `opi_agent::agent_loop` export remains stable, no shared types crate was introduced, and `cargo test -p opi-agent` passes.

### Task 8: Tighten TUI scope around coding-agent needs

**Files:**
- Modify: `crates/opi-tui/src/branch_picker.rs`
- Modify: `crates/opi-tui/src/lib.rs`
- Modify only if required by command UX: `crates/opi-tui/src/select_list.rs`
- Test: `crates/opi-tui/tests/session_branching_snapshots.rs`
- Test: `crates/opi-tui/tests/theme_snapshots.rs`

- [x] **Step 1: Avoid copying `pi-tui` wholesale**

Only add TUI primitives needed by the current coding-agent flows: branch tree display, selection overlays, model/session pickers, markdown/code/diff views, image rendering, theme, and keybindings.

- [x] **Step 2: Add display-width coverage where labels can contain non-ASCII text**

Snapshot tests must include branch/session/model labels with CJK-width characters so truncation and selection markers stay aligned.

- [x] **Step 3: Run TUI tests**

Run:

```powershell
cargo test -p opi-tui
```

Expected: pass.

Progress note: `opi-tui` stayed scoped to coding-agent widgets. `SelectList` now uses `unicode-width`, selected row marker width is included in both `SelectList` and `BranchPicker` column layout, and branch/session snapshot tests include CJK-width labels and metadata. `cargo test -p opi-tui` passes.

### Task 9: Final gates and release-readiness check

**Files:**
- Modify if code changed: `CHANGELOG.md`
- Modify if docs changed: localized counterpart docs for every updated localized source

- [x] **Step 1: Run formatting**

Run:

```powershell
cargo fmt --all
cargo fmt --check --all
```

Expected: pass.

- [x] **Step 2: Run clippy**

Run:

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: pass.

- [x] **Step 3: Run all tests**

Run:

```powershell
cargo test --workspace --all-targets
```

Expected: pass.

- [x] **Step 4: Run docs with warnings denied**

Run:

```powershell
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps
```

Expected: pass.

- [x] **Step 5: Run smoke commands**

Run:

```powershell
cargo run -p opi-coding-agent -- --version
cargo run -p opi-coding-agent -- --help
cargo run -p opi-coding-agent -- --list-models --json
cargo run -p opi-coding-agent -- --generate-completion powershell
```

Expected: all commands exit successfully without requiring provider API keys.

Progress note: Final verification passed with `cargo fmt --check --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, `cargo run -p opi-coding-agent -- --version`, `--help`, `--list-models --json`, and `--generate-completion powershell`. Updating `docs/opi-spec.md` required refreshing the Phase 4 ledger hash in `docs/snapshots/phase4/opi-impl-state.json`.

## Non-Goals

- Do not make `opi` API-compatible with `pi`.
- Do not make `opi` read `pi` sessions or config by default.
- Do not add TypeScript extension loading to the Rust binary.
- Do not turn MCP, sub-agents, plan mode, todos, or permission popups into built-in core features.
- Do not publish `opi-web-ui` as a browser app until a separate web product plan exists.
- Do not add a shared internal types crate unless two crates require the same owned type and the dependency direction cannot stay clean.

## Review Checklist

- Every stale current-version `0.4.0` claim is removed or reclassified as historical.
- `docs/pi-alignment-matrix.md` shows package and phase alignment by feature family.
- Session tree commands have tests before implementation.
- Package/resource discovery is connected to an executable extension path.
- Provider breadth expands through registry/profile configuration.
- `opi-web-ui` documentation matches its unpublished Rust crate scope.
- `opi-agent` internals become deeper without changing the public crate boundary.
- Localized documentation is synchronized when English docs change.
- `cargo fmt --check --all`, clippy, tests, docs, and smoke commands pass.
