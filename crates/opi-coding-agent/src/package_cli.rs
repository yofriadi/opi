//! Package CLI command execution.
//!
//! Handles `opi package add/remove/list/doctor` subcommands. Runs before
//! provider construction so no API keys are needed for local package commands.
//!
//! # Unstable
//!
//! This module is part of the **unstable 0.x extension API**. Breaking changes
//! may occur between minor versions without a major version bump.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::cli::PackageCommand;
use crate::package_discovery::{PackageManifest, resolve_adapter_command_checked};
use crate::package_resolver::{
    InstalledPackageResolution, InstalledPackageScope, PackageDiagnostic,
    PackageDiagnosticSeverity, ResolvedInstalledPackage, git_lock_entry, local_lock_entry,
    resolve_installed_packages, resolve_local_source_path,
};
use crate::package_store::{
    PackageDeclaration, PackageLockEntry, PackageSource, PackageStore, PackageStoreError,
    PackageStoreScope, PendingCacheReplacement,
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
    let scope = resolve_scope(command, workspace_root.clone(), user_config_dir.clone());
    let store = PackageStore::new(scope.clone());
    match run_command(command, &store, &scope, &workspace_root, &user_config_dir) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("opi package: {e}");
            2
        }
    }
}

/// Determine the write scope from the command's `local` flag.
///
/// `list` and `doctor` use the project scope as a placeholder here; their
/// handlers read both global and project package stores.
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
        PackageCommand::List { .. } | PackageCommand::Doctor { .. } => {
            PackageStoreScope::Project { workspace_root }
        }
    }
}

fn run_command(
    command: &PackageCommand,
    store: &PackageStore,
    scope: &PackageStoreScope,
    workspace_root: &Path,
    user_config_dir: &Path,
) -> Result<(), PackageStoreError> {
    match command {
        PackageCommand::Add { source, .. } => cmd_add(store, scope, source),
        PackageCommand::Remove { name_or_source, .. } => cmd_remove(store, scope, name_or_source),
        PackageCommand::List { json } => cmd_list(workspace_root, user_config_dir, *json),
        PackageCommand::Doctor { json } => cmd_doctor(workspace_root, user_config_dir, *json),
    }
}

fn cmd_add(
    store: &PackageStore,
    scope: &PackageStoreScope,
    source: &str,
) -> Result<(), PackageStoreError> {
    match PackageSource::parse(source)? {
        PackageSource::Local { path } => install_local_package(store, scope, source, path),
        PackageSource::Git { url, refspec } => {
            install_git_package(store, scope, source, url, refspec)
        }
    }
}

fn install_local_package(
    store: &PackageStore,
    scope: &PackageStoreScope,
    source: &str,
    path: PathBuf,
) -> Result<(), PackageStoreError> {
    let source_root = resolve_local_source_path(scope_base(scope), source, path);
    if !source_root.is_dir() {
        return Err(PackageStoreError::Package(format!(
            "package root not found: {}",
            source_root.display()
        )));
    }

    let canonical_root = source_root.canonicalize()?;
    let manifest_path = canonical_root.join("package.toml");
    if !manifest_path.is_file() {
        return Err(PackageStoreError::Package(format!(
            "package.toml not found in package root: {}",
            canonical_root.display()
        )));
    }

    let manifest = read_package_manifest(&manifest_path)?;
    let lock_entry = local_lock_entry(source.to_string(), &canonical_root)
        .map_err(|e| PackageStoreError::Package(e.to_string()))?;

    write_declaration_if_missing(store, scope, source, &lock_entry)?;
    write_or_replace_lock(store, lock_entry)?;
    println!(
        "Installed {} {} from {} ({})",
        manifest.name,
        display_version(manifest.version.as_deref()),
        source,
        scope_label(scope)
    );
    Ok(())
}

fn install_git_package(
    store: &PackageStore,
    scope: &PackageStoreScope,
    source: &str,
    url: String,
    refspec: Option<String>,
) -> Result<(), PackageStoreError> {
    let cache_dir = store.cache_dir().join(sha256_hex(&format!("git:{url}")));
    let staging_dir = store.git_clone_to_staging(&url, refspec.as_deref(), &cache_dir)?;
    let metadata = read_package_metadata_snapshot(store, scope)?;

    let validated = (|| {
        let manifest_path = staging_dir.join("package.toml");
        if !manifest_path.is_file() {
            return Err(PackageStoreError::Package(format!(
                "package.toml not found in package root: {}",
                staging_dir.display()
            )));
        }
        let manifest = read_package_manifest(&manifest_path)?;
        let git_commit = store.git_rev_parse_head(&staging_dir)?;
        Ok((manifest, git_commit))
    })();

    let (manifest, git_commit) = match validated {
        Ok(validated) => validated,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(e);
        }
    };

    let replacement = store.stage_cache_replacement(&cache_dir, &staging_dir)?;
    if !cache_dir.join("package.toml").is_file() {
        return Err(rollback_cache_on_error(
            replacement,
            PackageStoreError::Package(format!(
                "package.toml not found in package root: {}",
                cache_dir.display()
            )),
        ));
    }

    let lock_entry =
        match git_lock_entry(source.to_string(), url, &cache_dir, &cache_dir, git_commit) {
            Ok(entry) => entry,
            Err(e) => {
                return Err(rollback_cache_on_error(
                    replacement,
                    PackageStoreError::Package(e.to_string()),
                ));
            }
        };

    let declarations =
        declarations_with_package(metadata.declarations.clone(), scope, source, &lock_entry);
    let locks = locks_with_package(metadata.locks.clone(), &lock_entry);
    if let Err(e) = write_package_metadata(store, &declarations, &locks) {
        let metadata_restore = metadata.restore(store);
        let cache_restore = replacement.rollback();
        return Err(metadata_update_error(e, metadata_restore, cache_restore));
    }

    replacement.commit();
    println!(
        "Installed {} {} from {} ({})",
        manifest.name,
        display_version(manifest.version.as_deref()),
        source,
        scope_label(scope)
    );
    Ok(())
}

fn cmd_remove(
    store: &PackageStore,
    scope: &PackageStoreScope,
    name_or_source: &str,
) -> Result<(), PackageStoreError> {
    let mut decls = store.read_declarations()?;
    if let Some(index) = decls.iter().position(|d| d.source == name_or_source) {
        let removed = decls.remove(index);
        store.write_declarations(&decls)?;
        remove_locks_for_declaration(store, scope, &removed)?;
        return Ok(());
    }

    let matches = declarations_matching_manifest_name(store, scope, &decls, name_or_source)?;
    match matches.as_slice() {
        [] => {
            eprintln!("opi package: no declaration matching '{name_or_source}'");
            Ok(())
        }
        [matched] => {
            let removed = decls.remove(matched.index);
            store.write_declarations(&decls)?;
            remove_locks_for_declaration(store, scope, &removed)
        }
        _ => Err(PackageStoreError::Package(format!(
            "ambiguous package '{name_or_source}'; matches: {}",
            matches
                .iter()
                .map(|m| format!("{} source={} name={}", scope_label(scope), m.source, m.name))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn cmd_list(
    workspace_root: &Path,
    user_config_dir: &Path,
    json: bool,
) -> Result<(), PackageStoreError> {
    let resolution = resolve_installed_packages(workspace_root, user_config_dir)
        .map_err(|e| PackageStoreError::Package(e.to_string()))?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if json {
        for package in &resolution.packages {
            writeln!(out, "{}", list_package_json(package, &[])).map_err(PackageStoreError::Io)?;
        }
        for diagnostic in &resolution.diagnostics {
            writeln!(out, "{}", list_diagnostic_json(diagnostic)).map_err(PackageStoreError::Io)?;
        }
    } else if resolution.packages.is_empty() && resolution.diagnostics.is_empty() {
        writeln!(out, "No packages installed.").map_err(PackageStoreError::Io)?;
    } else {
        writeln!(out, "scope\tname\tversion\tsource\tstatus").map_err(PackageStoreError::Io)?;
        for package in &resolution.packages {
            writeln!(
                out,
                "{}\t{}\t{}\t{}\tok",
                installed_scope_label(package.scope),
                package.package.manifest.name,
                display_version(package.package.manifest.version.as_deref()),
                package.declaration.source
            )
            .map_err(PackageStoreError::Io)?;
        }
        for diagnostic in &resolution.diagnostics {
            writeln!(
                out,
                "{}\t-\t-\t{}\t{}",
                installed_scope_label(diagnostic.scope),
                diagnostic.source,
                severity_label(&diagnostic.severity)
            )
            .map_err(PackageStoreError::Io)?;
        }
    }
    Ok(())
}

fn cmd_doctor(
    workspace_root: &Path,
    user_config_dir: &Path,
    json: bool,
) -> Result<(), PackageStoreError> {
    let resolution = resolve_installed_packages(workspace_root, user_config_dir)
        .map_err(|e| PackageStoreError::Package(e.to_string()))?;
    let has_errors = resolution
        .diagnostics
        .iter()
        .any(|d| d.severity == PackageDiagnosticSeverity::Error);

    if json {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        writeln!(
            out,
            "{}",
            serde_json::Value::Array(doctor_rows(&resolution))
        )
        .map_err(PackageStoreError::Io)?;
    } else if resolution.diagnostics.is_empty() {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        if resolution.packages.is_empty() {
            writeln!(out, "No packages installed.").map_err(PackageStoreError::Io)?;
        } else {
            writeln!(out, "All {} package(s) OK.", resolution.packages.len())
                .map_err(PackageStoreError::Io)?;
        }
    } else {
        for diagnostic in &resolution.diagnostics {
            eprintln!(
                "{}: {} ({})",
                diagnostic.source, diagnostic.code, diagnostic.message
            );
        }
    }

    if has_errors {
        return Err(PackageStoreError::Package(format!(
            "{} diagnostic(s) found",
            resolution.diagnostics.len()
        )));
    }

    Ok(())
}

fn write_declaration_if_missing(
    store: &PackageStore,
    scope: &PackageStoreScope,
    source: &str,
    lock_entry: &PackageLockEntry,
) -> Result<(), PackageStoreError> {
    let decls = declarations_with_package(store.read_declarations()?, scope, source, lock_entry);
    store.write_declarations(&decls)
}

fn declarations_with_package(
    mut decls: Vec<PackageDeclaration>,
    scope: &PackageStoreScope,
    source: &str,
    lock_entry: &PackageLockEntry,
) -> Vec<PackageDeclaration> {
    let mut changed = false;
    for decl in &mut decls {
        if decl.source == source {
            return decls;
        }
        if declaration_identity(scope, decl).is_some_and(|(kind, value)| {
            kind == lock_entry.identity_kind && value == lock_entry.identity_value
        }) {
            decl.source = source.to_string();
            changed = true;
            break;
        }
    }

    if !changed {
        decls.push(PackageDeclaration {
            source: source.to_string(),
            filters: Default::default(),
        });
    }

    decls
}

fn write_or_replace_lock(
    store: &PackageStore,
    lock_entry: PackageLockEntry,
) -> Result<(), PackageStoreError> {
    let locks = locks_with_package(store.read_lock()?, &lock_entry);
    store.write_lock(&locks)
}

fn locks_with_package(
    mut locks: Vec<PackageLockEntry>,
    lock_entry: &PackageLockEntry,
) -> Vec<PackageLockEntry> {
    locks.retain(|lock| !lock_matches_entry(lock, lock_entry));
    locks.push(lock_entry.clone());
    locks
}

fn write_package_metadata(
    store: &PackageStore,
    declarations: &[PackageDeclaration],
    locks: &[PackageLockEntry],
) -> Result<(), PackageStoreError> {
    store.write_declarations(declarations)?;
    store.write_lock(locks)
}

fn remove_locks_for_declaration(
    store: &PackageStore,
    scope: &PackageStoreScope,
    declaration: &PackageDeclaration,
) -> Result<(), PackageStoreError> {
    let identity = declaration_identity(scope, declaration);
    let mut locks = store.read_lock()?;
    let before = locks.len();
    locks.retain(|lock| {
        if lock.source == declaration.source {
            return false;
        }
        if let Some((kind, value)) = &identity {
            return !(lock.identity_kind == *kind && lock.identity_value == *value);
        }
        true
    });
    if locks.len() != before {
        store.write_lock(&locks)?;
    }
    Ok(())
}

fn declarations_matching_manifest_name(
    store: &PackageStore,
    scope: &PackageStoreScope,
    declarations: &[PackageDeclaration],
    name: &str,
) -> Result<Vec<RemoveMatch>, PackageStoreError> {
    let locks = store.read_lock()?;
    let mut matches = Vec::new();
    for (index, declaration) in declarations.iter().enumerate() {
        if let Some(manifest_name) = declaration_manifest_name(scope, declaration, &locks)?
            && manifest_name == name
        {
            matches.push(RemoveMatch {
                index,
                source: declaration.source.clone(),
                name: manifest_name,
            });
        }
    }
    Ok(matches)
}

fn declaration_manifest_name(
    scope: &PackageStoreScope,
    declaration: &PackageDeclaration,
    locks: &[PackageLockEntry],
) -> Result<Option<String>, PackageStoreError> {
    let source = match PackageSource::parse(&declaration.source) {
        Ok(source) => source,
        Err(_) => return Ok(None),
    };
    let manifest_path = match source {
        PackageSource::Local { path } => {
            let source_root =
                resolve_local_source_path(scope_base(scope), &declaration.source, path);
            let Ok(canonical_root) = source_root.canonicalize() else {
                return Ok(None);
            };
            canonical_root.join("package.toml")
        }
        PackageSource::Git { url, .. } => {
            let Some(lock) = locks.iter().find(|lock| {
                lock.identity_kind == "git"
                    && (lock.source == declaration.source || lock.identity_value == url)
            }) else {
                return Ok(None);
            };
            lock.package_root.join("package.toml")
        }
    };

    if !manifest_path.is_file() {
        return Ok(None);
    }

    Ok(Some(read_package_manifest(&manifest_path)?.name))
}

fn declaration_identity(
    scope: &PackageStoreScope,
    declaration: &PackageDeclaration,
) -> Option<(String, String)> {
    let source = PackageSource::parse(&declaration.source).ok()?;
    match source {
        PackageSource::Local { path } => {
            let source_root =
                resolve_local_source_path(scope_base(scope), &declaration.source, path);
            let canonical_root = source_root.canonicalize().ok()?;
            Some(("local".to_string(), canonical_root.display().to_string()))
        }
        PackageSource::Git { url, .. } => Some(("git".to_string(), url)),
    }
}

fn lock_matches_entry(lock: &PackageLockEntry, entry: &PackageLockEntry) -> bool {
    lock.source == entry.source
        || (lock.identity_kind == entry.identity_kind
            && lock.identity_value == entry.identity_value)
}

fn read_package_manifest(path: &Path) -> Result<PackageManifest, PackageStoreError> {
    let content = std::fs::read_to_string(path)?;
    PackageManifest::from_toml(&content, path)
        .map_err(|e| PackageStoreError::Package(e.to_string()))
}

fn list_package_json(
    package: &ResolvedInstalledPackage,
    diagnostics: &[PackageDiagnostic],
) -> serde_json::Value {
    let adapter = package.package.manifest.adapter.as_ref();
    serde_json::json!({
        "scope": installed_scope_label(package.scope),
        "name": package.package.manifest.name.as_str(),
        "version": package.package.manifest.version.as_deref(),
        "source": package.declaration.source.as_str(),
        "status": "ok",
        "package_root": package.package.path.display().to_string(),
        "adapter_command": adapter.map(|adapter| adapter.command.as_str()),
        "adapter_resolved_command": adapter
            .and_then(|adapter| resolve_adapter_command_checked(adapter, &package.package.path).ok())
            .map(|path| path.display().to_string()),
        "diagnostics": diagnostics.iter().map(diagnostic_json).collect::<Vec<_>>(),
    })
}

fn list_diagnostic_json(diagnostic: &PackageDiagnostic) -> serde_json::Value {
    serde_json::json!({
        "scope": installed_scope_label(diagnostic.scope),
        "name": serde_json::Value::Null,
        "version": serde_json::Value::Null,
        "source": diagnostic.source.as_str(),
        "status": severity_label(&diagnostic.severity),
        "package_root": serde_json::Value::Null,
        "adapter_command": serde_json::Value::Null,
        "adapter_resolved_command": serde_json::Value::Null,
        "diagnostics": [diagnostic_json(diagnostic)],
    })
}

fn doctor_rows(resolution: &InstalledPackageResolution) -> Vec<serde_json::Value> {
    let mut rows = Vec::new();
    for package in &resolution.packages {
        rows.push(serde_json::json!({
            "scope": installed_scope_label(package.scope),
            "source": package.declaration.source.as_str(),
            "name": package.package.manifest.name.as_str(),
            "status": "ok",
            "diagnostics": [],
        }));
    }
    for diagnostic in &resolution.diagnostics {
        rows.push(serde_json::json!({
            "scope": installed_scope_label(diagnostic.scope),
            "source": diagnostic.source.as_str(),
            "name": serde_json::Value::Null,
            "status": severity_label(&diagnostic.severity),
            "diagnostics": [diagnostic_json(diagnostic)],
        }));
    }
    rows
}

fn diagnostic_json(diagnostic: &PackageDiagnostic) -> serde_json::Value {
    serde_json::json!({
        "severity": severity_label(&diagnostic.severity),
        "code": diagnostic.code.as_str(),
        "message": diagnostic.message.as_str(),
    })
}

fn scope_base(scope: &PackageStoreScope) -> &Path {
    match scope {
        PackageStoreScope::Project { workspace_root } => workspace_root,
        PackageStoreScope::Global { user_config_dir } => user_config_dir,
    }
}

fn scope_label(scope: &PackageStoreScope) -> &'static str {
    match scope {
        PackageStoreScope::Project { .. } => "project",
        PackageStoreScope::Global { .. } => "global",
    }
}

fn installed_scope_label(scope: InstalledPackageScope) -> &'static str {
    match scope {
        InstalledPackageScope::Project => "project",
        InstalledPackageScope::Global => "global",
    }
}

fn severity_label(severity: &PackageDiagnosticSeverity) -> &'static str {
    match severity {
        PackageDiagnosticSeverity::Info => "info",
        PackageDiagnosticSeverity::Warning => "warning",
        PackageDiagnosticSeverity::Error => "error",
    }
}

fn display_version(version: Option<&str>) -> &str {
    version.unwrap_or("-")
}

fn sha256_hex(input: &str) -> String {
    use sha2::Digest as _;
    format!("{:x}", sha2::Sha256::digest(input.as_bytes()))
}

struct RemoveMatch {
    index: usize,
    source: String,
    name: String,
}

struct PackageMetadataSnapshot {
    declarations: Vec<PackageDeclaration>,
    locks: Vec<PackageLockEntry>,
    declarations_path: PathBuf,
    lock_path: PathBuf,
    declarations_existed: bool,
    lock_existed: bool,
}

impl PackageMetadataSnapshot {
    fn restore(&self, store: &PackageStore) -> Result<(), PackageStoreError> {
        restore_package_file(&self.declarations_path, self.declarations_existed, || {
            store.write_declarations(&self.declarations)
        })?;
        restore_package_file(&self.lock_path, self.lock_existed, || {
            store.write_lock(&self.locks)
        })
    }
}

fn read_package_metadata_snapshot(
    store: &PackageStore,
    scope: &PackageStoreScope,
) -> Result<PackageMetadataSnapshot, PackageStoreError> {
    let declarations_path = scope.config_path();
    let lock_path = scope.lock_path();
    Ok(PackageMetadataSnapshot {
        declarations: store.read_declarations()?,
        locks: store.read_lock()?,
        declarations_existed: declarations_path.exists(),
        lock_existed: lock_path.exists(),
        declarations_path,
        lock_path,
    })
}

fn restore_package_file(
    path: &Path,
    existed: bool,
    write_existing: impl FnOnce() -> Result<(), PackageStoreError>,
) -> Result<(), PackageStoreError> {
    if existed {
        write_existing()
    } else {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(PackageStoreError::Io(e)),
        }
    }
}

fn rollback_cache_on_error(
    replacement: PendingCacheReplacement,
    error: PackageStoreError,
) -> PackageStoreError {
    match replacement.rollback() {
        Ok(()) => error,
        Err(rollback) => PackageStoreError::Package(format!(
            "{error}; cache rollback failed after package install error: {rollback}"
        )),
    }
}

fn metadata_update_error(
    error: PackageStoreError,
    metadata_restore: Result<(), PackageStoreError>,
    cache_restore: Result<(), PackageStoreError>,
) -> PackageStoreError {
    if metadata_restore.is_ok() && cache_restore.is_ok() {
        return error;
    }

    let mut details = vec![format!("{error}")];
    if let Err(e) = metadata_restore {
        details.push(format!("metadata rollback failed: {e}"));
    }
    if let Err(e) = cache_restore {
        details.push(format!("cache rollback failed: {e}"));
    }
    PackageStoreError::Package(format!(
        "package metadata update failed after cache replacement: {}",
        details.join("; ")
    ))
}
