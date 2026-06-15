//! Runtime startup for installed package declarations.

use std::path::Path;

use opi_agent::extension::ExtensionRegistry;

use crate::adapter_extension::start_adapters_from_packages;
use crate::package_discovery::PackageResource;
use crate::package_resolver::{PackageDiagnostic, resolve_installed_packages};

/// Installed packages and adapter registry prepared before harness startup.
pub struct RuntimePackageStartup {
    pub extension_registry: ExtensionRegistry,
    pub installed_packages: Vec<PackageResource>,
    pub diagnostics: Vec<String>,
}

/// Resolve installed package declarations and start package adapters.
pub async fn start_installed_package_runtime(
    workspace_root: &Path,
    user_config_dir: &Path,
) -> RuntimePackageStartup {
    let registry = ExtensionRegistry::new();
    let mut diagnostics = Vec::new();
    let resolution = match resolve_installed_packages(workspace_root, user_config_dir) {
        Ok(resolution) => resolution,
        Err(e) => {
            diagnostics.push(format!("installed package resolution failed: {e}"));
            return RuntimePackageStartup {
                extension_registry: registry,
                installed_packages: Vec::new(),
                diagnostics,
            };
        }
    };

    diagnostics.extend(
        resolution
            .diagnostics
            .iter()
            .map(format_installed_package_diagnostic),
    );
    let installed_packages = resolution
        .packages
        .into_iter()
        .map(|package| package.package)
        .collect::<Vec<_>>();
    let (extension_registry, adapter_diagnostics) =
        start_adapters_from_packages(&installed_packages, workspace_root, registry).await;
    diagnostics.extend(adapter_diagnostics);

    RuntimePackageStartup {
        extension_registry,
        installed_packages,
        diagnostics,
    }
}

fn format_installed_package_diagnostic(diagnostic: &PackageDiagnostic) -> String {
    format!(
        "installed package {:?} {}: {} ({})",
        diagnostic.scope, diagnostic.source, diagnostic.code, diagnostic.message
    )
}
