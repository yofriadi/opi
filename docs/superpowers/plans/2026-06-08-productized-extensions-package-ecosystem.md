# Productized Extensions and Package Ecosystem Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Phase 5 MVP for local/git packages plus process-JSONL adapters, so packages can be added, listed, diagnosed, and loaded as executable extension adapters.

**Architecture:** Keep package installation, CLI, git interaction, adapter hosting, and diagnostics in `opi-coding-agent`. Keep `opi-agent` limited to existing runtime contracts: `Tool`, `Extension`, `AgentHooks`, events, and state. Process adapters are bridged into those contracts by `ProcessAdapter`, `ProcessAdapterTool`, and `ProcessAdapterHooks`.

**Tech Stack:** Rust 2024, clap, serde/TOML, tokio process I/O, JSONL over stdin/stdout, existing `opi-agent` extension traits, existing `opi-coding-agent` resource discovery.

---

## Scope Inputs

- Design spec: `docs/superpowers/specs/2026-06-08-productized-extensions-package-ecosystem-design.md`
- `/opi-implement` supplemental source registration: `.claude/skills/opi-implement/skill.md`, `.agents/skills/opi-implement/skill.md`, and both `references/initializer.md` mirrors

Phase 5 task IDs below are intended for `/opi-implement` as `5.1` through `5.9`.

## File Map

| Path | Action | Responsibility |
|---|---|---|
| `crates/opi-coding-agent/src/package_store.rs` | Create | Package source parsing, declarations, lock entries, add/remove/list/doctor store operations |
| `crates/opi-coding-agent/src/package_cli.rs` | Create | User-facing `opi package` command execution and table/JSON output |
| `crates/opi-coding-agent/src/package_discovery.rs` | Modify | Manifest V2 fields: `opi_version`, `[adapter]`, filters remain backward compatible |
| `crates/opi-coding-agent/src/adapter_protocol.rs` | Create | `opi-extension-jsonl-v1` serde message types and validation helpers |
| `crates/opi-coding-agent/src/adapter_host.rs` | Create | Child process host, request correlation, timeouts, cancellation, event delivery, shutdown |
| `crates/opi-coding-agent/src/adapter_extension.rs` | Create | `ProcessAdapter`, `ProcessAdapterTool`, `ProcessAdapterHooks` bridge into runtime traits |
| `crates/opi-coding-agent/src/cli.rs` | Modify | Add `package` subcommand group without disturbing existing flags |
| `crates/opi-coding-agent/src/main.rs` | Modify | Handle package commands before provider construction |
| `crates/opi-coding-agent/src/harness.rs` | Modify | Load installed packages, start adapters, merge adapter registry/tools/hooks/diagnostics |
| `crates/opi-coding-agent/src/lib.rs` | Modify | Export new internal modules for tests |
| `crates/opi-coding-agent/tests/package_store.rs` | Create | Store/source/lock tests |
| `crates/opi-coding-agent/tests/package_cli.rs` | Create | CLI command tests |
| `crates/opi-coding-agent/tests/package_manifest_v2.rs` | Create | Manifest V2 and compatibility tests |
| `crates/opi-coding-agent/tests/adapter_protocol.rs` | Create | Protocol serde and validation tests |
| `crates/opi-coding-agent/tests/adapter_host.rs` | Create | Mock adapter process contract tests |
| `crates/opi-coding-agent/tests/adapter_runtime.rs` | Create | Harness/RPC integration tests for adapter tools, commands, hooks, events, cancellation, state |
| `crates/opi-coding-agent/examples/package_adapter_example.rs` | Create | Development-only adapter executable used by example packages |
| `examples/todo/package.toml` | Modify | Add process adapter declaration |
| `examples/permission-gate/package.toml` | Modify | Add process adapter declaration |
| `examples/protected-paths/package.toml` | Modify | Add process adapter declaration |
| `README.md` / `README.zh.md` | Modify | Document Phase 5 MVP honestly |
| `docs/opi-spec.md` / `docs/opi-spec.zh.md` | Modify | Add Phase 5 status/scope after implementation |
| `docs/pi-alignment-matrix.md` | Modify | Update package ecosystem alignment |

---

### Task 5.1: Package Store and Source Model

**Files:**
- Create: `crates/opi-coding-agent/src/package_store.rs`
- Modify: `crates/opi-coding-agent/src/lib.rs`
- Test: `crates/opi-coding-agent/tests/package_store.rs`

**Definition of done:** Local and git sources parse deterministically; global/project `packages.toml` and `package-lock.toml` read/write through temp directories; lock entries record source path, optional git commit, cache path, and manifest hash; tests cover Windows-style paths without touching real user config.

- [ ] **Step 1: Write failing source parser tests**

Create `crates/opi-coding-agent/tests/package_store.rs` with these tests:

```rust
use std::path::PathBuf;

use opi_coding_agent::package_store::{PackageSource, PackageStoreScope};

#[test]
fn parses_local_relative_source() {
    let source = PackageSource::parse("./vendor/todo").expect("parse source");
    assert!(matches!(source, PackageSource::Local { .. }));
    assert_eq!(source.identity_key().kind, "local");
}

#[test]
fn parses_github_shorthand_with_ref() {
    let source = PackageSource::parse("git:github.com/user/repo@v1").expect("parse source");
    match source {
        PackageSource::Git { url, refspec } => {
            assert_eq!(url, "https://github.com/user/repo");
            assert_eq!(refspec.as_deref(), Some("v1"));
        }
        PackageSource::Local { .. } => panic!("expected git source"),
    }
}

#[test]
fn project_scope_paths_live_under_dot_opi() {
    let root = PathBuf::from(r"C:\work\opi");
    let scope = PackageStoreScope::Project { workspace_root: root.clone() };
    assert_eq!(scope.config_path(), root.join(".opi").join("packages.toml"));
    assert_eq!(scope.lock_path(), root.join(".opi").join("package-lock.toml"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p opi-coding-agent --test package_store`

Expected: compile failure because `package_store` module and types do not exist.

- [ ] **Step 3: Implement source and store types**

Create `crates/opi-coding-agent/src/package_store.rs` with these public types:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSource {
    Local { path: PathBuf },
    Git { url: String, refspec: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageIdentity {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageStoreScope {
    Global { user_config_dir: PathBuf },
    Project { workspace_root: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PackageDeclaration {
    pub source: String,
    #[serde(default)]
    pub filters: PackageFilters,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PackageFilters {
    pub extensions: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub fragments: Option<Vec<String>>,
    pub themes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PackageLockEntry {
    pub identity_kind: String,
    pub identity_value: String,
    pub source: String,
    pub package_root: PathBuf,
    pub cache_path: Option<PathBuf>,
    pub git_commit: Option<String>,
    pub manifest_sha256: String,
}
```

Also add `pub mod package_store;` to `crates/opi-coding-agent/src/lib.rs`.

- [ ] **Step 4: Implement parsing and path helpers**

Add these methods:

```rust
impl PackageSource {
    pub fn parse(raw: &str) -> Result<Self, PackageStoreError>;
    pub fn identity_key(&self) -> PackageIdentity;
}

impl PackageStoreScope {
    pub fn config_path(&self) -> PathBuf;
    pub fn lock_path(&self) -> PathBuf;
    pub fn cache_dir(&self) -> PathBuf;
}
```

Use `std::process::Command` for git operations in later steps; do not add `git2` or `gix`.

- [ ] **Step 5: Add read/write tests for declarations and lock entries**

Append tests:

```rust
use tempfile::tempdir;
use opi_coding_agent::package_store::{PackageDeclaration, PackageLockEntry, PackageStore};

#[test]
fn writes_and_reads_project_declarations() {
    let dir = tempdir().expect("tempdir");
    let store = PackageStore::project(dir.path().to_path_buf());
    store.write_declarations(&[PackageDeclaration {
        source: "./examples/todo".into(),
        filters: Default::default(),
    }]).expect("write declarations");
    let loaded = store.read_declarations().expect("read declarations");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].source, "./examples/todo");
}

#[test]
fn writes_and_reads_lock_entries() {
    let dir = tempdir().expect("tempdir");
    let store = PackageStore::project(dir.path().to_path_buf());
    store.write_lock(&[PackageLockEntry {
        identity_kind: "local".into(),
        identity_value: dir.path().join("pkg").display().to_string(),
        source: "./pkg".into(),
        package_root: dir.path().join("pkg"),
        cache_path: None,
        git_commit: None,
        manifest_sha256: "abc123".into(),
    }]).expect("write lock");
    let loaded = store.read_lock().expect("read lock");
    assert_eq!(loaded[0].manifest_sha256, "abc123");
}
```

- [ ] **Step 6: Run task tests**

Run: `cargo test -p opi-coding-agent --test package_store`

Expected: all package store tests pass.

- [ ] **Step 7: Run crate check**

Run: `cargo clippy -p opi-coding-agent --test package_store -- -D warnings`

Expected: no warnings.

---

### Task 5.2: Package CLI MVP

**Files:**
- Create: `crates/opi-coding-agent/src/package_cli.rs`
- Modify: `crates/opi-coding-agent/src/cli.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Modify: `crates/opi-coding-agent/src/lib.rs`
- Test: `crates/opi-coding-agent/tests/package_cli.rs`

**Definition of done:** `opi package add/remove/list/doctor` works before provider construction, supports project scope with `-l`, supports JSON output for list and doctor, and never reads real user config during tests.

- [ ] **Step 1: Add failing CLI parser tests**

Create `crates/opi-coding-agent/tests/package_cli.rs`:

```rust
use clap::Parser;
use opi_coding_agent::cli::{Cli, Command, PackageCommand};

#[test]
fn parses_package_add_project_scope() {
    let cli = Cli::parse_from(["opi", "package", "add", "./pkg", "-l"]);
    match cli.command {
        Some(Command::Package(PackageCommand::Add { source, local })) => {
            assert_eq!(source, "./pkg");
            assert!(local);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_package_doctor_json() {
    let cli = Cli::parse_from(["opi", "package", "doctor", "--json"]);
    match cli.command {
        Some(Command::Package(PackageCommand::Doctor { json })) => assert!(json),
        other => panic!("unexpected command: {other:?}"),
    }
}
```

- [ ] **Step 2: Run parser tests to verify failure**

Run: `cargo test -p opi-coding-agent --test package_cli parses_package`

Expected: compile failure because `Command` and `PackageCommand` do not exist.

- [ ] **Step 3: Extend CLI types**

In `crates/opi-coding-agent/src/cli.rs`, add:

```rust
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Package(PackageCommand),
}

#[derive(Debug, clap::Subcommand)]
pub enum PackageCommand {
    Add {
        source: String,
        #[arg(short = 'l', long = "local")]
        local: bool,
    },
    Remove {
        name_or_source: String,
        #[arg(short = 'l', long = "local")]
        local: bool,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Doctor {
        #[arg(long)]
        json: bool,
    },
}
```

Add `#[command(subcommand)] pub command: Option<Command>,` to `Cli`.

- [ ] **Step 4: Implement package command entrypoint**

Create `crates/opi-coding-agent/src/package_cli.rs`:

```rust
use std::path::PathBuf;

use crate::cli::PackageCommand;
use crate::package_store::{PackageStore, PackageStoreScope};

pub fn handle_package_command(
    command: &PackageCommand,
    workspace_root: PathBuf,
    user_config_dir: PathBuf,
) -> i32 {
    let scope = match command {
        PackageCommand::Add { local, .. }
        | PackageCommand::Remove { local, .. } if *local => {
            PackageStoreScope::Project { workspace_root }
        }
        _ => PackageStoreScope::Global { user_config_dir },
    };
    let store = PackageStore::new(scope);
    match run_package_command(command, &store) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("opi package: {e}");
            2
        }
    }
}

fn run_package_command(command: &PackageCommand, store: &PackageStore) -> Result<(), String> {
    match command {
        PackageCommand::Add { source, .. } => store.add(source).map_err(|e| e.to_string()),
        PackageCommand::Remove { name_or_source, .. } => {
            store.remove(name_or_source).map_err(|e| e.to_string())
        }
        PackageCommand::List { json } => store.print_list(*json).map_err(|e| e.to_string()),
        PackageCommand::Doctor { json } => store.print_doctor(*json).map_err(|e| e.to_string()),
    }
}
```

Export `pub mod package_cli;` from `lib.rs`.

- [ ] **Step 5: Wire package command before provider construction**

In `crates/opi-coding-agent/src/main.rs`, after completion generation and verbose handling, add:

```rust
if let Some(opi_coding_agent::cli::Command::Package(command)) = &cli.command {
    let workspace_root = std::env::current_dir().unwrap_or_default();
    let user_config_dir = opi_coding_agent::config::user_config_dir();
    let exit_code =
        opi_coding_agent::package_cli::handle_package_command(command, workspace_root, user_config_dir);
    std::process::exit(exit_code);
}
```

- [ ] **Step 6: Add command behavior tests**

Append tests that call `handle_package_command` with temp directories:

```rust
use tempfile::tempdir;
use opi_coding_agent::cli::PackageCommand;
use opi_coding_agent::package_cli::handle_package_command;

#[test]
fn package_add_writes_project_config() {
    let workspace = tempdir().expect("workspace");
    let user = tempdir().expect("user");
    let code = handle_package_command(
        &PackageCommand::Add { source: "./pkg".into(), local: true },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(code, 0);
    assert!(workspace.path().join(".opi").join("packages.toml").exists());
}
```

- [ ] **Step 7: Run task tests**

Run: `cargo test -p opi-coding-agent --test package_cli`

Expected: all package CLI tests pass.

---

### Task 5.3: Manifest V2 Compatibility

**Files:**
- Modify: `crates/opi-coding-agent/src/package_discovery.rs`
- Test: `crates/opi-coding-agent/tests/package_manifest_v2.rs`

**Definition of done:** Existing flat manifests still parse; optional `opi_version` and `[adapter]` parse; relative adapter command resolution is specified; missing resources and path containment behavior remain unchanged.

- [ ] **Step 1: Write failing Manifest V2 tests**

Create `crates/opi-coding-agent/tests/package_manifest_v2.rs`:

```rust
use std::path::Path;

use opi_coding_agent::package_discovery::{AdapterManifest, PackageManifest};

#[test]
fn parses_manifest_v2_adapter_fields() {
    let manifest = PackageManifest::from_toml(
        r#"
name = "todo"
description = "Todo package"
version = "0.1.0"
opi_version = ">=0.5,<0.7"
skills = ["todo"]

[adapter]
kind = "process-jsonl"
command = "todo-adapter"
args = ["--mode", "todo"]
protocol = "opi-extension-jsonl-v1"
timeout_ms = 30000
"#,
        Path::new("package.toml"),
    )
    .expect("parse manifest");

    assert_eq!(manifest.opi_version.as_deref(), Some(">=0.5,<0.7"));
    assert_eq!(manifest.adapter.as_ref().map(|a| a.protocol.as_str()), Some("opi-extension-jsonl-v1"));
}

#[test]
fn flat_manifest_without_adapter_stays_valid() {
    let manifest = PackageManifest::from_toml(
        r#"
name = "resource-only"
description = "Resource only package"
skills = ["review"]
"#,
        Path::new("package.toml"),
    )
    .expect("parse manifest");

    assert!(manifest.adapter.is_none());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p opi-coding-agent --test package_manifest_v2`

Expected: compile failure because `AdapterManifest`, `opi_version`, and `adapter` do not exist.

- [ ] **Step 3: Extend manifest structs**

In `package_discovery.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterManifest {
    pub kind: String,
    pub command: String,
    pub args: Vec<String>,
    pub protocol: String,
    pub timeout_ms: Option<u64>,
}
```

Extend `PackageManifest`:

```rust
pub opi_version: Option<String>,
pub adapter: Option<AdapterManifest>,
```

Extend TOML structs with `opi_version: Option<String>` and `adapter: Option<TomlAdapterTable>`.

- [ ] **Step 4: Validate adapter fields**

Add validation rules:

```rust
fn validate_adapter(adapter: &AdapterManifest, path: &Path) -> Result<(), PackageDiscoveryError> {
    if adapter.kind != "process-jsonl" {
        return Err(PackageDiscoveryError::InvalidManifest {
            path: path.to_path_buf(),
            reason: format!("unsupported adapter kind '{}'", adapter.kind),
        });
    }
    if adapter.command.trim().is_empty() {
        return Err(PackageDiscoveryError::MissingField {
            field: "adapter.command".into(),
            path: path.to_path_buf(),
        });
    }
    if adapter.protocol != "opi-extension-jsonl-v1" {
        return Err(PackageDiscoveryError::InvalidManifest {
            path: path.to_path_buf(),
            reason: format!("unsupported adapter protocol '{}'", adapter.protocol),
        });
    }
    Ok(())
}
```

- [ ] **Step 5: Run compatibility tests**

Run: `cargo test -p opi-coding-agent --test package_manifest_v2`

Expected: manifest tests pass.

Run: `cargo test -p opi-coding-agent --test package_discovery`

Expected: existing package discovery tests still pass.

---

### Task 5.4: Adapter Protocol Types

**Files:**
- Create: `crates/opi-coding-agent/src/adapter_protocol.rs`
- Modify: `crates/opi-coding-agent/src/lib.rs`
- Test: `crates/opi-coding-agent/tests/adapter_protocol.rs`

**Definition of done:** Protocol serde supports initialize/capabilities/tool/command/hook/event/state/cancel/shutdown messages; unknown protocol is rejected; JSONL messages round-trip without provider access.

- [ ] **Step 1: Write failing protocol tests**

Create `crates/opi-coding-agent/tests/adapter_protocol.rs`:

```rust
use opi_coding_agent::adapter_protocol::{AdapterHostMessage, AdapterProcessMessage, PROTOCOL_VERSION};

#[test]
fn initialize_serializes_with_protocol_version() {
    let value = serde_json::to_value(AdapterHostMessage::initialize("1", "todo")).expect("json");
    assert_eq!(value["type"], "initialize");
    assert_eq!(value["protocol"], PROTOCOL_VERSION);
    assert_eq!(value["package"], "todo");
}

#[test]
fn capabilities_deserialize_tools_commands_hooks() {
    let msg: AdapterProcessMessage = serde_json::from_str(
        r#"{"type":"capabilities","id":"1","tools":[],"commands":[],"hooks":["before_tool_call","event"]}"#,
    )
    .expect("parse capabilities");
    assert!(matches!(msg, AdapterProcessMessage::Capabilities { .. }));
}

#[test]
fn cancel_serializes_for_inflight_request() {
    let value = serde_json::to_value(AdapterHostMessage::cancel("2", "user_abort")).expect("json");
    assert_eq!(value["type"], "cancel");
    assert_eq!(value["id"], "2");
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p opi-coding-agent --test adapter_protocol`

Expected: compile failure because module does not exist.

- [ ] **Step 3: Implement protocol enums**

Create `adapter_protocol.rs` with:

```rust
pub const PROTOCOL_VERSION: &str = "opi-extension-jsonl-v1";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdapterHostMessage {
    Initialize { id: String, protocol: String, package: String },
    ToolCall { id: String, tool: String, args: serde_json::Value },
    Command { id: String, name: String, args: serde_json::Value },
    Hook { id: String, hook: String, payload: serde_json::Value },
    Event { event: serde_json::Value },
    StateSerialize { id: String },
    StateRestore { id: String, state: serde_json::Value },
    Cancel { id: String, reason: String },
    Shutdown { id: String, reason: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdapterProcessMessage {
    Capabilities {
        id: String,
        tools: Vec<AdapterToolCapability>,
        commands: Vec<AdapterCommandCapability>,
        hooks: Vec<String>,
        model_overrides: Vec<AdapterModelOverride>,
    },
    ToolResult { id: String, content: Vec<serde_json::Value>, is_error: bool },
    CommandResult { id: String, data: serde_json::Value },
    HookResult { id: String, action: String, data: Option<serde_json::Value> },
    StateResult { id: String, state: serde_json::Value },
    Error { id: Option<String>, message: String },
}
```

Add helper constructors for `initialize` and `cancel`.

- [ ] **Step 4: Run protocol tests**

Run: `cargo test -p opi-coding-agent --test adapter_protocol`

Expected: all protocol tests pass.

---

### Task 5.5: Adapter Process Host

**Files:**
- Create: `crates/opi-coding-agent/src/adapter_host.rs`
- Modify: `crates/opi-coding-agent/src/lib.rs`
- Test: `crates/opi-coding-agent/tests/adapter_host.rs`

**Definition of done:** Host starts a child process, performs initialize/capabilities handshake, sends correlated requests, times out requests, sends best-effort cancel, drops event messages under backpressure, and reaps the child on shutdown.

- [ ] **Step 1: Write failing host tests**

Create `crates/opi-coding-agent/tests/adapter_host.rs`:

```rust
use std::time::Duration;

use opi_coding_agent::adapter_host::{AdapterHost, AdapterProcessConfig};

#[tokio::test]
async fn host_initializes_mock_adapter() {
    let config = AdapterProcessConfig::test_current_exe("capabilities");
    let host = AdapterHost::start("mock", config, Duration::from_secs(2))
        .await
        .expect("start host");
    assert!(host.capabilities().hooks.iter().any(|hook| hook == "event"));
    host.shutdown("test_end").await.expect("shutdown");
}

#[tokio::test]
async fn host_times_out_unresponsive_adapter() {
    let config = AdapterProcessConfig::test_current_exe("hang");
    let err = AdapterHost::start("mock", config, Duration::from_millis(50))
        .await
        .expect_err("timeout expected");
    assert!(err.to_string().contains("initialize timed out"));
}
```

In the same file, implement a test helper `main` path by spawning `std::env::current_exe()` with an env var such as `OPI_ADAPTER_TEST_MODE`. The test binary should act as adapter only when that env var is present.

- [ ] **Step 2: Run host tests to verify failure**

Run: `cargo test -p opi-coding-agent --test adapter_host host_initializes_mock_adapter -- --nocapture`

Expected: compile failure because `adapter_host` module does not exist.

- [ ] **Step 3: Implement process config and host shell**

Create:

```rust
#[derive(Debug, Clone)]
pub struct AdapterProcessConfig {
    pub command: std::path::PathBuf,
    pub args: Vec<String>,
    pub working_dir: std::path::PathBuf,
}

pub struct AdapterHost {
    package_name: String,
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    capabilities: AdapterCapabilities,
    timeout: std::time::Duration,
}
```

Use `tokio::process::Command` with piped stdin/stdout and line-based JSONL framing.

- [ ] **Step 4: Implement handshake and request correlation**

Add:

```rust
impl AdapterHost {
    pub async fn start(
        package_name: impl Into<String>,
        config: AdapterProcessConfig,
        timeout: std::time::Duration,
    ) -> Result<Self, AdapterHostError>;

    pub fn capabilities(&self) -> &AdapterCapabilities;

    pub async fn send_request(
        &self,
        message: AdapterHostMessage,
        timeout: std::time::Duration,
    ) -> Result<AdapterProcessMessage, AdapterHostError>;

    pub async fn send_event(&self, event: serde_json::Value);

    pub async fn cancel(&self, id: &str, reason: &str);

    pub async fn shutdown(self, reason: &str) -> Result<(), AdapterHostError>;
}
```

Use a single writer task and a reader task with a `HashMap<String, oneshot::Sender<_>>` for pending responses.

- [ ] **Step 5: Run host tests**

Run: `cargo test -p opi-coding-agent --test adapter_host -- --nocapture`

Expected: host tests pass and leave no child process alive.

---

### Task 5.6: Adapter Runtime Bridge

**Files:**
- Create: `crates/opi-coding-agent/src/adapter_extension.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/lib.rs`
- Test: `crates/opi-coding-agent/tests/adapter_runtime.rs`

**Definition of done:** Adapter capabilities become runtime tools, commands, selected hooks, event observers, state handlers, cancellation bridge, and static model overrides through existing extension/hook contracts.

- [ ] **Step 1: Write failing bridge tests**

Create `crates/opi-coding-agent/tests/adapter_runtime.rs`:

```rust
use opi_agent::extension::ExtensionCommand;
use opi_coding_agent::adapter_extension::ProcessAdapter;
use opi_coding_agent::adapter_host::AdapterProcessConfig;

#[tokio::test]
async fn adapter_command_dispatches_through_extension_registry() {
    let adapter = ProcessAdapter::start_for_test("todo", AdapterProcessConfig::test_current_exe("todo"))
        .await
        .expect("adapter");
    let registry = adapter.into_registry();
    let result = registry
        .dispatch_command(&ExtensionCommand::new("todo/list", serde_json::json!({})))
        .await
        .expect("dispatch")
        .expect("handled");
    assert_eq!(result["items"].as_array().expect("items").len(), 0);
}

#[tokio::test]
async fn adapter_before_tool_hook_can_block() {
    let adapter = ProcessAdapter::start_for_test("gate", AdapterProcessConfig::test_current_exe("gate"))
        .await
        .expect("adapter");
    let result = adapter.on_before_tool_call("bash", &serde_json::json!({"command":"rm -rf target"})).await;
    assert!(matches!(result, opi_agent::extension::ExtensionHookResult::Block { .. }));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p opi-coding-agent --test adapter_runtime adapter_command_dispatches -- --nocapture`

Expected: compile failure because bridge module does not exist.

- [ ] **Step 3: Implement ProcessAdapter and ProcessAdapterTool**

Create:

```rust
pub struct ProcessAdapter {
    name: String,
    host: std::sync::Arc<AdapterHost>,
    tools: Vec<AdapterToolDefinition>,
    commands: Vec<String>,
    hooks: std::collections::BTreeSet<String>,
}

pub struct ProcessAdapterTool {
    name: String,
    description: String,
    schema: serde_json::Value,
    host: std::sync::Arc<AdapterHost>,
}
```

Implement `opi_agent::extension::Extension` for `ProcessAdapter` and `opi_agent::tool::Tool` for `ProcessAdapterTool`.

- [ ] **Step 4: Implement cancellation bridge**

In `ProcessAdapterTool::execute`, wrap the request future with `tokio::select!`:

```rust
tokio::select! {
    result = host.call_tool(&request_id, &tool_name, args) => result,
    _ = signal.cancelled() => {
        host.cancel(&request_id, "tool_cancelled").await;
        Ok(opi_agent::tool::ToolResult::error("adapter tool cancelled"))
    }
}
```

- [ ] **Step 5: Implement ProcessAdapterHooks**

Add a wrapper that implements `AgentHooks` and delegates `transform_context` to adapters that declared the hook. It must call the base hooks first and only call adapters that declared `transform_context`.

- [ ] **Step 6: Run bridge tests**

Run: `cargo test -p opi-coding-agent --test adapter_runtime -- --nocapture`

Expected: bridge tests pass.

---

### Task 5.7: Harness and Startup Integration

**Files:**
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Test: `crates/opi-coding-agent/tests/adapter_runtime.rs`
- Test: `crates/opi-coding-agent/tests/harness_resource_integration.rs`
- Test: `crates/opi-coding-agent/tests/rpc_jsonl.rs`

**Definition of done:** Startup reads declared package stores, composes package resources, starts adapters in deterministic order, merges adapter tools/commands/hooks/state into the harness, and reports adapter diagnostics through existing resource metadata and RPC `session_info`.

- [ ] **Step 1: Add failing harness integration test**

Append to `adapter_runtime.rs`:

```rust
#[tokio::test]
async fn harness_session_info_reports_adapter_diagnostic() {
    let workspace = tempfile::tempdir().expect("workspace");
    write_package_with_broken_adapter(workspace.path());
    let harness = build_harness_for_workspace(workspace.path()).await;
    let diagnostics = harness.resource_metadata().diagnostics.join("\n");
    assert!(diagnostics.contains("adapter"));
    assert!(diagnostics.contains("failed"));
}
```

The helper writes `.opi/packages.toml`, a package root, and a package manifest with `[adapter] command = "missing-opi-adapter-test-binary"`.

- [ ] **Step 2: Run failing test**

Run: `cargo test -p opi-coding-agent --test adapter_runtime harness_session_info_reports_adapter_diagnostic -- --nocapture`

Expected: failure because harness ignores installed package stores and adapters.

- [ ] **Step 3: Add package store loading to harness build**

In `CodingHarness::discover_resources`, load global/project package declarations before direct resource discovery. Merge discovered package roots into package discovery layers.

- [ ] **Step 4: Start adapters after package manifest parsing**

Create an internal `AdapterRuntimeSet` that returns:

```rust
pub struct AdapterRuntimeSet {
    pub registry: opi_agent::extension::ExtensionRegistry,
    pub hooks: Option<Box<dyn opi_agent::hooks::AgentHooks>>,
    pub diagnostics: Vec<String>,
}
```

Merge its registry with any in-process registry supplied by SDK embedders.

- [ ] **Step 5: Preserve tool selection behavior**

Ensure adapter tools obey existing `ToolSelection`:

```rust
let adapter_tools = filter_extension_tools(adapter_registry.collect_tools(), &build_options.tool_selection);
```

If `--no-tools` is active, adapter tools are not exposed. If `--no-builtin-tools` is active, adapter tools remain available.

- [ ] **Step 6: Run integration tests**

Run:

```sh
cargo test -p opi-coding-agent --test adapter_runtime -- --nocapture
cargo test -p opi-coding-agent --test harness_resource_integration -- --nocapture
cargo test -p opi-coding-agent --test rpc_jsonl rpc_session_info -- --nocapture
```

Expected: all targeted tests pass.

---

### Task 5.8: Runnable Example Adapter Packages

**Files:**
- Create: `crates/opi-coding-agent/examples/package_adapter_example.rs`
- Modify: `examples/todo/package.toml`
- Modify: `examples/permission-gate/package.toml`
- Modify: `examples/protected-paths/package.toml`
- Modify: corresponding example READMEs
- Test: `crates/opi-coding-agent/tests/adapter_runtime.rs`

**Definition of done:** `todo`, `permission-gate`, and `protected-paths` examples declare process adapters and can be exercised in tests without Node, npm, or live providers.

- [ ] **Step 1: Create example adapter executable**

Create `crates/opi-coding-agent/examples/package_adapter_example.rs` with a small JSONL loop:

```rust
use std::io::{self, BufRead, Write};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "todo".into());
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.expect("stdin line");
        let value: serde_json::Value = serde_json::from_str(&line).expect("json line");
        let response = handle_message(&mode, value);
        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response).expect("write response");
            stdout.write_all(b"\n").expect("write newline");
            stdout.flush().expect("flush");
        }
    }
}

fn handle_message(mode: &str, value: serde_json::Value) -> Option<serde_json::Value> {
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("0");
    match value.get("type").and_then(|v| v.as_str()) {
        Some("initialize") => Some(capabilities(id, mode)),
        Some("command") if mode == "todo" => {
            Some(serde_json::json!({"type":"command_result","id":id,"data":{"items":[]}}))
        }
        Some("hook") if mode == "permission-gate" => {
            Some(serde_json::json!({"type":"hook_result","id":id,"action":"block","data":{"reason":"blocked by example adapter"}}))
        }
        Some("hook") if mode == "protected-paths" => {
            Some(serde_json::json!({"type":"hook_result","id":id,"action":"continue"}))
        }
        Some("shutdown") => Some(serde_json::json!({"type":"command_result","id":id,"data":{"shutdown":true}})),
        _ => None,
    }
}

fn capabilities(id: &str, mode: &str) -> serde_json::Value {
    match mode {
        "todo" => serde_json::json!({"type":"capabilities","id":id,"tools":[],"commands":[{"name":"todo/list","description":"List todo items"}],"hooks":[],"model_overrides":[]}),
        "permission-gate" => serde_json::json!({"type":"capabilities","id":id,"tools":[],"commands":[],"hooks":["before_tool_call"],"model_overrides":[]}),
        _ => serde_json::json!({"type":"capabilities","id":id,"tools":[],"commands":[],"hooks":["before_tool_call"],"model_overrides":[]}),
    }
}
```

- [ ] **Step 2: Add adapter declarations to examples**

For `examples/todo/package.toml`:

```toml
[adapter]
kind = "process-jsonl"
command = "cargo"
args = ["run", "-p", "opi-coding-agent", "--example", "package_adapter_example", "--", "todo"]
protocol = "opi-extension-jsonl-v1"
timeout_ms = 30000
```

Use mode `permission-gate` for `examples/permission-gate/package.toml` and mode `protected-paths` for `examples/protected-paths/package.toml`.

- [ ] **Step 3: Add runnable example tests**

Add tests that run `opi package add ./examples/todo -l`, then `opi package doctor --json` in a temp copy of the example package, and assert the adapter handshake succeeds.

- [ ] **Step 4: Run example tests**

Run: `cargo test -p opi-coding-agent --test adapter_runtime example_adapters -- --nocapture`

Expected: example adapter tests pass without provider credentials.

---

### Task 5.9: Documentation, Alignment, and Guards

**Files:**
- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `docs/pi-alignment-matrix.md`
- Create: `crates/opi-coding-agent/tests/productized_packages_docs.rs`

**Definition of done:** User docs describe the Phase 5 MVP truthfully; localized docs are synchronized; docs guard tests reject claims that npm, marketplace, hot reload, provider streaming adapters, custom TUI adapters, or package permission enforcement are complete.

- [ ] **Step 1: Add docs guard tests**

Create `crates/opi-coding-agent/tests/productized_packages_docs.rs`:

```rust
use std::fs;
use std::path::Path;

#[test]
fn docs_do_not_claim_deferred_package_features_are_complete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let docs = [
        "README.md",
        "README.zh.md",
        "docs/opi-spec.md",
        "docs/opi-spec.zh.md",
        "docs/pi-alignment-matrix.md",
    ];
    let forbidden = [
        "npm packages are supported",
        "package marketplace is supported",
        "adapter hot reload is supported",
        "package permissions are enforced",
        "external provider streaming adapters are supported",
    ];
    for doc in docs {
        let text = fs::read_to_string(root.join(doc)).expect("read doc");
        for phrase in forbidden {
            assert!(
                !text.contains(phrase),
                "{doc} claims deferred feature as complete: {phrase}"
            );
        }
    }
}
```

- [ ] **Step 2: Run docs guard to verify current behavior**

Run: `cargo test -p opi-coding-agent --test productized_packages_docs`

Expected: pass or fail only on real false claims. If it fails, update the offending docs in this task.

- [ ] **Step 3: Update README files**

Add a concise section describing:

```text
Phase 5 package MVP:
- opi package add/remove/list/doctor
- local and git package sources
- package.toml resource bundles plus optional process-jsonl adapters
- adapter tools, commands, selected hooks, event observation, cancellation, and session-scoped state

Deferred:
- npm registry packages
- marketplace/gallery
- package permission enforcement
- hot reload
- custom TUI adapter protocol
- external provider streaming adapters
```

Apply equivalent content to `README.zh.md`.

- [ ] **Step 4: Update opi spec files**

In `docs/opi-spec.md` and `docs/opi-spec.zh.md`, add a Phase 5 product hardening milestone after Phase 4. Keep the central rule unchanged: Rust-native semantics, not TypeScript API compatibility.

- [ ] **Step 5: Update pi alignment matrix**

Update `docs/pi-alignment-matrix.md` package rows:

```text
opi-coding-agent: Partial -> productized package MVP present; npm/marketplace/hot reload still missing.
opi-agent: Full core semantics unchanged; process adapters map into existing runtime traits.
```

- [ ] **Step 6: Run documentation tests**

Run: `cargo test -p opi-coding-agent --test productized_packages_docs`

Expected: docs guard passes.

- [ ] **Step 7: Run final Phase 5 gates**

Run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p opi-coding-agent --test package_store
cargo test -p opi-coding-agent --test package_cli
cargo test -p opi-coding-agent --test package_manifest_v2
cargo test -p opi-coding-agent --test adapter_protocol
cargo test -p opi-coding-agent --test adapter_host -- --nocapture
cargo test -p opi-coding-agent --test adapter_runtime -- --nocapture
cargo test -p opi-coding-agent --test productized_packages_docs
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Expected: all commands pass.

---

## `/opi-implement` Task Graph Summary

| Task ID | Title | Depends on | Tier | Owned paths |
|---|---|---|---|---|
| 5.1 | Package store and source model | Phase 4 complete | `cli-runtime` | `crates/opi-coding-agent/**`, `Cargo.toml` |
| 5.2 | Package CLI MVP | 5.1 | `cli-runtime` | `crates/opi-coding-agent/**` |
| 5.3 | Manifest V2 compatibility | 5.1 | `cli-runtime` | `crates/opi-coding-agent/**`, `examples/**` |
| 5.4 | Adapter protocol types | 5.3 | `cli-runtime` | `crates/opi-coding-agent/**` |
| 5.5 | Adapter process host | 5.4 | `cli-runtime` | `crates/opi-coding-agent/**` |
| 5.6 | Adapter runtime bridge | 5.5 | `workspace` | `crates/opi-coding-agent/**`, `crates/opi-agent/**` |
| 5.7 | Harness and startup integration | 5.6 | `workspace` | `crates/opi-coding-agent/**`, `crates/opi-agent/**` |
| 5.8 | Runnable example adapter packages | 5.7 | `cli-runtime` | `examples/**`, `crates/opi-coding-agent/**` |
| 5.9 | Documentation, alignment, and guards | 5.8 | `workspace` | `README.md`, `README.zh.md`, `docs/opi-spec.md`, `docs/opi-spec.zh.md`, `docs/pi-alignment-matrix.md`, `crates/opi-coding-agent/**` |

## Self-Review Checklist

- Spec coverage: every success criterion in the design spec maps to one of tasks 5.1 through 5.9.
- Scope control: npm, marketplace, hot reload, custom TUI adapters, provider streaming adapters, package permissions, event bus, and dynamic registration are explicitly deferred.
- Type consistency: package store types live in `package_store`, protocol types in `adapter_protocol`, process hosting in `adapter_host`, trait bridge in `adapter_extension`.
- Verification: each task has a concrete test command and a crate-level or workspace gate.
- Git safety: execution through `/opi-implement` must stage only task-owned files and must respect unrelated dirty working-tree state.
