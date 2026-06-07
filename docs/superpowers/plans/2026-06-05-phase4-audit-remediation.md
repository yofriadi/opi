# Phase 4 Audit Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the verified Phase 4 audit blockers so the phase can be honestly marked complete, or explicitly narrow the documented scope where the current architecture cannot support a product-level claim.

**Architecture:** Treat Phase 4 as two surfaces: product runtime paths for the `opi` binary, and SDK/library embedding paths for Rust consumers. Fix false-success RPC commands first, then connect extension/discovery/provider/web-ui surfaces through explicit builders and config; write `phase_exit.4` only after tests and docs match the implemented behavior.

**Tech Stack:** Rust 2024, Tokio, serde/serde_json, TOML config, ratatui TUI widgets, existing opi-agent/opi-ai registries, PowerShell verification on Windows.

---

## Verification Summary

Confirmed blockers:

- `RpcRunner::run` reads stdin and awaits `harness.prompt()` / `harness.continue_()` in the same loop, so mid-turn `abort`, `steer`, and `follow_up` are not processed until the turn finishes.
- RPC events are drained before the next command and after the turn, not continuously during provider/tool streaming.
- `run_rpc()` accepts `_tool_selection` and drops it; `RpcRunner::new()` always resolves `ToolSelection::Default`.
- `set_model` returns success for cross-provider changes without rebuilding or validating the provider. Manual reproduction accepted `openai:gpt-4o` while running with an Anthropic provider.
- `set_thinking_level` acknowledges success while the code comment says runtime config updates are not implemented.
- `ExtensionRegistry`, resource/package/skill/fragment/theme discovery, and `ProviderRegistry::all_models()` exist but are not wired into `main.rs`, `harness.rs`, or config-driven production paths.
- `Extension` module docs mention custom agent messages, but the trait has no message injection API and `CompositeHooks::prepare_next_turn()` delegates only to the base hook.
- Four example packages (`sub-agent`, `plan-mode`, `todo`, `mcp-adapter`) use `[package]` schema that `PackageManifest::from_toml()` cannot parse.
- `BranchPicker` exists in `opi-tui` but is not reachable from `opi-coding-agent` interactive mode.
- `opi-web-ui` drops `RpcResponse.data`, so real `session_info` / model data does not update UI state.
- `web_ui_rpc` subprocess tests pass on Windows by skipping: `target/debug/opi.exe` exists, but the test looks for `target/debug/opi`.
- `docs/snapshots/phase4/opi-impl-state.json` has `phase_exit` entries for 1-3 only.
- Root docs and localized docs still contain stale placeholder / Phase 3 descriptions.

Downgraded or corrected audit wording:

- `StreamingProxy::run` is truly an `async fn` around blocking `BufRead::read_line`, but it is not currently wired to a production async transport. Fix it in the hardening pass unless the RPC refactor reuses it directly.
- The example package schema issue affects four examples, not `permission-gate` or `protected-paths`; those two use the flat schema expected by the parser.
- `opi-web-ui` is a Rust component/state/rendering crate, not a browser app. The remediation should either add an app shell or update docs to claim only reusable web-facing components.

## File Structure

- Modify: `crates/opi-coding-agent/src/rpc.rs`
  - Replace the single blocking command loop with a command/event/run coordinator.
  - Accept `ToolSelection`.
  - Return honest errors for unsupported or invalid runtime changes.
- Modify: `crates/opi-coding-agent/src/main.rs`
  - Pass `tool_selection` into RPC.
  - Centralize provider/model registry construction for `build_provider`, `list_models`, and runtime model validation.
- Modify: `crates/opi-coding-agent/src/harness.rs`
  - Expose safe runtime controls needed by RPC without holding a mutable harness lock for an entire turn.
  - Add explicit methods for model validation/switching and thinking config updates.
- Modify: `crates/opi-agent/src/agent.rs`
  - Add a clonable control handle for cancel/steer/follow-up and thinking config mutation, or expose equivalent queue/cancel operations safely.
- Modify: `crates/opi-agent/src/extension.rs`
  - Add custom-message injection through `prepare_next_turn` or an explicit extension method.
- Modify: `crates/opi-coding-agent/src/config.rs`
  - Add explicit `[extensions]` and `[packages]` config sections for declarative resource loading.
- Modify: `crates/opi-coding-agent/src/resource.rs`
  - Align same-layer duplicate behavior with docs and remove unsafe canonicalize fallback.
- Modify: `crates/opi-coding-agent/src/package_discovery.rs`
  - Remove unsafe canonicalize fallback and expose package-to-resource layer composition.
- Modify: `crates/opi-coding-agent/src/skill.rs`
  - Consume discovered skill layers from config/package paths.
- Modify: `crates/opi-coding-agent/src/prompt_fragment.rs`
  - Expose fragments as slash-style prompt commands and RPC metadata.
- Modify: `crates/opi-coding-agent/src/theme_discovery.rs`
  - Feed discovered themes into interactive theme resolution.
- Modify: `crates/opi-ai/src/registry.rs`
  - Add public error stability annotations if kept public.
- Modify: `crates/opi-ai/src/lib.rs`
  - Re-export registry errors if public consumers are expected to handle them.
- Modify: `crates/opi-agent/src/sdk.rs`
  - Add `Deserialize` to `SdkResponse` and replace the misleading serialization fallback type.
- Modify: `crates/opi-agent/src/streaming_proxy.rs`
  - Make I/O semantics honest: either synchronous `run` or real async I/O.
- Modify: `crates/opi-tui/src/branch_picker.rs`
  - Use real display-width handling.
- Modify: `crates/opi-coding-agent/src/interactive.rs`
  - Add a reachable branch-selection workflow.
- Modify: `crates/opi-web-ui/src/event.rs`
  - Preserve `RpcResponse.data` and map response payloads into typed UI events.
- Modify: `crates/opi-web-ui/src/state.rs`
  - Update model/session/compaction state from real RPC responses.
- Modify: `crates/opi-web-ui/Cargo.toml`
  - Move `opi-agent` to dev-dependencies if it remains test-only; remove or use `opi-ai`.
- Modify: `crates/opi-coding-agent/tests/rpc_jsonl.rs`
  - Fix binary path resolution and add real RPC behavior tests.
- Modify: `crates/opi-coding-agent/tests/web_ui_rpc.rs`
  - Fix binary path resolution and assert state changes from real RPC data.
- Modify: `crates/opi-web-ui/tests/web_ui.rs`
  - Cover response data, tool-call mapping, and XSS-sensitive rendering paths.
- Modify: `examples/sub-agent/package.toml`
- Modify: `examples/plan-mode/package.toml`
- Modify: `examples/todo/package.toml`
- Modify: `examples/mcp-adapter/package.toml`
  - Convert to the parser-supported flat package schema.
- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `CHANGELOG.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
  - Synchronize Phase 4 reality and localized counterparts.
- Modify last: `docs/snapshots/phase4/opi-impl-state.json`
  - Add `phase_exit.4` only after remediation gates pass.

### Task 1: Lock In the Current Failures

**Files:**
- Modify: `crates/opi-coding-agent/tests/web_ui_rpc.rs`
- Modify: `crates/opi-coding-agent/tests/rpc_jsonl.rs`
- Modify: `crates/opi-web-ui/tests/web_ui.rs`
- Modify: `crates/opi-agent/tests/extensions.rs`
- Modify: `crates/opi-coding-agent/tests/package_discovery.rs`

- [ ] **Step 1: Fix subprocess binary lookup in RPC tests**

Replace hand-built binary paths with `CARGO_BIN_EXE_opi`, falling back to the old path only for local manual runs:

```rust
fn opi_binary_path() -> std::path::PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_opi") {
        return std::path::PathBuf::from(path);
    }
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let mut path = std::path::PathBuf::from(manifest_dir);
    path.push("../../target/debug/opi");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}
```

Run:

```powershell
cargo test -p opi-coding-agent --test web_ui_rpc web_ui_conversation_renders_from_rpc_output -- --nocapture
```

Expected before later Web UI fixes: the test no longer prints a skip line; it fails because status-bar data is not populated from `RpcResponse.data`.

- [ ] **Step 2: Add a regression test for Web UI response data**

Add this test to `crates/opi-web-ui/tests/web_ui.rs`:

```rust
#[test]
fn rpc_session_info_response_updates_status_state() {
    let raw = serde_json::json!({
        "type": "response",
        "command": "session_info",
        "success": true,
        "id": "si-1",
        "data": {
            "model": "anthropic:claude-sonnet-4",
            "session_id": "abc123",
            "turn_count": 2,
            "message_count": 5
        }
    });
    let event = WebUiEvent::parse(&raw).unwrap();
    let mut state = ConversationState::new();
    state.process(event);
    assert_eq!(state.model(), Some("anthropic:claude-sonnet-4"));
    assert_eq!(state.session_id(), Some("abc123"));
    assert_eq!(state.turn_count(), 2);
}
```

Run:

```powershell
cargo test -p opi-web-ui rpc_session_info_response_updates_status_state -- --nocapture
```

Expected before the fix: FAIL because `RpcResponse.data` is discarded.

- [ ] **Step 3: Add a regression test for `set_model` false success**

Add an RPC unit or subprocess test that sends:

```json
{"type":"set_model","id":"set","model":"openai:gpt-4o"}
```

while the process was started with:

```powershell
.\target\debug\opi.exe --rpc --model anthropic:claude-sonnet-4
```

Expected after the fix:

```json
{"type":"response","command":"set_model","id":"set","success":false,"error":"cannot switch provider from anthropic to openai at runtime"}
```

- [ ] **Step 4: Add a regression test for RPC tool selection**

Create a test that starts RPC with `--no-tools`, sends a prompt to a mockable in-process `RpcRunner` fixture, and asserts the system prompt has no built-in tool definitions. If a subprocess mock provider is unavailable, add a constructor-level unit test around `RpcRunner::new` that inspects `harness.system_prompt()` after tool config resolution.

Expected before the fix: FAIL because `ToolSelection::Default` is always used.

- [ ] **Step 5: Add a regression test for extension custom messages**

Add an extension in `crates/opi-agent/tests/extensions.rs` whose `prepare_next_turn` injects `AgentMessage::Custom`, then assert the composite hook passes the update through.

Expected before the fix: FAIL because `CompositeHooks::prepare_next_turn()` only calls the base hook.

- [ ] **Step 6: Add package schema tests for the four broken examples**

Add a parameterized test in `crates/opi-coding-agent/tests/package_discovery.rs` that parses these files with `PackageManifest::from_toml()`:

```text
examples/sub-agent/package.toml
examples/plan-mode/package.toml
examples/todo/package.toml
examples/mcp-adapter/package.toml
```

Expected before the fix: FAIL with missing top-level `name`.

### Task 2: Repair RPC Control Semantics

**Files:**
- Modify: `crates/opi-agent/src/agent.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/rpc.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Test: `crates/opi-coding-agent/tests/rpc_jsonl.rs`

- [ ] **Step 1: Add an agent control handle**

Add a clonable handle in `crates/opi-agent/src/agent.rs` so RPC can cancel and queue messages while a turn owns `&mut Agent`:

```rust
#[derive(Clone)]
pub struct AgentControl {
    cancel: CancellationToken,
    steering_queue: Arc<Mutex<VecDeque<String>>>,
    follow_up_queue: Arc<Mutex<VecDeque<String>>>,
}

impl AgentControl {
    pub fn abort(&self) {
        self.cancel.cancel();
    }

    pub fn steer(&self, message: String) {
        self.steering_queue.lock().unwrap().push_back(message);
    }

    pub fn follow_up(&self, message: String) {
        self.follow_up_queue.lock().unwrap().push_back(message);
    }
}
```

Add:

```rust
pub fn control_handle(&self) -> AgentControl {
    AgentControl {
        cancel: self.cancel.clone(),
        steering_queue: self.steering_queue.clone(),
        follow_up_queue: self.follow_up_queue.clone(),
    }
}
```

- [ ] **Step 2: Expose the handle through `CodingHarness`**

Add:

```rust
pub fn control_handle(&self) -> opi_agent::agent::AgentControl {
    self.agent.control_handle()
}
```

Use this for `RpcRunner` runtime commands rather than taking a harness lock.

- [ ] **Step 3: Pass tool selection into RPC**

Change `run_rpc` and `RpcRunner::new` signatures to accept `tool_selection: ToolSelection`, then resolve:

```rust
let tool_config = crate::policy::ToolRuntimeConfig::resolve(
    RunMode::NonInteractive,
    allow_mutating,
    tool_selection,
)?;
```

Run:

```powershell
cargo test -p opi-coding-agent --test rpc_jsonl rpc_tool_selection_respects_no_tools -- --nocapture
```

Expected after the fix: PASS.

- [ ] **Step 4: Refactor the RPC loop around concurrent run/event/control paths**

Use one agent-run task at a time, a continuous event receiver drain, and a command loop that can accept control commands while the run task is active. The run task owns the mutable harness for `prompt` / `continue`; `abort`, `steer`, and `follow_up` use `AgentControl`.

Minimum behavior:

```text
prompt/continue while idle:
  write success response
  spawn run task

prompt/continue while running:
  write error response

abort while running:
  control.abort()
  write success response immediately

steer/follow_up while running:
  control.steer(...) or control.follow_up(...)
  write success response immediately

event received:
  write event immediately

run task completed:
  mark idle
  drain remaining events
```

Do not hold a `tokio::sync::Mutex<CodingHarness>` across an entire turn if that prevents control commands from using queues.

- [ ] **Step 5: Make stdin I/O honest**

Either use `tokio::io::BufReader::new(tokio::io::stdin()).lines()` in production or isolate blocking stdin in `tokio::task::spawn_blocking` that forwards parsed command lines to an async channel. Avoid synchronous `std::io::BufRead::lines()` on the Tokio runtime thread.

- [ ] **Step 6: Verify mid-turn control**

Run targeted tests:

```powershell
cargo test -p opi-coding-agent --test rpc_jsonl rpc_mid_turn_abort_is_processed -- --nocapture
cargo test -p opi-coding-agent --test rpc_jsonl rpc_mid_turn_steer_is_queued -- --nocapture
cargo test -p opi-coding-agent --test rpc_jsonl rpc_events_stream_before_turn_end -- --nocapture
```

Expected after the fix: all pass without external network.

### Task 3: Stop RPC False Successes

**Files:**
- Modify: `crates/opi-agent/src/agent.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/rpc.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Test: `crates/opi-coding-agent/tests/rpc_jsonl.rs`

- [ ] **Step 1: Restrict `set_model` to validated same-provider switches**

Until provider rebuilding is implemented, reject provider-family changes. For same-provider changes, validate against the current provider's `models()` list before updating the model string.

Expected errors:

```text
invalid model spec: expected provider:model
cannot switch provider from anthropic to openai at runtime
unknown model 'bad-model' for provider 'anthropic'
```

- [ ] **Step 2: Return model data on successful `set_model`**

On success, return:

```json
{"type":"response","command":"set_model","success":true,"data":{"model":"anthropic:..."}}
```

This gives `opi-web-ui` a real payload to consume.

- [ ] **Step 3: Implement or reject `set_thinking_level` honestly**

Implement explicit levels if product semantics are desired:

```text
off    -> thinking: None
low    -> enabled, budget_tokens = 2048
medium -> enabled, budget_tokens = current config default
high   -> enabled, budget_tokens = max(current config default, 20000)
```

If these semantics are rejected during review, return an unsupported-command error instead of success. Do not leave a success no-op.

- [ ] **Step 4: Verify command behavior**

Run:

```powershell
cargo test -p opi-coding-agent --test rpc_jsonl rpc_set_model_rejects_cross_provider -- --nocapture
cargo test -p opi-coding-agent --test rpc_jsonl rpc_set_model_returns_model_data -- --nocapture
cargo test -p opi-coding-agent --test rpc_jsonl rpc_set_thinking_level_changes_runtime_config -- --nocapture
```

Expected after the fix: all pass.

### Task 4: Connect Extension and Discovery Surfaces

**Files:**
- Modify: `crates/opi-agent/src/extension.rs`
- Modify: `crates/opi-coding-agent/src/config.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/resource.rs`
- Modify: `crates/opi-coding-agent/src/package_discovery.rs`
- Modify: `crates/opi-coding-agent/src/skill.rs`
- Modify: `crates/opi-coding-agent/src/prompt_fragment.rs`
- Modify: `crates/opi-coding-agent/src/theme_discovery.rs`
- Test: `crates/opi-agent/tests/extensions.rs`
- Test: `crates/opi-coding-agent/tests/extensions.rs`
- Test: `crates/opi-coding-agent/tests/extension_resources.rs`
- Test: `crates/opi-coding-agent/tests/skills_discovery.rs`
- Test: `crates/opi-coding-agent/tests/prompt_fragments.rs`
- Test: `crates/opi-coding-agent/tests/theme_discovery.rs`
- Test: `crates/opi-coding-agent/tests/package_discovery.rs`

- [ ] **Step 1: Add extension custom-message injection**

Prefer extending `Extension` with:

```rust
fn prepare_next_turn(
    &self,
    _ctx: &PrepareNextTurnContext,
) -> Pin<Box<dyn Future<Output = Option<AgentLoopTurnUpdate>> + Send>> {
    Box::pin(async { None })
}
```

Then merge extension updates after the base hook in `CompositeHooks::prepare_next_turn`.

- [ ] **Step 2: Add config sections for declarative resources**

Add resolved config:

```rust
pub struct ExtensionsConfig {
    pub paths: Vec<PathBuf>,
}

pub struct PackagesConfig {
    pub paths: Vec<PathBuf>,
}
```

Add TOML support:

```toml
[extensions]
paths = ["vendor/my-extension"]

[packages]
paths = ["vendor/my-package"]
```

- [ ] **Step 3: Build resource layers from user, project, and explicit config**

Create a helper that returns `Vec<DiscoveryLayer>` for each resource kind using:

```text
user config dir precedence 0
project .opi dir precedence 1
config paths precedence 2
```

Do not silently accept canonicalization failures for paths used in security checks.

- [ ] **Step 4: Compose packages into resource discovery layers**

For each discovered package, call `compose()` and route `ComposedResource` paths into extension/skill/fragment/theme discovery. Keep package metadata lazily available through `PackageRegistry`.

- [ ] **Step 5: Wire discovered resources into harness startup**

At minimum:

- extension manifests are discovered and surfaced in prompt/RPC metadata;
- skills are listed in the system prompt with progressive-discovery summaries;
- prompt fragments are exposed as slash-style commands and RPC metadata;
- themes discovered from config/package paths can be selected through `[defaults].theme`;
- extension hooks/tools/providers from SDK-provided `ExtensionRegistry` can wrap the agent loop through a builder.

Do not claim arbitrary Rust extension code can be loaded by the CLI unless a plugin/process runtime is actually implemented.

- [ ] **Step 6: Add a public builder for SDK embedders**

Expose a builder that accepts:

```rust
ExtensionRegistry
ProviderRegistry
Vec<DiscoveryLayer>
ToolSelection
```

This is the Rust path for third parties that need custom trait-object extensions/providers without patching core crates.

- [ ] **Step 7: Verify runtime reachability**

Run:

```powershell
cargo test -p opi-agent --test extensions -- --nocapture
cargo test -p opi-coding-agent --test extensions -- --nocapture
cargo test -p opi-coding-agent --test extension_resources -- --nocapture
cargo test -p opi-coding-agent --test skills_discovery -- --nocapture
cargo test -p opi-coding-agent --test prompt_fragments -- --nocapture
cargo test -p opi-coding-agent --test theme_discovery -- --nocapture
cargo test -p opi-coding-agent --test package_discovery -- --nocapture
```

Expected after the fix: all pass, with at least one test proving config-loaded resources affect a real harness or TUI/RPC metadata path.

### Task 5: Unify Provider Registry Usage

**Files:**
- Modify: `crates/opi-ai/src/registry.rs`
- Modify: `crates/opi-ai/src/lib.rs`
- Modify: `crates/opi-coding-agent/Cargo.toml`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Add: `crates/opi-coding-agent/src/model_listing.rs`
- Modify: `crates/opi-coding-agent/src/picker.rs`
- Test: `crates/opi-ai/tests/custom_provider_registration.rs`
- Test: `crates/opi-coding-agent/tests/custom_provider_registration.rs`
- Test: `crates/opi-coding-agent/tests/extensions.rs`
- Add: `crates/opi-coding-agent/tests/model_listing.rs`
- Test: `crates/opi-coding-agent/tests/picker_integration.rs`

- [x] **Step 1: Re-export registry error types**

If public callers are expected to recover from registry errors, add:

```rust
pub use registry::{RegistrationError, RegistryError};
```

in `crates/opi-ai/src/lib.rs`.

- [x] **Step 2: Add `#[non_exhaustive]` to public registry errors**

Add:

```rust
#[non_exhaustive]
```

to `RegistryError` and `RegistrationError`.

- [x] **Step 3: Centralize built-in provider registration**

Move the provider construction and model listing logic behind one registry/factory helper in `opi-coding-agent`. `--list-models` should enumerate the same model source that runtime validation uses.

- [x] **Step 4: Verify list-models and runtime use the same source**

Run:

```powershell
cargo test -p opi-ai --test custom_provider_registration -- --nocapture
cargo test -p opi-coding-agent --test custom_provider_registration -- --nocapture
```

Expected after the fix: all pass and include coverage for `--list-models` seeing registry-provided models.

Completed notes:
- `opi-ai` re-exports `ProviderRegistry`, `RegistrationError`, and `RegistryError`; registry errors are `#[non_exhaustive]`.
- `ProviderRegistry::all_models()` now has deterministic override ordering.
- `--list-models` builds a `ProviderRegistry` and renders via `model_entries_from_registry()`.
- `picker::model_picker_items()` consumes `ProviderRegistry::all_models()`.
- `CodingHarness` keeps a model metadata registry for active-provider validation and `/model` items; it includes extension model overrides but filters `/model` to the active provider so users are not shown unsupported cross-provider runtime switches.
- Verified with targeted tests and clippy for `opi-ai` and `opi-coding-agent`.

### Task 6: Make Web UI Consume Real RPC Responses

**Files:**
- Modify: `crates/opi-web-ui/src/event.rs`
- Modify: `crates/opi-web-ui/src/state.rs`
- Modify: `crates/opi-web-ui/src/components.rs`
- Modify: `crates/opi-web-ui/src/render.rs`
- Modify: `crates/opi-web-ui/Cargo.toml`
- Modify: `crates/opi-web-ui/tests/web_ui.rs`
- Modify: `crates/opi-coding-agent/tests/web_ui_rpc.rs`

- [x] **Step 1: Add `data` to RPC response events**

Change `WebUiEvent::RpcResponse` to include:

```rust
data: Option<serde_json::Value>,
```

and preserve `obj.get("data").cloned()`.

- [x] **Step 2: Update state from response payloads**

In `ConversationState::process`, when a successful `RpcResponse` has `command == "session_info"`, read:

```text
data.model
data.session_id
data.turn_count
data.message_count
```

When `command == "set_model"`, read `data.model`.

- [x] **Step 3: Stop mapping tool-call stream fragments to assistant text**

Map `tool_call_start`, `tool_call_delta`, and `tool_call_end` into structured tool-call state or preserve them as typed unknowns. Do not append tool-call JSON deltas to visible assistant text.

- [x] **Step 4: Add XSS coverage for every rendered dynamic surface**

Add tests for message text, thinking content, tool name/result, status bar model/session values, and attribute-like content.

- [x] **Step 5: Clean dependency placement**

If `opi-agent` remains used only by tests, move it to `[dev-dependencies]`. Remove `opi-ai` if still unused by `src`, or use it in typed event conversion.

- [x] **Step 6: Verify Web UI**

Run:

```powershell
cargo test -p opi-web-ui -- --nocapture
cargo test -p opi-coding-agent --test web_ui_rpc -- --nocapture
```

Expected after the fix: no skip lines on Windows; real RPC `session_info` data updates the status bar.

Completed notes:
- `RpcResponse` preserves `data`, stores it in `TrackedResponse`, and successful `session_info` / `set_model` responses update state.
- `ConversationState` now tracks `message_count` from `session_info` data and `AgentEnd`.
- `tool_call_start`, `tool_call_delta`, and `tool_call_end` message updates are preserved as `Unknown` rather than appended to visible assistant text.
- HTML escaping tests cover message text, thinking content, tool names/results, status model/session values, and attribute-like content.
- `opi-agent` moved to `dev-dependencies`; unused `opi-ai` was removed from `opi-web-ui`.
- Verified with `cargo test -p opi-web-ui -- --nocapture`, `cargo test -p opi-coding-agent --test web_ui_rpc -- --nocapture`, `cargo clippy -p opi-web-ui --all-targets -- -D warnings`, and `cargo clippy -p opi-coding-agent --test web_ui_rpc -- -D warnings`.

### Task 7: Make Session Branching Reachable

**Files:**
- Modify: `crates/opi-agent/src/session_branch.rs`
- Modify: `crates/opi-coding-agent/src/picker.rs`
- Modify: `crates/opi-coding-agent/src/session_coordinator.rs`
- Modify: `Cargo.toml`
- Modify: `crates/opi-tui/Cargo.toml`
- Modify: `crates/opi-tui/src/branch_picker.rs`
- Modify: `crates/opi-coding-agent/src/interactive.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Test: `crates/opi-agent/tests/session_branching.rs`
- Test: `crates/opi-tui/tests/session_branching_snapshots.rs`
- Test: `crates/opi-coding-agent/src/interactive.rs`
- Test: `crates/opi-coding-agent/tests/picker_integration.rs`
- Test: `crates/opi-coding-agent/tests/session_runtime.rs`

- [x] **Step 1: Make branch order deterministic**

Sort roots and children before walking the session graph so snapshots and UI order are stable.

- [x] **Step 2: Add conversion from `BranchInfo` to UI items**

Create a helper that maps branch metadata into `BranchItem` or an equivalent picker item.

- [x] **Step 3: Add a reachable TUI workflow**

Add `/branch` for the current session or extend `/session` resume so a branched session opens branch selection. Selecting a branch must resume from the chosen tip, not just the session's last leaf.

- [x] **Step 4: Use real display-width handling**

Use the workspace dependency path for `unicode-width` if available; otherwise add it through `[workspace.dependencies]` first, then `workspace = true` in `opi-tui`.

- [x] **Step 5: Verify branch workflow**

Run:

```powershell
cargo test -p opi-agent --test session_branching -- --nocapture
cargo test -p opi-tui --test session_branching_snapshots -- --nocapture
cargo test -p opi-coding-agent interactive::tests::session_picker_enter_returns_selected_session -- --nocapture
```

Expected after the fix: data-layer tests pass, snapshots are stable, and interactive tests cover branch selection.

Completed notes:
- `SessionTree` now sorts roots and child branches deterministically.
- `picker::branch_picker_items()` maps `SessionTree` branches into selectable UI items and marks the active branch in metadata.
- Interactive TUI supports `/branch`, opens a branch picker for the current session, and switches by appending a Leaf pointer before rebuilding the harness context from the selected branch.
- `BranchPicker` width handling uses `unicode-width`, with CJK coverage.
- Verified with `cargo test -p opi-agent --test session_branching -- --nocapture`, `cargo test -p opi-tui --test session_branching_snapshots -- --nocapture`, targeted interactive/picker/session runtime tests, and targeted clippy for `opi-agent`, `opi-tui`, and `opi-coding-agent`.

### Task 8: Normalize Example Packages

**Files:**
- Modify: `examples/sub-agent/package.toml`
- Modify: `examples/plan-mode/package.toml`
- Modify: `examples/todo/package.toml`
- Modify: `examples/mcp-adapter/package.toml`
- Test: `crates/opi-coding-agent/tests/package_discovery.rs`

- [x] **Step 1: Convert `sub-agent` package schema**

Use:

```toml
name = "sub-agent"
description = "Example package demonstrating a sub-agent extension that runs nested agent workflows"
version = "0.1.0"

extensions = ["sub-agent"]
```

- [x] **Step 2: Convert `plan-mode` package schema**

Use:

```toml
name = "plan-mode"
description = "Example package demonstrating a plan mode extension that blocks mutating tools during planning"
version = "0.1.0"

extensions = ["plan-mode"]
```

- [x] **Step 3: Convert `todo` package schema**

Use:

```toml
name = "todo"
description = "Example package demonstrating a todo extension that tracks task state through extension state and custom commands"
version = "0.1.0"

extensions = ["todo"]
```

- [x] **Step 4: Convert `mcp-adapter` package schema**

Use:

```toml
name = "mcp-adapter"
description = "Example package demonstrating an MCP adapter extension that maps MCP-style tools/resources through the extension API"
version = "0.1.0"

extensions = ["mcp-adapter"]
```

- [x] **Step 5: Verify package discovery**

Run:

```powershell
cargo test -p opi-coding-agent --test package_discovery -- --nocapture
```

Expected after the fix: the four example manifests parse through `PackageManifest::from_toml()`.

Completed notes:
- Converted `examples/sub-agent`, `examples/plan-mode`, `examples/todo`, and `examples/mcp-adapter` package manifests from the obsolete `[package]` / `[package.extensions]` form to the supported top-level schema.
- Verified with `cargo test -p opi-coding-agent --test package_discovery -- --nocapture` (39 passed).

### Task 9: Security and API Hardening

**Files:**
- Modify: `crates/opi-coding-agent/src/resource.rs`
- Modify: `crates/opi-coding-agent/src/package_discovery.rs`
- Modify: `crates/opi-coding-agent/src/skill.rs`
- Modify: `crates/opi-coding-agent/src/prompt_fragment.rs`
- Modify: `crates/opi-coding-agent/src/theme_discovery.rs`
- Modify: `crates/opi-agent/Cargo.toml`
- Modify: `crates/opi-agent/src/streaming_proxy.rs`
- Modify: `crates/opi-agent/src/sdk.rs`
- Test: `crates/opi-coding-agent/tests/extension_resources.rs`
- Test: `crates/opi-coding-agent/tests/package_discovery.rs`
- Test: `crates/opi-coding-agent/tests/skills_discovery.rs`
- Test: `crates/opi-coding-agent/tests/prompt_fragments.rs`
- Test: `crates/opi-coding-agent/tests/theme_discovery.rs`
- Test: `crates/opi-agent/tests/streaming_proxy.rs`
- Test: `crates/opi-agent/tests/sdk_embedding.rs`

- [x] **Step 1: Remove canonicalize security fallbacks**

For security-sensitive path containment, convert canonicalize failure into an error rather than falling back to the original path.

- [x] **Step 2: Add symlink traversal tests**

Add real symlink escape tests for package composition and resource discovery. On platforms where symlink creation needs permissions, gate only that test with a clear skip reason.

- [x] **Step 3: Align resource duplicate behavior**

Either implement same-layer duplicate errors or update module docs to say same-layer first-wins. Prefer implementing the documented error because silent duplicates hide packaging mistakes.

- [x] **Step 4: Make streaming proxy I/O semantics honest**

Choose one:

```text
Option A: remove async from run() and document it as synchronous transport-agnostic I/O.
Option B: use tokio::io traits and tokio::sync channels for a genuinely async proxy.
```

If RPC uses this proxy after Task 2, choose Option B.

- [x] **Step 5: Tighten secret redaction matching**

Use regex or explicit length/shape validation so benign strings containing `sk-` or starting with `eyJ` are not automatically redacted.

- [x] **Step 6: Fix SDK response and fallback semantics**

Add:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
```

to `SdkResponse`, and change serialization fallback to a dedicated event type such as:

```json
{"type":"SdkSerializationError","message":"failed to serialize agent event"}
```

- [x] **Step 7: Verify hardening**

Run:

```powershell
cargo test -p opi-coding-agent --test extension_resources -- --nocapture
cargo test -p opi-coding-agent --test package_discovery -- --nocapture
cargo test -p opi-agent --test streaming_proxy -- --nocapture
cargo test -p opi-agent --test sdk_embedding -- --nocapture
```

Expected after the fix: all pass.

Completed notes:
- Removed canonicalize fallbacks from extension, package, skill, fragment, and theme discovery; package composition now treats canonicalization failure as an I/O error instead of comparing unresolved paths.
- Added explicit same-layer `DuplicateName` errors across extension, package, skill, fragment, and theme discovery while preserving higher-precedence override semantics across layers.
- Added symlink tests for extension discovery canonicalization and package composition escape detection. On the current Windows run, symlink creation was skipped with `os error 1314` (missing privilege); the tests otherwise execute and report a clear skip reason.
- Changed `StreamingProxy::run` to synchronous transport-agnostic I/O and updated cancellation docs to state cancellation is observed between blocking reads.
- Replaced heuristic prefix redaction with compiled regex matching and added a regression test proving short `sk-` / `eyJ`-like strings are preserved.
- Added `Deserialize` for `SdkResponse` and changed `agent_event_to_value` serialization fallback to `SdkSerializationError`.
- Verified with:
  - `cargo test -p opi-coding-agent --test extension_resources -- --nocapture` (20 passed)
  - `cargo test -p opi-coding-agent --test package_discovery -- --nocapture` (41 passed)
  - `cargo test -p opi-coding-agent --test skills_discovery -- --nocapture` (30 passed)
  - `cargo test -p opi-coding-agent --test prompt_fragments -- --nocapture` (41 passed)
  - `cargo test -p opi-coding-agent --test theme_discovery -- --nocapture` (37 passed)
  - `cargo test -p opi-agent --test streaming_proxy -- --nocapture` (26 passed)
  - `cargo test -p opi-agent --test sdk_embedding -- --nocapture` (34 passed)
  - `cargo test -p opi-agent --test transport -- --nocapture` (2 passed)
  - `cargo clippy -p opi-agent --all-targets -- -D warnings` (passed)
  - `cargo clippy -p opi-coding-agent --all-targets -- -D warnings` (passed)

### Task 10: Documentation and Ledger Closure

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify last: `docs/snapshots/phase4/opi-impl-state.json`

- [x] **Step 1: Update changelog**

Add Phase 4 entries under `## [Unreleased]`, using existing Keep a Changelog sections only. Do not edit released sections.

- [x] **Step 2: Synchronize root READMEs**

Update both English and Chinese README files in the same change. Remove stale claims that implemented component/state crates are pure placeholders, but keep `opi-web-ui publish = false` and avoid claiming a browser app unless one exists.

- [x] **Step 3: Synchronize specs**

Update `docs/opi-spec.md` and `docs/opi-spec.zh.md` so Phase 4 rows match in both languages and describe the actual runtime/SDK split.

- [x] **Step 4: Update agent-facing docs**

Update `AGENTS.md` and `CLAUDE.md` to reflect only verified behavior. If arbitrary third-party Rust extension loading is not implemented, explicitly describe extension trait support as SDK/embedder-facing rather than CLI dynamic loading.

- [x] **Step 5: Run full gates**

Run:

```powershell
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps
```

Expected output: all commands exit 0.

- [x] **Step 6: Write `phase_exit.4` only after gates pass**

Update `docs/snapshots/phase4/opi-impl-state.json` with a `phase_exit.4` entry that includes:

```json
{
  "completed_at": "2026-06-05T00:00:00Z",
  "exit_criteria_met": true,
  "evaluator_summary": "Phase 4 audit blockers closed: RPC control semantics, tool-selection policy, runtime resource wiring, Web UI RPC data consumption, package schemas, branch UI reachability, docs synchronization, and full gates passing.",
  "snapshot_path": "docs/snapshots/phase4/opi-impl-state.json"
}
```

Use the actual completion timestamp when executing.

Completed notes:
- Updated `CHANGELOG.md` with Phase 4 Added/Changed/Fixed/Removed entries under `## [Unreleased]`.
- Synchronized `README.md` and `README.zh.md` to describe Phase 4 RPC/SDK/extensibility surfaces, `/branch`, config-driven extensions/packages, and `opi-web-ui` as an unpublished reusable component/state/rendering crate rather than a standalone browser app.
- Synchronized `docs/opi-spec.md` and `docs/opi-spec.zh.md` around Phase 4 status, crate roles, dependency graph, release scope, and the removed public `Transport` surface.
- Updated `AGENTS.md` and `CLAUDE.md` so agent-facing instructions reflect verified Phase 4 behavior without claiming dynamic third-party Rust plugin loading.
- Re-exported `ThinkingBlock` from `opi-web-ui` crate root to close the component model API surface documented in the audit.
- Wrote `phase_exit.4` to `docs/snapshots/phase4/opi-impl-state.json` after full gates passed and validated the JSON with `ConvertFrom-Json`.
- Verified with:
  - `cargo fmt --check --all` (passed)
  - `cargo clippy --workspace --all-targets -- -D warnings` (passed)
  - `cargo test --workspace --all-targets` (passed)
  - `$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps` (passed)
  - `cargo test -p opi-coding-agent --test web_ui_rpc -- --nocapture` (9 passed)

## Final Verification

Run these commands before claiming Phase 4 is complete:

```powershell
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
$env:RUSTDOCFLAGS='-D warnings'; cargo doc --workspace --no-deps
cargo test -p opi-coding-agent --test web_ui_rpc -- --nocapture
```

Expected:

```text
No skip lines for target/debug/opi on Windows.
All tests pass.
No clippy warnings.
No rustdoc warnings.
```

## Commit Guidance

Do not commit unless the user explicitly asks. If committing, stage only files intentionally modified by the executed tasks. Never use `git add -A` or `git add .`.

Suggested commit series:

```text
fix(opi-coding-agent): make rpc control commands honest
feat(opi-coding-agent): wire phase 4 resource discovery
fix(opi-web-ui): consume rpc response data
feat(opi-coding-agent): expose session branch selection
fix(examples): normalize phase 4 package manifests
docs: synchronize phase 4 status
```

## Self-Review

- Spec coverage: Every confirmed blocker maps to at least one task. Ambiguous browser-app and dynamic-Rust-extension claims are handled by either implementation or explicit documentation narrowing.
- Placeholder scan: No `TBD`, `TODO`, or unspecified follow-up remains in the execution steps.
- Type consistency: RPC response payload handling is aligned across `SdkResponse`, `RpcRunner`, `WebUiEvent`, and `ConversationState`.
- Safety: `phase_exit.4` is last, not a substitute for fixes or gates.
