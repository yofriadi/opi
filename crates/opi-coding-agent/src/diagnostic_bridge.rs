//! Diagnostic bridges for opi-coding-agent-owned error families (Phase 7 task 7.2).
//!
//! `opi-agent` owns the shared [`opi_agent::Diagnostic`] model, and
//! `opi-coding-agent` depends on `opi-agent`. The package and config error
//! families live here, so they cannot host `impl From<&E> for Diagnostic`
//! without violating the orphan rule (both `From` and `Diagnostic` are foreign).
//! These free functions perform the mapping instead.
//!
//! The package layer already produces structured `PackageDiagnostic` values
//! (with a dynamic `code: String`). Because `Diagnostic.code` is a `&'static
//! str`, the granular package code is preserved in `details.package_code` and
//! the shared diagnostic carries a stable `package_diagnostic` code.

use opi_agent::diagnostic::code::*;
use opi_agent::diagnostic::{Diagnostic, SOURCE_CONFIG, SOURCE_PACKAGE, Severity};

use crate::config::ConfigError;
use crate::package_resolver::{
    InstalledPackageScope, PackageDiagnostic, PackageDiagnosticSeverity,
};

/// Map a [`PackageDiagnostic`] into the shared [`Diagnostic`] vocabulary.
///
/// Severity is mapped directly. The package's granular `code`, `source`, and
/// `scope` are carried in `details` so the shared `code` stays a stable literal.
pub fn diagnostic_from_package(pd: &PackageDiagnostic) -> Diagnostic {
    let severity = match pd.severity {
        PackageDiagnosticSeverity::Info => Severity::Info,
        PackageDiagnosticSeverity::Warning => Severity::Warning,
        PackageDiagnosticSeverity::Error => Severity::Error,
    };
    let scope = match pd.scope {
        InstalledPackageScope::Global => "global",
        InstalledPackageScope::Project => "project",
    };
    Diagnostic::new(
        severity,
        CODE_PACKAGE_DIAGNOSTIC,
        SOURCE_PACKAGE,
        pd.message.clone(),
    )
    .details(serde_json::json!({
        "package_code": pd.code,
        "package_source": pd.source,
        "scope": scope,
    }))
}

/// Map a [`ConfigError`] into the shared [`Diagnostic`] vocabulary.
///
/// Both variants are startup-fatal, so they are `Error`. The config path is
/// placed in `details` (redactable in summary mode) rather than the message.
pub fn diagnostic_from_config(err: &ConfigError) -> Diagnostic {
    match err {
        ConfigError::Parse { path, .. } => Diagnostic::new(
            Severity::Error,
            CODE_CONFIG_PARSE_FAILED,
            SOURCE_CONFIG,
            "failed to parse config file",
        )
        .details(serde_json::json!({ "path": path.display().to_string() }))
        .action("fix the malformed TOML or remove the file to use defaults"),
        ConfigError::Read { path, .. } => Diagnostic::new(
            Severity::Error,
            CODE_CONFIG_READ_FAILED,
            SOURCE_CONFIG,
            "failed to read config file",
        )
        .details(serde_json::json!({ "path": path.display().to_string() }))
        .action("check file permissions or remove the file to use defaults"),
    }
}
