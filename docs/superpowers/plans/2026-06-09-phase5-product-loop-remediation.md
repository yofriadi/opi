# Phase 5 Product Loop Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the Phase 5 package ecosystem product loop so `opi package add <source>` produces a resolved, locked package that a later `opi` startup loads, starts, diagnoses, persists, and documents truthfully.

**Architecture:** Add one shared package resolver in `opi-coding-agent` and make the package CLI plus runtime startup use it. Keep package installation and process-adapter hosting out of `opi-agent`; only extend `opi-agent` where session state and extension hook contracts need generic runtime support.

**Tech Stack:** Rust 2024, Tokio, clap, serde/serde_json, toml, sha2/hex, existing `opi-agent` extension/session APIs, existing `opi-coding-agent` package discovery and adapter host APIs.

---

## Verified Findings

These findings were confirmed against source and behavior on 2026-06-09.

1. **P0: Installed packages are not connected to runtime startup.**
   - `start_adapters_from_packages()` is defined in `crates/opi-coding-agent/src/adapter_extension.rs` and used only in tests.
   - `CodingHarness::discover_resources()` scans configured/user/project package directories and `config.packages.paths`; it does not read `packages.toml`.
   - `main.rs`, `runner.rs`, and `rpc.rs` construct harnesses without starting adapters from installed declarations.

2. **P0: `opi package add/remove/list/doctor` is declaration-level.**
   - `cmd_add()` only parses the source string and writes a `PackageDeclaration`.
   - It does not require local paths to exist, parse `package.toml`, clone git sources, compute `manifest_sha256`, or write `package-lock.toml`.
   - Reproduction: in a temp directory, `opi package add .\missing -l` exited 0, wrote `.opi/packages.toml`, and did not write `.opi/package-lock.toml`.
   - `cmd_doctor()` parses `package.toml` as generic `toml::Value`. Reproduction: a manifest with `[adapter] kind = "grpc"` and wrong protocol returned `[]` with exit code 0.

3. **P1/P0 depending on design authority: adapter hook bridge is incomplete.**
   - `ProcessAdapter` implements `before_tool_call`, `after_tool_call`, events, commands, tools, and state.
   - `ProcessAdapter` does not implement `prepare_next_turn`.
   - `ExtensionRegistry::CompositeHooks::transform_context()` delegates only to base hooks. There is no extension surface for transform-context adapters.

4. **P1: adapter state is not persisted through session JSONL.**
   - `ExtensionRegistry::serialize_states()` and `restore_states()` exist and tests cover isolated registry state round trips.
   - `SessionEntry` only has `Message`, `Compaction`, and `Leaf`.
   - `SessionCoordinator` writes messages, compactions, and leaf pointers only. No production path calls `serialize_states()` or `restore_states()`.

5. **P1: adapter diagnostics are incomplete.**
   - `AdapterHost::send_event()` intentionally drops timeout/failure results without diagnostics.
   - `AdapterHost::shutdown_inner()` sends shutdown and immediately kills the child process.
   - Adapter startup diagnostics are produced by `start_adapters_from_packages()`, but production startup does not call it.

6. **P2: source/path hardening gaps are real.**
   - Local identity uses the raw path string, not a canonical absolute path.
   - `PackageSource::parse()` splits git refs with `rfind('@')`, so `git:ssh://git@github.com/user/repo` misparses.
   - `resolve_adapter_command()` joins relative paths without rejecting `..` escapes.

7. **P2: docs/release hygiene gaps are real.**
   - The design spec says packages are trusted code and docs/CLI must say so directly.
   - `README.md`, `README.zh.md`, `docs/opi-spec.md`, and `docs/opi-spec.zh.md` do not contain a direct trusted-code warning.
   - `CHANGELOG.md` has no Phase 5 entries for package CLI, manifest V2, adapter protocol, adapter host, adapter bridge, or example adapter packages.

## Success Criteria

- `opi package add ./examples/todo -l` validates the package, writes `.opi/packages.toml`, writes `.opi/package-lock.toml`, and prints package name/version/source/scope.
- A fresh harness startup from the same workspace discovers that declaration without `config.packages.paths`.
- Adapter packages start in interactive, non-interactive, and RPC modes, and their tools/commands/hooks are registered.
- `opi package doctor --json` validates source, lock, manifest V2, resources, opi version, adapter command resolution, and adapter handshake diagnostics.
- Adapter state survives session resume with a fresh adapter process.
- `prepare_next_turn` and `transform_context` adapter hooks either pass tests or the design docs are narrowed. This plan implements both.
- Event drop diagnostics, graceful shutdown, canonical local identity, SSH git parsing, and adapter command containment are covered by tests.
- README, README.zh.md, opi spec EN/ZH, and CHANGELOG match the implementation.

## File Structure

- Create `crates/opi-coding-agent/src/package_resolver.rs`
  - Shared installed-package resolver for CLI, runtime startup, and doctor.
  - Owns declaration merging, lock lookup, manifest parsing, manifest hashing, package-resource construction, adapter command diagnostics, and JSON-friendly diagnostics.

- Modify `crates/opi-coding-agent/src/lib.rs`
  - Export `package_resolver`.

- Modify `crates/opi-coding-agent/src/package_store.rs`
  - Add canonical local identity helper.
  - Fix git source parsing for SSH URLs with and without explicit refs.
  - Add git commit helper used by resolver install flow.

- Modify `crates/opi-coding-agent/src/package_cli.rs`
  - Replace declaration-only add/remove/list/doctor with resolver-backed lifecycle operations.
  - Make list/doctor cover global and project scopes.

- Modify `crates/opi-coding-agent/src/package_discovery.rs`
  - Expose a single-package discovery function.
  - Harden `resolve_adapter_command()` to reject relative path escapes.

- Modify `crates/opi-coding-agent/src/main.rs`, `runner.rs`, `rpc.rs`, and `harness.rs`
  - Run installed-package startup before harness construction in all run modes.
  - Inject `ExtensionRegistry`, installed package layers, and adapter diagnostics into `CodingHarness`.

- Modify `crates/opi-agent/src/session.rs`, `crates/opi-coding-agent/src/session_coordinator.rs`, `session_cli.rs`, and `harness.rs`
  - Add `extension_state` session entries, persist snapshots, restore latest active-branch state on resume, and preserve state on fork.

- Modify `crates/opi-agent/src/extension.rs`, `crates/opi-agent/src/hooks.rs`, and `crates/opi-coding-agent/src/adapter_extension.rs`
  - Add extension transform-context hook support.
  - Bridge `prepare_next_turn` and `transform_context` adapter hook messages.

- Modify `crates/opi-coding-agent/src/adapter_host.rs`
  - Add event drop diagnostics and a bounded graceful shutdown wait.

- Modify `crates/opi-coding-agent/tests/*.rs`
  - Add resolver, CLI lifecycle, startup E2E, session state, hook, diagnostic, and hardening tests.

- Modify `crates/opi-coding-agent/Cargo.toml` and add `crates/opi-coding-agent/examples/package_adapter_example.rs`
  - Make the example adapter usable outside test-only binaries.

- Modify `examples/todo/package.toml`, `examples/permission-gate/package.toml`, `examples/protected-paths/package.toml`
  - Point dev examples at the runnable Cargo example adapter.

- Modify `README.md`, `README.zh.md`, `docs/opi-spec.md`, `docs/opi-spec.zh.md`, `CHANGELOG.md`, and `crates/opi-coding-agent/tests/productized_packages_docs.rs`
  - Add trusted-code warnings, accurate package lifecycle language, and changelog coverage.

No commit steps are included because repository instructions say not to commit unless the user asks.

---

### Task 1: Add Resolver Contracts And Failing Tests

**Files:**
- Create: `crates/opi-coding-agent/src/package_resolver.rs`
- Modify: `crates/opi-coding-agent/src/lib.rs`
- Modify: `crates/opi-coding-agent/src/package_discovery.rs`
- Test: `crates/opi-coding-agent/tests/package_resolver.rs`

- [ ] **Step 1: Export a placeholder resolver module**

Add to `crates/opi-coding-agent/src/lib.rs`:

```rust
pub mod package_resolver;
```

Create `crates/opi-coding-agent/src/package_resolver.rs` with these public contracts:

```rust
use std::path::{Path, PathBuf};

use crate::package_discovery::PackageResource;
use crate::package_store::{PackageDeclaration, PackageIdentity, PackageLockEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledPackageScope {
    Global,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PackageDiagnostic {
    pub scope: InstalledPackageScope,
    pub source: String,
    pub severity: PackageDiagnosticSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedInstalledPackage {
    pub scope: InstalledPackageScope,
    pub declaration: PackageDeclaration,
    pub identity: PackageIdentity,
    pub lock: Option<PackageLockEntry>,
    pub package: PackageResource,
}

#[derive(Debug, Clone, Default)]
pub struct InstalledPackageResolution {
    pub packages: Vec<ResolvedInstalledPackage>,
    pub diagnostics: Vec<PackageDiagnostic>,
}

#[derive(Debug, thiserror::Error)]
pub enum PackageResolverError {
    #[error("package resolver failed: {0}")]
    Failed(String),
}

pub fn resolve_installed_packages(
    _workspace_root: &Path,
    _user_config_dir: &Path,
) -> Result<InstalledPackageResolution, PackageResolverError> {
    Ok(InstalledPackageResolution::default())
}
```

- [ ] **Step 2: Add a single-package discovery function contract**

In `crates/opi-coding-agent/src/package_discovery.rs`, add this public function near `discover_packages()`:

```rust
pub fn discover_package_root(
    path: &Path,
    layer_precedence: u32,
) -> Result<PackageResource, PackageDiscoveryError> {
    let layer = crate::resource::DiscoveryLayer {
        root: path.to_path_buf(),
        subdirectory: None,
        precedence: layer_precedence,
    };
    let mut seen = std::collections::HashMap::new();
    discover_package_dir(path, &layer, &mut seen)?;
    seen.into_values()
        .next()
        .ok_or_else(|| PackageDiscoveryError::InvalidManifest {
            path: path.join("package.toml"),
            reason: "package.toml did not produce a package resource".to_string(),
        })
}
```

- [ ] **Step 3: Write failing resolver tests**

Create `crates/opi-coding-agent/tests/package_resolver.rs`:

```rust
use std::fs;

use opi_coding_agent::package_resolver::{
    InstalledPackageScope, PackageDiagnosticSeverity, resolve_installed_packages,
};
use opi_coding_agent::package_store::{
    PackageDeclaration, PackageLockEntry, PackageStore, PackageStoreScope,
};
use tempfile::tempdir;

fn write_package(root: &std::path::Path, name: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("package.toml"),
        format!(
            "name = \"{name}\"\n\
             description = \"{name} package\"\n\
             version = \"0.1.0\"\n"
        ),
    )
    .unwrap();
}

#[test]
fn resolver_reads_project_package_declaration_as_package_resource() {
    let workspace = tempdir().unwrap();
    let user = tempdir().unwrap();
    let package_root = workspace.path().join("vendor/todo");
    write_package(&package_root, "todo");

    let store = PackageStore::project(workspace.path().to_path_buf());
    store
        .write_declarations(&[PackageDeclaration {
            source: ".\\vendor\\todo".to_string(),
            filters: Default::default(),
        }])
        .unwrap();
    store
        .write_lock(&[PackageLockEntry {
            identity_kind: "local".to_string(),
            identity_value: package_root.canonicalize().unwrap().display().to_string(),
            source: ".\\vendor\\todo".to_string(),
            package_root: package_root.canonicalize().unwrap(),
            cache_path: None,
            git_commit: None,
            manifest_sha256: opi_coding_agent::package_resolver::manifest_sha256(
                &package_root.join("package.toml"),
            )
            .unwrap(),
        }])
        .unwrap();

    let result = resolve_installed_packages(workspace.path(), user.path()).unwrap();

    assert_eq!(result.diagnostics, []);
    assert_eq!(result.packages.len(), 1);
    assert_eq!(result.packages[0].scope, InstalledPackageScope::Project);
    assert_eq!(result.packages[0].package.manifest.name, "todo");
}

#[test]
fn resolver_reports_missing_local_package_as_error() {
    let workspace = tempdir().unwrap();
    let user = tempdir().unwrap();
    let store = PackageStore::project(workspace.path().to_path_buf());
    store
        .write_declarations(&[PackageDeclaration {
            source: "./missing".to_string(),
            filters: Default::default(),
        }])
        .unwrap();

    let result = resolve_installed_packages(workspace.path(), user.path()).unwrap();

    assert_eq!(result.packages.len(), 0);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, PackageDiagnosticSeverity::Error);
    assert_eq!(result.diagnostics[0].code, "source_missing");
}

#[test]
fn resolver_prefers_project_package_over_global_package_with_same_manifest_name() {
    let workspace = tempdir().unwrap();
    let user = tempdir().unwrap();
    let global_root = user.path().join("global-todo");
    let project_root = workspace.path().join("project-todo");
    write_package(&global_root, "todo");
    write_package(&project_root, "todo");

    let global = PackageStore::global(user.path().to_path_buf());
    global
        .write_declarations(&[PackageDeclaration {
            source: global_root.display().to_string(),
            filters: Default::default(),
        }])
        .unwrap();
    global
        .write_lock(&[opi_coding_agent::package_resolver::local_lock_entry(
            global_root.display().to_string(),
            &global_root,
        )
        .unwrap()])
        .unwrap();

    let project = PackageStore::project(workspace.path().to_path_buf());
    project
        .write_declarations(&[PackageDeclaration {
            source: project_root.display().to_string(),
            filters: Default::default(),
        }])
        .unwrap();
    project
        .write_lock(&[opi_coding_agent::package_resolver::local_lock_entry(
            project_root.display().to_string(),
            &project_root,
        )
        .unwrap()])
        .unwrap();

    let result = resolve_installed_packages(workspace.path(), user.path()).unwrap();

    assert_eq!(result.packages.len(), 1);
    assert_eq!(result.packages[0].scope, InstalledPackageScope::Project);
    assert_eq!(result.packages[0].package.path, project_root.canonicalize().unwrap());
}
```

Expected before implementation: unresolved functions such as `manifest_sha256()` and `local_lock_entry()` or empty resolver behavior fail compilation/tests.

- [ ] **Step 4: Run the failing test**

Run:

```powershell
cargo test -p opi-coding-agent --test package_resolver
```

Expected: FAIL before the resolver is implemented.

- [ ] **Step 5: Implement resolver helpers**

In `package_resolver.rs`, implement:

```rust
pub fn manifest_sha256(path: &Path) -> Result<String, PackageResolverError> {
    let bytes = std::fs::read(path)
        .map_err(|e| PackageResolverError::Failed(format!("read {}: {e}", path.display())))?;
    use sha2::Digest as _;
    Ok(hex::encode(sha2::Sha256::digest(bytes)))
}

pub fn local_lock_entry(
    source: String,
    package_root: &Path,
) -> Result<PackageLockEntry, PackageResolverError> {
    let canonical = package_root.canonicalize().map_err(|e| {
        PackageResolverError::Failed(format!("canonicalize {}: {e}", package_root.display()))
    })?;
    Ok(PackageLockEntry {
        identity_kind: "local".to_string(),
        identity_value: canonical.display().to_string(),
        source,
        manifest_sha256: manifest_sha256(&canonical.join("package.toml"))?,
        package_root: canonical,
        cache_path: None,
        git_commit: None,
    })
}
```

- [ ] **Step 6: Implement local declaration resolution**

Resolver rules:

- Read global declarations from `PackageStore::global(user_config_dir.to_path_buf())`.
- Read project declarations from `PackageStore::project(workspace_root.to_path_buf())`.
- Resolve relative local sources against the scope base:
  - project scope: `workspace_root`
  - global scope: `user_config_dir`
- Require an existing package root and `package.toml`.
- Use `PackageManifest::from_toml()` through `discover_package_root()`.
- Check lock presence and lock manifest hash. Missing or drifted lock is an error diagnostic for runtime startup, not a panic.
- Sort final packages by scope precedence then package name. Project wins over global on manifest-name conflicts.

- [ ] **Step 7: Run resolver tests**

Run:

```powershell
cargo test -p opi-coding-agent --test package_resolver
```

Expected: PASS.

---

### Task 2: Upgrade Package CLI To Lifecycle Operations

**Files:**
- Modify: `crates/opi-coding-agent/src/package_cli.rs`
- Modify: `crates/opi-coding-agent/src/package_resolver.rs`
- Modify: `crates/opi-coding-agent/src/package_store.rs`
- Test: `crates/opi-coding-agent/tests/package_cli.rs`
- Test: `crates/opi-coding-agent/tests/package_store.rs`

- [ ] **Step 1: Add failing CLI tests for add validation and lock writes**

In `crates/opi-coding-agent/tests/package_cli.rs`, add:

```rust
#[test]
fn package_add_rejects_missing_local_package() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();

    let code = opi_coding_agent::package_cli::handle_package_command(
        &opi_coding_agent::cli::PackageCommand::Add {
            source: "./missing".to_string(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );

    assert_eq!(code, 2);
    assert!(!workspace.path().join(".opi/packages.toml").exists());
    assert!(!workspace.path().join(".opi/package-lock.toml").exists());
}

#[test]
fn package_add_local_writes_declaration_and_lock() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let pkg = workspace.path().join("vendor/todo");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(
        pkg.join("package.toml"),
        "name = \"todo\"\ndescription = \"Todo package\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let code = opi_coding_agent::package_cli::handle_package_command(
        &opi_coding_agent::cli::PackageCommand::Add {
            source: "./vendor/todo".to_string(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );

    assert_eq!(code, 0);
    let decls = opi_coding_agent::package_store::PackageStore::project(
        workspace.path().to_path_buf(),
    )
    .read_declarations()
    .unwrap();
    assert_eq!(decls[0].source, "./vendor/todo");

    let locks = opi_coding_agent::package_store::PackageStore::project(
        workspace.path().to_path_buf(),
    )
    .read_lock()
    .unwrap();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].identity_kind, "local");
    assert_eq!(locks[0].package_root, pkg.canonicalize().unwrap());
    assert!(!locks[0].manifest_sha256.is_empty());
}
```

- [ ] **Step 2: Add failing doctor tests for manifest V2 validation**

Add:

```rust
#[test]
fn package_doctor_rejects_manifest_v2_adapter_errors() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let pkg = workspace.path().join("badpkg");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(
        pkg.join("package.toml"),
        "name = \"badpkg\"\n\
         description = \"Bad package\"\n\
         version = \"0.1.0\"\n\
         [adapter]\n\
         kind = \"grpc\"\n\
         protocol = \"not-opi\"\n\
         command = \"bad\"\n",
    )
    .unwrap();

    let add_code = opi_coding_agent::package_cli::handle_package_command(
        &opi_coding_agent::cli::PackageCommand::Add {
            source: "./badpkg".to_string(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(add_code, 2);
}
```

Expected before implementation: the test currently observes add success for a TOML-valid but manifest-invalid package.

- [ ] **Step 3: Change `cmd_add()` to install**

Replace declaration-only `cmd_add(store, source)` with:

```rust
fn cmd_add(
    store: &PackageStore,
    scope: &PackageStoreScope,
    source: &str,
) -> Result<(), PackageStoreError>
```

Behavior:

- Parse `PackageSource`.
- For local sources:
  - Resolve relative paths against the scope base.
  - Canonicalize.
  - Require package root and `package.toml`.
  - Parse with `PackageManifest::from_toml()`.
  - Write declaration if identity is not already present.
  - Write or replace lock entry.
- For git sources:
  - Clone into `store.cache_dir()/sha256(identity)`.
  - Checkout explicit ref when provided.
  - Resolve current commit with `git rev-parse HEAD`.
  - Parse manifest and write lock with `git_commit`.
- Print one line:

```text
Installed <name> <version> from <source> (<scope>)
```

- [ ] **Step 4: Change `cmd_remove()` to support source or manifest name**

Rules:

- Match exact declaration source first.
- Otherwise resolve declarations enough to compare manifest names.
- If one match: remove declaration and matching lock entry.
- If multiple matches: return code 2 with an ambiguity diagnostic listing scope/source/name.
- If no match: keep exit code 0 and print the current "no declaration matching" message.

- [ ] **Step 5: Change `list` and `doctor` to global plus project**

Do not add `-l` to `list` or `doctor`; the design says both cover both scopes.

`list --json` should emit one JSON object per installed package with:

```json
{
  "scope": "project",
  "name": "todo",
  "version": "0.1.0",
  "source": "./vendor/todo",
  "package_root": "D:\\...",
  "adapter_command": "cargo",
  "adapter_resolved_command": "D:\\...",
  "diagnostics": []
}
```

Plain `list` should show columns: `scope`, `name`, `version`, `source`, `status`.

`doctor --json` should return one JSON array containing package rows and diagnostics:

```json
[
  {
    "scope": "project",
    "source": "./vendor/todo",
    "name": "todo",
    "status": "ok",
    "diagnostics": []
  }
]
```

Exit code:

- 0 when no error diagnostics exist.
- 2 when any error diagnostic exists.

- [ ] **Step 6: Run package CLI tests**

Run:

```powershell
cargo test -p opi-coding-agent --test package_cli -- --nocapture
cargo test -p opi-coding-agent --test package_store
```

Expected: PASS.

---

### Task 3: Wire Installed Packages Into Runtime Startup

**Files:**
- Modify: `crates/opi-coding-agent/src/package_resolver.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Modify: `crates/opi-coding-agent/src/runner.rs`
- Modify: `crates/opi-coding-agent/src/rpc.rs`
- Test: `crates/opi-coding-agent/tests/harness_resource_integration.rs`
- Test: `crates/opi-coding-agent/tests/package_runtime_startup.rs`

- [ ] **Step 1: Add runtime package startup type**

In `package_resolver.rs`, add:

```rust
#[derive(Debug, Clone)]
pub struct RuntimePackageStartup {
    pub packages: Vec<crate::package_discovery::PackageResource>,
    pub package_layers: crate::resource::ResourceDiscoveryLayers,
    pub diagnostics: Vec<String>,
}

pub fn runtime_package_startup(
    workspace_root: &Path,
    user_config_dir: &Path,
    config: &crate::config::OpiConfig,
) -> Result<RuntimePackageStartup, PackageResolverError> {
    let resolution = resolve_installed_packages(workspace_root, user_config_dir)?;
    let mut explicit = crate::resource::ExplicitResourcePaths {
        extensions: config.extensions.paths.clone(),
        packages: config.packages.paths.clone(),
        ..Default::default()
    };
    for resolved in &resolution.packages {
        explicit.packages.push(resolved.package.path.clone());
    }
    let layers = crate::resource::standard_discovery_layers(
        workspace_root,
        Some(user_config_dir),
        explicit,
    );
    Ok(RuntimePackageStartup {
        packages: resolution.packages.into_iter().map(|p| p.package).collect(),
        package_layers: layers,
        diagnostics: resolution
            .diagnostics
            .into_iter()
            .map(|d| format!("package {}: {}", d.source, d.message))
            .collect(),
    })
}
```

- [ ] **Step 2: Add harness builder support for extra diagnostics**

Add `extra_resource_diagnostics: Vec<String>` to `CodingHarnessBuilder` and `HarnessBuildOptions`.

Add builder method:

```rust
pub fn extra_resource_diagnostics(mut self, diagnostics: Vec<String>) -> Self {
    self.extra_resource_diagnostics = diagnostics;
    self
}
```

In `new_with_build_options()`, after resource discovery and model diagnostics:

```rust
resources
    .metadata
    .diagnostics
    .extend(build_options.extra_resource_diagnostics);
```

- [ ] **Step 3: Add async runtime package preparation helper**

In `main.rs`, add private helper:

```rust
async fn prepare_runtime_packages(
    workspace_root: &std::path::Path,
    config: &opi_coding_agent::config::OpiConfig,
) -> (
    opi_agent::extension::ExtensionRegistry,
    opi_coding_agent::resource::ResourceDiscoveryLayers,
    Vec<String>,
) {
    let user_config_dir = opi_coding_agent::config::user_config_dir();
    let startup = match opi_coding_agent::package_resolver::runtime_package_startup(
        workspace_root,
        &user_config_dir,
        config,
    ) {
        Ok(startup) => startup,
        Err(e) => {
            return (
                opi_agent::extension::ExtensionRegistry::new(),
                opi_coding_agent::resource::standard_discovery_layers(
                    workspace_root,
                    Some(&user_config_dir),
                    opi_coding_agent::resource::ExplicitResourcePaths::default(),
                ),
                vec![format!("package startup failed: {e}")],
            );
        }
    };
    let registry = opi_agent::extension::ExtensionRegistry::new();
    let (registry, adapter_diagnostics) =
        opi_coding_agent::adapter_extension::start_adapters_from_packages(
            &startup.packages,
            workspace_root,
            registry,
        )
        .await;
    let mut diagnostics = startup.diagnostics;
    diagnostics.extend(adapter_diagnostics);
    (registry, startup.package_layers, diagnostics)
}
```

- [ ] **Step 4: Use builder injection in interactive mode**

Replace `CodingHarness::new_with_hooks_and_resume_tool_config(...)` in `run_interactive()` with:

```rust
let (registry, resource_layers, package_diagnostics) =
    prepare_runtime_packages(&workspace_root, &config).await;
let harness = CodingHarness::builder(provider, config.defaults.model.clone(), config.clone(), workspace_root)
    .hooks(hooks)
    .initial_messages(initial_messages)
    .tool_config(tool_config)
    .resource_layers(resource_layers)
    .extension_registry(registry)
    .extra_resource_diagnostics(package_diagnostics)
    .build();
```

When `resume_info` is `Some`, call `.resume(resume_info)` before `.build()`.

- [ ] **Step 5: Add non-interactive and RPC constructors that accept package startup**

Add to `NonInteractiveRunner`:

```rust
pub fn new_with_resume_and_runtime_packages(
    provider: Box<dyn Provider>,
    model: String,
    config: OpiConfig,
    workspace_root: PathBuf,
    allow_mutating: bool,
    user_system_prompt: Option<String>,
    initial_messages: Vec<AgentMessage>,
    resume_info: Option<ResumeInfo>,
    tool_selection: ToolSelection,
    extension_registry: ExtensionRegistry,
    resource_layers: ResourceDiscoveryLayers,
    package_diagnostics: Vec<String>,
) -> Result<Self, ToolPolicyError>
```

Build the harness with `CodingHarness::builder(...)` and the same injected values.

Add equivalent parameters to `RpcRunner::new()` or create `RpcRunner::new_with_runtime_packages(...)`.

- [ ] **Step 6: Add runtime startup tests**

Create `crates/opi-coding-agent/tests/package_runtime_startup.rs`:

```rust
use opi_agent::extension::ExtensionRegistry;
use opi_coding_agent::adapter_extension::start_adapters_from_packages;
use opi_coding_agent::package_resolver::{local_lock_entry, runtime_package_startup};
use opi_coding_agent::package_store::{PackageDeclaration, PackageStore};
use tempfile::tempdir;

#[tokio::test]
async fn installed_project_adapter_starts_without_configured_package_paths() {
    let workspace = tempdir().unwrap();
    let user = tempdir().unwrap();
    let package_root = workspace.path().join("pkg");
    std::fs::create_dir_all(&package_root).unwrap();
    std::fs::write(
        package_root.join("package.toml"),
        format!(
            "name = \"gate\"\n\
             description = \"Gate package\"\n\
             version = \"0.1.0\"\n\
             [adapter]\n\
             kind = \"process-jsonl\"\n\
             command = \"{}\"\n\
             args = [\"gate\"]\n\
             protocol = \"opi-extension-jsonl-v1\"\n",
            opi_coding_agent::test_support::adapter_host_mock_path().display()
        ),
    )
    .unwrap();

    let store = PackageStore::project(workspace.path().to_path_buf());
    store
        .write_declarations(&[PackageDeclaration {
            source: package_root.display().to_string(),
            filters: Default::default(),
        }])
        .unwrap();
    store
        .write_lock(&[local_lock_entry(package_root.display().to_string(), &package_root).unwrap()])
        .unwrap();

    let config = opi_coding_agent::config::OpiConfig::default();
    let startup = runtime_package_startup(workspace.path(), user.path(), &config).unwrap();
    let (registry, diagnostics) =
        start_adapters_from_packages(&startup.packages, workspace.path(), ExtensionRegistry::new())
            .await;

    assert_eq!(diagnostics, Vec::<String>::new());
    assert!(registry.names().contains(&"gate"));
    assert!(registry.collect_tools().iter().any(|tool| tool.definition().name == "echo"));
}
```

Create `crates/opi-coding-agent/tests/support/mod.rs` with an `adapter_host_mock_path()` helper copied from the existing adapter tests, then add `mod support;` to `package_runtime_startup.rs` and call `support::adapter_host_mock_path()`.

- [ ] **Step 7: Run startup tests**

Run:

```powershell
cargo test -p opi-coding-agent --test package_runtime_startup -- --nocapture
cargo test -p opi-coding-agent --test harness_resource_integration -- --nocapture
cargo test -p opi-coding-agent --test package_cli -- --nocapture
```

Expected: PASS.

---

### Task 4: Make Example Adapter Packages Runnable End To End

**Files:**
- Create: `crates/opi-coding-agent/examples/package_adapter_example.rs`
- Modify: `crates/opi-coding-agent/Cargo.toml`
- Modify: `crates/opi-coding-agent/tests/package_adapter_example.rs`
- Modify: `examples/todo/package.toml`
- Modify: `examples/permission-gate/package.toml`
- Modify: `examples/protected-paths/package.toml`
- Test: `crates/opi-coding-agent/tests/example_adapters.rs`
- Test: `crates/opi-coding-agent/tests/package_runtime_startup.rs`

- [ ] **Step 1: Move the example adapter implementation to a Cargo example**

Copy the contents of `crates/opi-coding-agent/tests/package_adapter_example.rs` into:

```text
crates/opi-coding-agent/examples/package_adapter_example.rs
```

Update the module comment to:

```rust
//! Development example adapter binary for todo, permission-gate, and protected-paths packages.
//!
//! Run with:
//! cargo run -p opi-coding-agent --example package_adapter_example -- todo
```

- [ ] **Step 2: Keep tests using the same implementation**

Replace `crates/opi-coding-agent/tests/package_adapter_example.rs` with:

```rust
include!("../examples/package_adapter_example.rs");
```

This keeps the existing `[[test]]` target and all test binary lookup helpers working.

- [ ] **Step 3: Update example package manifests**

Change each adapter package manifest:

`examples/todo/package.toml`:

```toml
[adapter]
kind = "process-jsonl"
command = "cargo"
args = ["run", "-p", "opi-coding-agent", "--example", "package_adapter_example", "--", "todo"]
protocol = "opi-extension-jsonl-v1"
timeout_ms = 30000
```

`examples/permission-gate/package.toml`:

```toml
[adapter]
kind = "process-jsonl"
command = "cargo"
args = ["run", "-p", "opi-coding-agent", "--example", "package_adapter_example", "--", "permission-gate"]
protocol = "opi-extension-jsonl-v1"
timeout_ms = 30000
```

`examples/protected-paths/package.toml`:

```toml
[adapter]
kind = "process-jsonl"
command = "cargo"
args = ["run", "-p", "opi-coding-agent", "--example", "package_adapter_example", "--", "protected-paths"]
protocol = "opi-extension-jsonl-v1"
timeout_ms = 30000
```

- [ ] **Step 4: Add an end-to-end example install/start test**

In `package_runtime_startup.rs`, add:

```rust
#[tokio::test]
async fn examples_todo_can_be_installed_and_started_from_declaration() {
    let workspace = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let user = tempfile::tempdir().unwrap();

    let code = opi_coding_agent::package_cli::handle_package_command(
        &opi_coding_agent::cli::PackageCommand::Add {
            source: "./examples/todo".to_string(),
            local: true,
        },
        workspace.clone(),
        user.path().to_path_buf(),
    );
    assert_eq!(code, 0);

    let config = opi_coding_agent::config::OpiConfig::default();
    let startup =
        opi_coding_agent::package_resolver::runtime_package_startup(&workspace, user.path(), &config)
            .unwrap();
    assert!(startup.packages.iter().any(|p| p.manifest.name == "todo"));
}
```

Use an isolated project root instead of the actual repository if the test would modify repo `.opi/`; copying `examples/todo` into a temp workspace is safer:

```rust
fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
```

- [ ] **Step 5: Run example tests**

Run:

```powershell
cargo test -p opi-coding-agent --test example_adapters -- --nocapture
cargo test -p opi-coding-agent --test package_runtime_startup -- --nocapture
cargo run -p opi-coding-agent --example package_adapter_example -- todo
```

Expected:

- Tests pass.
- The manual `cargo run --example` process waits for JSONL input; stop it with Ctrl+C after confirming it starts.

---

### Task 5: Persist Adapter State Through Sessions

**Files:**
- Modify: `crates/opi-agent/src/session.rs`
- Modify: `crates/opi-coding-agent/src/session_coordinator.rs`
- Modify: `crates/opi-coding-agent/src/session_cli.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Test: `crates/opi-coding-agent/tests/session_extension_state.rs`
- Test: `crates/opi-agent/tests/session.rs`

- [ ] **Step 1: Add failing session state tests**

Create `crates/opi-coding-agent/tests/session_extension_state.rs`:

```rust
use opi_agent::session::{ExtensionStateEntry, SessionEntry, SessionHeader, SessionReader, SessionWriter};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn extension_state_entries_do_not_reconstruct_as_agent_messages() {
    let entries = vec![
        SessionEntry::ExtensionState(ExtensionStateEntry {
            id: "state-1".to_string(),
            parent_id: Some("msg-1".to_string()),
            timestamp: "2026-06-09T00:00:00Z".to_string(),
            state: json!({"todo": {"items": []}}),
        }),
    ];

    let messages = opi_coding_agent::session_cli::reconstruct_context(&entries);

    assert!(messages.is_empty());
}

#[test]
fn session_coordinator_restores_latest_extension_state_for_active_branch() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    let header = SessionHeader::new(
        "s".to_string(),
        "2026-06-09T00:00:00Z".to_string(),
        dir.path().display().to_string(),
    );
    let mut writer = SessionWriter::create(&path, header).unwrap();
    writer
        .append(&SessionEntry::ExtensionState(ExtensionStateEntry {
            id: "state-1".to_string(),
            parent_id: Some("msg-1".to_string()),
            timestamp: "2026-06-09T00:00:00Z".to_string(),
            state: json!({"todo": {"items": [{"id": "todo-1"}]}}),
        }))
        .unwrap();
    drop(writer);

    let (_, entries) = SessionReader::read_all(&path).unwrap();
    let state = opi_coding_agent::session_coordinator::latest_extension_state(&entries);

    assert_eq!(state.unwrap()["todo"]["items"][0]["id"], "todo-1");
}
```

Expected before implementation: compilation fails because `ExtensionStateEntry` and `latest_extension_state()` do not exist.

- [ ] **Step 2: Add session entry type**

In `crates/opi-agent/src/session.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionStateEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub timestamp: String,
    pub state: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEntry {
    Message(MessageEntry),
    Compaction(CompactionEntry),
    Leaf(LeafEntry),
    ExtensionState(ExtensionStateEntry),
}
```

Update `entry_id()`:

```rust
SessionEntry::ExtensionState(s) => &s.id,
```

- [ ] **Step 3: Keep extension state out of message branch content**

In `session_cli.rs`:

- `select_ordered_entries()` should continue to exclude `Leaf`.
- `walk_active_branch()` should not add `ExtensionState` entries to `by_id`.
- `apply_entries()` should ignore `ExtensionState`.
- `fork_session()` should append the latest active-branch extension state after copied content entries.

Add:

```rust
pub(crate) fn latest_extension_state_for_active_branch(
    entries: &[SessionEntry],
) -> Option<serde_json::Value>
```

Rules:

- Determine active content ids through `select_ordered_entries()`.
- Choose the last `ExtensionState` whose `parent_id` is one of those content ids.
- If there are no leaf/content ids, choose the last `ExtensionState` in file order.

- [ ] **Step 4: Persist state at turn end**

In `SessionCoordinator`, add:

```rust
pub fn append_extension_state(
    &mut self,
    state: serde_json::Value,
) -> Result<(), std::io::Error> {
    let entry = SessionEntry::ExtensionState(ExtensionStateEntry {
        id: format!("state-{}", ENTRY_SEQ.fetch_add(1, Ordering::Relaxed)),
        parent_id: self.active_tip_entry_id.clone(),
        timestamp: now_iso(),
        state,
    });
    self.writer.append(&entry)
}
```

In `CodingHarness::persist_turn()`, after message persistence and compaction:

```rust
if let (Some(session), Some(registry)) = (&mut self.session, &self.extension_registry) {
    match registry.serialize_states() {
        Ok(state) if state.as_object().is_some_and(|m| !m.is_empty()) => {
            if let Err(e) = session.append_extension_state(state) {
                self.agent.emit_event(AgentEvent::SessionPersistError {
                    message: format!("extension state write failed: {e}"),
                });
            }
        }
        Ok(_) => {}
        Err(e) => self.agent.emit_event(AgentEvent::SessionPersistError {
            message: format!("extension state serialize failed: {e}"),
        }),
    }
}
```

- [ ] **Step 5: Restore state after adapter startup**

In harness construction after `extension_registry` is active and after `SessionCoordinator::open_existing()` succeeds:

```rust
if let (Some(registry), Some(info)) = (active_extension_registry.as_ref(), resume_info_ref) {
    if let Some(state) = crate::session_coordinator::latest_extension_state(&info.entries) {
        if let Err(e) = registry.restore_states(state) {
            resources.metadata.diagnostics.push(format!("extension state restore failed: {e}"));
        }
    }
}
```

Preserve `resume` data before it is moved into `SessionCoordinator::open_existing()`.

- [ ] **Step 6: Add adapter resume integration test**

In `session_extension_state.rs`, add an async test that:

- Starts the todo adapter.
- Dispatches `todo/add`.
- Calls `registry.serialize_states()`.
- Writes an `ExtensionState` entry.
- Starts a fresh adapter process.
- Calls `restore_states()` from the session entry.
- Calls `todo/list`.
- Asserts the item exists.

Use existing `package_adapter_example` test binary lookup code from `example_adapters.rs`.

- [ ] **Step 7: Run session tests**

Run:

```powershell
cargo test -p opi-agent --test session
cargo test -p opi-coding-agent --test session_extension_state -- --nocapture
cargo test -p opi-coding-agent --test adapter_runtime -- --nocapture
```

Expected: PASS.

---

### Task 6: Bridge Missing Adapter Hooks

**Files:**
- Modify: `crates/opi-agent/src/extension.rs`
- Modify: `crates/opi-coding-agent/src/adapter_extension.rs`
- Modify: `crates/opi-coding-agent/tests/adapter_host_mock.rs`
- Test: `crates/opi-coding-agent/tests/adapter_runtime.rs`

- [ ] **Step 1: Add failing prepare-next-turn adapter test**

In `adapter_runtime.rs`, add:

```rust
#[tokio::test]
async fn adapter_prepare_next_turn_can_inject_message() {
    let (adapter, _host) = start_mock_process_adapter("prepare").await;
    let update = adapter
        .prepare_next_turn(&opi_agent::hooks::PrepareNextTurnContext {
            messages: vec![],
            turn: 1,
        })
        .await
        .expect("update");

    assert_eq!(update.extra_messages.len(), 1);
}
```

Add a mock mode or capability that advertises `hooks: ["prepare_next_turn"]` and returns:

```json
{
  "type": "hook_result",
  "id": "<same>",
  "action": "continue",
  "data": {
    "extra_messages": [
      {
        "type": "Custom",
        "kind": "adapter_note",
        "data": {"text": "next turn"},
        "include_in_llm_context": false
      }
    ]
  }
}
```

- [ ] **Step 2: Implement `ProcessAdapter::prepare_next_turn()`**

In `adapter_extension.rs`, add to `impl Extension for ProcessAdapter`:

```rust
fn prepare_next_turn(
    &self,
    ctx: &PrepareNextTurnContext,
) -> Pin<Box<dyn Future<Output = Option<AgentLoopTurnUpdate>> + Send>> {
    if !self.hooks.contains("prepare_next_turn") {
        return Box::pin(async { None });
    }
    let id = self.host.next_id();
    let host = self.host.clone();
    let payload = serde_json::json!({
        "turn": ctx.turn,
        "messages": ctx.messages,
    });
    Box::pin(async move {
        let request = AdapterHostMessage::Hook {
            id,
            hook: "prepare_next_turn".to_string(),
            payload,
        };
        match host.send_request(request, REQUEST_TIMEOUT).await.ok()? {
            AdapterProcessMessage::HookResult { data, .. } => {
                let extra_messages = data
                    .and_then(|d| serde_json::from_value(d["extra_messages"].clone()).ok())
                    .unwrap_or_default();
                Some(AgentLoopTurnUpdate { extra_messages })
            }
            _ => None,
        }
    })
}
```

Use actual imports for `Pin`, `Future`, `PrepareNextTurnContext`, `AgentLoopTurnUpdate`, and `AdapterProcessMessage`.

- [ ] **Step 3: Add transform-context extension surface**

In `opi-agent/src/extension.rs`, add a default method to `Extension`:

```rust
fn transform_context(
    &self,
    messages: Vec<AgentMessage>,
) -> Pin<Box<dyn Future<Output = Result<Vec<AgentMessage>, ExtensionError>> + Send>> {
    Box::pin(async move { Ok(messages) })
}
```

In `CompositeHooks::transform_context()`, call base first, then extensions in registration order:

```rust
let base = self.base.clone();
let extensions = self.extensions.clone();
Box::pin(async move {
    let mut messages = base.transform_context(messages, signal).await?;
    for ext in extensions.iter() {
        messages = ext.transform_context(messages).await?;
    }
    Ok(messages)
})
```

- [ ] **Step 4: Implement adapter transform-context**

In `ProcessAdapter`:

- Skip if `hooks` does not contain `"transform_context"`.
- Send hook payload `{ "messages": messages }`.
- Expect `data.messages` to deserialize as `Vec<AgentMessage>`.
- On adapter error or timeout, return `ExtensionError::HookError`. Add this variant to `crates/opi-agent/src/extension.rs`:

```rust
#[error("extension hook error in {name}: {reason}")]
HookError { name: String, reason: String }
```

- [ ] **Step 5: Run hook tests**

Run:

```powershell
cargo test -p opi-agent --test extensions
cargo test -p opi-coding-agent --test adapter_runtime -- --nocapture
```

Expected: PASS.

---

### Task 7: Add Adapter Diagnostics And Graceful Shutdown

**Files:**
- Modify: `crates/opi-coding-agent/src/adapter_host.rs`
- Modify: `crates/opi-coding-agent/src/adapter_extension.rs`
- Test: `crates/opi-coding-agent/tests/adapter_host.rs`

- [ ] **Step 1: Add diagnostics storage to AdapterHost**

Add field:

```rust
diagnostics: Arc<std::sync::Mutex<Vec<String>>>,
```

Add method:

```rust
pub fn take_diagnostics(&self) -> Vec<String> {
    std::mem::take(&mut *self.diagnostics.lock().unwrap())
}
```

- [ ] **Step 2: Record event drop diagnostics**

In `send_event()`:

```rust
match write_result {
    Ok(Some(())) => {}
    Ok(None) => self.record_diagnostic("event delivery failed"),
    Err(_) => self.record_diagnostic("event delivery timed out after 100ms"),
}
```

Add private:

```rust
fn record_diagnostic(&self, message: impl Into<String>) {
    self.diagnostics.lock().unwrap().push(message.into());
}
```

- [ ] **Step 3: Add graceful shutdown wait**

After sending shutdown:

```rust
if let Some(ref mut child) = self.child {
    match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
        Ok(_) => {}
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
}
```

Keep `Drop::start_kill()` unchanged because `Drop` cannot await.

- [ ] **Step 4: Surface adapter host diagnostics through startup**

In `start_adapters_from_packages()`, after a successful host start and before wrapping it in `Arc`, take diagnostics after startup only if present:

```rust
let startup_host_diagnostics = host.take_diagnostics();
for diagnostic in startup_host_diagnostics {
    diagnostics.push(format!("package '{}': {diagnostic}", package.manifest.name));
}
```

For ongoing event drops, `take_diagnostics()` is the retrieval API. `start_adapters_from_packages()` captures diagnostics produced during startup, and tests call `take_diagnostics()` directly for post-start event-drop behavior.

- [ ] **Step 5: Add tests**

In `adapter_host.rs` tests:

- `event_drop_records_diagnostic` uses a mock adapter mode that stops reading stdin after handshake, sends many events, then asserts `take_diagnostics()` is not empty.
- `shutdown_waits_for_child_exit_before_kill` uses a mock mode that exits on shutdown and writes a marker file. Assert marker file exists after `host.shutdown("test").await`.

- [ ] **Step 6: Run host tests**

Run:

```powershell
cargo test -p opi-coding-agent --test adapter_host -- --nocapture
```

Expected: PASS.

---

### Task 8: Harden Source Identity, SSH Git Parsing, And Adapter Command Paths

**Files:**
- Modify: `crates/opi-coding-agent/src/package_store.rs`
- Modify: `crates/opi-coding-agent/src/package_discovery.rs`
- Modify: `crates/opi-coding-agent/src/package_resolver.rs`
- Test: `crates/opi-coding-agent/tests/package_store.rs`
- Test: `crates/opi-coding-agent/tests/package_manifest_v2.rs`

- [ ] **Step 1: Add source parser tests**

In `package_store.rs` tests:

```rust
#[test]
fn ssh_git_source_without_ref_does_not_split_username_at() {
    let source = PackageSource::parse("git:ssh://git@github.com/user/repo").unwrap();
    assert_eq!(
        source,
        PackageSource::Git {
            url: "ssh://git@github.com/user/repo".to_string(),
            refspec: None,
        }
    );
}

#[test]
fn ssh_git_source_with_ref_splits_last_ref_separator_after_path() {
    let source = PackageSource::parse("git:ssh://git@github.com/user/repo@main").unwrap();
    assert_eq!(
        source,
        PackageSource::Git {
            url: "ssh://git@github.com/user/repo".to_string(),
            refspec: Some("main".to_string()),
        }
    );
}
```

- [ ] **Step 2: Fix git parsing**

Replace `rfind('@')` with logic that does not treat the SSH username separator as a ref separator:

- If rest starts with `ssh://`, only split an `@ref` after the final `/`.
- If rest starts with `http://` or `https://`, split an `@ref` after the final `/`.
- If rest starts with `github.com/`, split an `@ref` after the repo path.
- Reject `git:git@host:path@ref` until scp-like syntax is deliberately specified, with message:

```text
scp-like git sources are not supported; use git:ssh://git@host/path@ref
```

- [ ] **Step 3: Canonicalize local identity in resolver**

Do not change `PackageSource::identity_key()` to perform filesystem I/O. Add resolver helper:

```rust
pub fn source_identity_for_resolution(
    source: &PackageSource,
    base: &Path,
) -> Result<PackageIdentity, PackageResolverError>
```

For local sources, join relative paths to `base`, canonicalize, and return that path string.

- [ ] **Step 4: Add adapter command escape tests**

In `package_manifest_v2.rs`:

```rust
#[test]
fn adapter_relative_command_cannot_escape_package_root() {
    let pkg_dir = tempfile::tempdir().unwrap();
    let adapter = AdapterManifest {
        kind: "process-jsonl".to_string(),
        command: "../escape".to_string(),
        args: vec![],
        protocol: "opi-extension-jsonl-v1".to_string(),
        timeout_ms: None,
    };

    let err = resolve_adapter_command_checked(&adapter, pkg_dir.path()).unwrap_err();

    assert!(err.to_string().contains("escapes package root"));
}
```

- [ ] **Step 5: Add checked adapter command resolver**

Keep existing `resolve_adapter_command()` only if callers/tests need the infallible API. Add:

```rust
pub fn resolve_adapter_command_checked(
    adapter: &AdapterManifest,
    package_root: &Path,
) -> Result<PathBuf, PackageDiscoveryError>
```

Rules:

- Absolute path: return as-is.
- Bare name: return as-is for PATH lookup.
- Relative with separators: normalize `package_root.join(command)`.
- Reject if normalized path does not start with normalized package root.

Update `start_adapters_from_packages()` and `doctor` to use checked resolver.

- [ ] **Step 6: Run hardening tests**

Run:

```powershell
cargo test -p opi-coding-agent --test package_store
cargo test -p opi-coding-agent --test package_manifest_v2
cargo test -p opi-coding-agent --test harness_resource_integration -- --nocapture
```

Expected: PASS.

---

### Task 9: Fix Documentation, Guards, And Changelog

**Files:**
- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `CHANGELOG.md`
- Modify: `crates/opi-coding-agent/tests/productized_packages_docs.rs`

- [ ] **Step 1: Add docs guard tests**

In `productized_packages_docs.rs`, add tests that assert:

```rust
assert!(readme.contains("Packages are trusted code"));
assert!(readme.contains("not sandboxed"));
assert!(spec.contains("Packages are trusted code"));
assert!(changelog.contains("opi package add/remove/list/doctor"));
assert!(changelog.contains("opi-extension-jsonl-v1"));
```

Add matching Chinese assertions using stable phrases:

```rust
assert!(readme_zh.contains("Package 是受信任代码"));
assert!(readme_zh.contains("不会被 sandbox"));
```

- [ ] **Step 2: Update README EN/ZH**

In package section, add:

```markdown
Packages are trusted code. Installing a package can run adapter child processes with the same OS privileges as `opi`; Phase 5 does not sandbox package code or enforce package permission declarations.
```

Chinese counterpart:

```markdown
Package 是受信任代码。安装 package 后，其 adapter 子进程会以与 `opi` 相同的操作系统权限运行；第五阶段不会 sandbox package 代码，也不会执行 package 权限声明。
```

- [ ] **Step 3: Update opi spec EN/ZH**

Update Phase 5 sections to say:

- `opi package add` validates manifests and writes lock entries.
- Runtime startup reads declarations and lock state.
- `doctor` validates source, lock, manifest, resource, and adapter diagnostics.
- Packages are trusted code and not sandboxed.

Keep EN/ZH sections synchronized.

- [ ] **Step 4: Add CHANGELOG entries under Unreleased**

Add under `## [Unreleased]`:

```markdown
### Added

- `opi package add/remove/list/doctor` now validates package manifests, writes lock entries, and reports installed package diagnostics.
- Manifest V2 supports `[adapter]` process adapters with the `opi-extension-jsonl-v1` JSONL protocol.
- Installed package declarations are loaded during runtime startup so adapter tools, commands, hooks, events, state, and cancellation bridge into the extension API.
- Example adapter packages demonstrate todo state, permission gates, and protected path hooks through a runnable process adapter.

### Fixed

- `opi package doctor` now rejects invalid manifest V2 adapter declarations and reports lock/source/resource/adapter diagnostics.
- Adapter state snapshots are persisted in session JSONL and restored on resume.
- Adapter event drops are diagnostic-visible, shutdown allows a bounded graceful exit, local package identity is canonicalized, SSH git source parsing is URL-aware, and relative adapter commands cannot escape package roots.
```

- [ ] **Step 5: Run docs tests**

Run:

```powershell
cargo test -p opi-coding-agent --test productized_packages_docs
```

Expected: PASS.

---

### Task 10: Final Verification Gates

**Files:**
- All files modified in Tasks 1-9.

- [ ] **Step 1: Run focused Phase 5 tests**

Run:

```powershell
cargo test -p opi-coding-agent --test package_store
cargo test -p opi-coding-agent --test package_cli -- --nocapture
cargo test -p opi-coding-agent --test package_resolver
cargo test -p opi-coding-agent --test package_manifest_v2
cargo test -p opi-coding-agent --test package_discovery
cargo test -p opi-coding-agent --test adapter_protocol
cargo test -p opi-coding-agent --test adapter_host -- --nocapture
cargo test -p opi-coding-agent --test adapter_runtime -- --nocapture
cargo test -p opi-coding-agent --test harness_resource_integration -- --nocapture
cargo test -p opi-coding-agent --test package_runtime_startup -- --nocapture
cargo test -p opi-coding-agent --test session_extension_state -- --nocapture
cargo test -p opi-coding-agent --test example_adapters -- --nocapture
cargo test -p opi-coding-agent --test productized_packages_docs
```

Expected: all PASS.

- [ ] **Step 2: Run workspace gates**

Run:

```powershell
cargo fmt --check --all
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
$env:RUSTDOCFLAGS="-D warnings"; cargo doc --workspace --no-deps
powershell -ExecutionPolicy Bypass -File scripts/opi-impl-smoke.ps1
```

Expected: all PASS.

- [ ] **Step 3: Manual product-loop smoke**

Use a temporary workspace copy so the repository `.opi/` is not modified:

```powershell
$tmp = Join-Path $env:TEMP ("opi-phase5-smoke-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tmp | Out-Null
Copy-Item -Recurse .\examples $tmp\examples
Push-Location $tmp
& D:\Luiz\Odradek\opi\target\debug\opi.exe package add .\examples\todo -l
Test-Path .\.opi\packages.toml
Test-Path .\.opi\package-lock.toml
& D:\Luiz\Odradek\opi\target\debug\opi.exe package doctor --json
Pop-Location
```

Expected:

- `package add` exits 0.
- Both TOML files exist.
- `doctor --json` exits 0 and reports `todo` with `status = ok`.

- [ ] **Step 4: Re-audit exit criteria**

Create a short audit note in `docs/snapshots/phase5/` only if the user asks to update snapshots. Minimum content:

```markdown
# Phase 5 Remediation Verification

- Product loop: PASS
- CLI lifecycle: PASS
- Runtime startup: PASS
- Adapter state persistence: PASS
- Missing hooks: PASS
- Diagnostics/hardening: PASS
- Docs/changelog: PASS
```

Do not modify existing snapshot files unless explicitly requested.
