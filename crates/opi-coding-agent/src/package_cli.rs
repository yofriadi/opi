//! Package CLI command execution.
//!
//! Handles `opi package add/remove/list/doctor` subcommands. Runs before
//! provider construction so no API keys or network access are needed.
//!
//! # Unstable
//!
//! This module is part of the **unstable 0.x extension API**. Breaking changes
//! may occur between minor versions without a major version bump.

use std::io::Write;
use std::path::PathBuf;

use crate::cli::PackageCommand;
use crate::package_store::{
    PackageDeclaration, PackageSource, PackageStore, PackageStoreError, PackageStoreScope,
};

/// Execute a package CLI command and return an exit code.
///
/// `workspace_root` is typically `std::env::current_dir()`.
/// `user_config_dir` is the platform-specific user config directory
/// (see [`crate::config::user_config_dir`]).
pub fn handle_package_command(
    command: &PackageCommand,
    workspace_root: PathBuf,
    user_config_dir: PathBuf,
) -> i32 {
    let scope = resolve_scope(command, workspace_root, user_config_dir);
    let store = PackageStore::new(scope.clone());
    match run_command(command, &store, &scope) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("opi package: {e}");
            2
        }
    }
}

/// Determine the store scope from the command's `local` flag.
///
/// - `add`/`remove` with `-l` → project scope
/// - `add`/`remove` without `-l` → global scope
/// - `list`/`doctor` → always project scope (they read, not write, and show
///   what's installed for the current workspace)
fn resolve_scope(
    command: &PackageCommand,
    workspace_root: PathBuf,
    user_config_dir: PathBuf,
) -> PackageStoreScope {
    match command {
        PackageCommand::Add { local, .. } | PackageCommand::Remove { local, .. } if *local => {
            PackageStoreScope::Project { workspace_root }
        }
        PackageCommand::Add { .. } | PackageCommand::Remove { .. } => {
            PackageStoreScope::Global { user_config_dir }
        }
        // list and doctor always read from project scope
        PackageCommand::List { .. } | PackageCommand::Doctor { .. } => {
            PackageStoreScope::Project { workspace_root }
        }
    }
}

/// Dispatch the command to the appropriate store operation.
fn run_command(
    command: &PackageCommand,
    store: &PackageStore,
    scope: &PackageStoreScope,
) -> Result<(), PackageStoreError> {
    match command {
        PackageCommand::Add { source, .. } => cmd_add(store, source),
        PackageCommand::Remove { name_or_source, .. } => cmd_remove(store, name_or_source),
        PackageCommand::List { json } => cmd_list(store, *json),
        PackageCommand::Doctor { json } => cmd_doctor(store, *json, scope),
    }
}

/// Add a package declaration to the store.
///
/// Validates the source string via [`PackageSource::parse`] before persisting.
/// Idempotent: if a declaration with the same source already exists, this is
/// a no-op.
fn cmd_add(store: &PackageStore, source: &str) -> Result<(), PackageStoreError> {
    // Validate the source string before persisting.
    let _ = PackageSource::parse(source)?;
    let mut decls = store.read_declarations()?;
    // Idempotent check
    if decls.iter().any(|d| d.source == source) {
        return Ok(());
    }
    decls.push(PackageDeclaration {
        source: source.to_string(),
        filters: Default::default(),
    });
    store.write_declarations(&decls)
}

/// Remove a package declaration matching the given source or name.
fn cmd_remove(store: &PackageStore, name_or_source: &str) -> Result<(), PackageStoreError> {
    let mut decls = store.read_declarations()?;
    let before = decls.len();
    decls.retain(|d| d.source != name_or_source);
    if decls.len() == before {
        eprintln!("opi package: no declaration matching '{name_or_source}'");
    }
    store.write_declarations(&decls)
}

/// List installed packages. Outputs a table by default, or NDJSON with `--json`.
fn cmd_list(store: &PackageStore, json: bool) -> Result<(), PackageStoreError> {
    let decls = store.read_declarations()?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if json {
        for decl in &decls {
            let line = serde_json::json!({
                "source": decl.source,
            });
            writeln!(out, "{line}").map_err(PackageStoreError::Io)?;
        }
    } else if decls.is_empty() {
        writeln!(out, "No packages installed.").map_err(PackageStoreError::Io)?;
    } else {
        for decl in &decls {
            writeln!(out, "{}", decl.source).map_err(PackageStoreError::Io)?;
        }
    }
    Ok(())
}

/// Validate installed packages and report diagnostics.
///
/// Checks that each declared source resolves to a readable directory with a
/// valid `package.toml` manifest. Returns `Err` with a summary if any
/// diagnostics were found, so the CLI exits with a non-zero code.
fn cmd_doctor(
    store: &PackageStore,
    json: bool,
    scope: &PackageStoreScope,
) -> Result<(), PackageStoreError> {
    let decls = store.read_declarations()?;
    let mut diagnostics = Vec::new();

    let base_path: PathBuf = match scope {
        PackageStoreScope::Project { workspace_root } => workspace_root.clone(),
        PackageStoreScope::Global { user_config_dir } => user_config_dir.clone(),
    };

    for decl in &decls {
        let resolved = base_path.join(&decl.source);
        if !resolved.exists() {
            diagnostics.push(Diagnostic {
                source: decl.source.clone(),
                status: "missing".into(),
                reason: format!("package root not found: {}", resolved.display()),
            });
            continue;
        }
        let manifest_path = resolved.join("package.toml");
        if !manifest_path.exists() {
            diagnostics.push(Diagnostic {
                source: decl.source.clone(),
                status: "no_manifest".into(),
                reason: "package.toml not found in package root".into(),
            });
            continue;
        }
        // Try to parse the manifest
        match std::fs::read_to_string(&manifest_path) {
            Ok(content) => {
                if let Err(e) = toml::from_str::<toml::Value>(&content) {
                    diagnostics.push(Diagnostic {
                        source: decl.source.clone(),
                        status: "invalid_manifest".into(),
                        reason: format!("package.toml parse error: {e}"),
                    });
                }
                // Valid — no diagnostic needed
            }
            Err(e) => {
                diagnostics.push(Diagnostic {
                    source: decl.source.clone(),
                    status: "unreadable_manifest".into(),
                    reason: format!("cannot read package.toml: {e}"),
                });
            }
        }
    }

    if json {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let json_diag: Vec<serde_json::Value> = diagnostics
            .iter()
            .map(|d| {
                serde_json::json!({
                    "source": d.source,
                    "status": d.status,
                    "reason": d.reason,
                })
            })
            .collect();
        writeln!(
            out,
            "{}",
            serde_json::to_string(&json_diag).unwrap_or_else(|_| "[]".into())
        )
        .map_err(PackageStoreError::Io)?;
    } else if diagnostics.is_empty() {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        if decls.is_empty() {
            writeln!(out, "No packages installed.").map_err(PackageStoreError::Io)?;
        } else {
            writeln!(out, "All {} package(s) OK.", decls.len()).map_err(PackageStoreError::Io)?;
        }
    } else {
        for d in &diagnostics {
            eprintln!("{}: {} ({})", d.source, d.status, d.reason);
        }
    }

    // Return error if diagnostics were found, so the exit code is non-zero.
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(PackageStoreError::Git(format!(
            "{} diagnostic(s) found",
            diagnostics.len()
        )))
    }
}

/// A diagnostic entry from `package doctor`.
struct Diagnostic {
    source: String,
    status: String,
    reason: String,
}
