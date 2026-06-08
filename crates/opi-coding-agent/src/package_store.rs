//! Package store and source model.
//!
//! Handles package source parsing (local paths and git URLs), declaration
//! files (`packages.toml`), lock files (`package-lock.toml`), and the store
//! operations that read/write them.
//!
//! # Store Scopes
//!
//! - **Global**: user-level config directory (`~/.config/opi/` on Unix,
//!   `%APPDATA%\opi\` on Windows).
//! - **Project**: workspace-local `.opi/` directory.
//!
//! # File Format
//!
//! `packages.toml` — user-facing declaration of desired packages:
//!
//! ```toml
//! [[package]]
//! source = "./vendor/todo"
//!
//! [[package]]
//! source = "git:github.com/user/repo@v1"
//! filters = { extensions = ["my-ext"] }
//! ```
//!
//! `package-lock.toml` — machine-generated lock state:
//!
//! ```toml
//! [[lock]]
//! identity_kind = "local"
//! identity_value = "/abs/path/to/pkg"
//! source = "./vendor/todo"
//! package_root = "/abs/path/to/pkg"
//! manifest_sha256 = "abcdef..."
//! ```
//!
//! # Unstable
//!
//! This module is part of the **unstable 0.x extension API**. Breaking changes
//! may occur between minor versions without a major version bump.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from package store operations.
#[derive(Debug, thiserror::Error)]
pub enum PackageStoreError {
    /// A package source string could not be parsed.
    #[error("invalid package source '{input}': {reason}")]
    InvalidSource { input: String, reason: String },
    /// An I/O error occurred during store operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A TOML deserialization error.
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
    /// A TOML serialization error.
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    /// A git operation failed.
    #[error("git operation failed: {0}")]
    Git(String),
}

// ---------------------------------------------------------------------------
// Source types
// ---------------------------------------------------------------------------

/// A parsed package source: either a local path or a git URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSource {
    /// A local filesystem path (relative or absolute).
    Local { path: PathBuf },
    /// A git repository URL with an optional ref spec.
    Git {
        url: String,
        refspec: Option<String>,
    },
}

impl PackageSource {
    /// Parse a source string into a [`PackageSource`].
    ///
    /// Supported formats:
    /// - `./relative/path` or `/absolute/path` or `.\relative\path` or `D:\path` — local
    /// - `git:github.com/user/repo@ref` — GitHub shorthand (becomes `https://github.com/user/repo`)
    /// - `git:https://example.com/repo.git@ref` — full git URL with optional ref
    pub fn parse(raw: &str) -> Result<Self, PackageStoreError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PackageStoreError::InvalidSource {
                input: raw.into(),
                reason: "source string is empty".into(),
            });
        }

        if let Some(rest) = trimmed.strip_prefix("git:") {
            // Git source
            let (url, refspec) = if let Some(at_pos) = rest.rfind('@') {
                (
                    rest[..at_pos].to_string(),
                    Some(rest[at_pos + 1..].to_string()),
                )
            } else {
                (rest.to_string(), None)
            };

            if url.is_empty() {
                return Err(PackageStoreError::InvalidSource {
                    input: raw.into(),
                    reason: "git URL is empty after 'git:' prefix".into(),
                });
            }

            // Expand GitHub shorthand: no scheme and no "://" → prepend https://
            let expanded_url = if url.contains("://") {
                url
            } else if url.starts_with("github.com/") {
                format!("https://{url}")
            } else {
                return Err(PackageStoreError::InvalidSource {
                    input: raw.into(),
                    reason: format!(
                        "unrecognized git source '{url}': must be a URL with scheme or \
                         github.com shorthand"
                    ),
                });
            };

            Ok(PackageSource::Git {
                url: expanded_url,
                refspec,
            })
        } else if is_local_path(trimmed) {
            Ok(PackageSource::Local {
                path: PathBuf::from(trimmed),
            })
        } else {
            Err(PackageStoreError::InvalidSource {
                input: raw.into(),
                reason: "unrecognized source format: must start with './', '/', a drive letter, \
                     or 'git:' prefix"
                    .to_string(),
            })
        }
    }

    /// Return an identity key for this source.
    ///
    /// The `kind` field is `"local"` or `"git"`. The `value` field is the path
    /// or URL respectively.
    pub fn identity_key(&self) -> PackageIdentity {
        match self {
            PackageSource::Local { path } => PackageIdentity {
                kind: "local".into(),
                value: path.display().to_string(),
            },
            PackageSource::Git { url, .. } => PackageIdentity {
                kind: "git".into(),
                value: url.clone(),
            },
        }
    }
}

/// An identity key derived from a package source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageIdentity {
    /// Source kind: `"local"` or `"git"`.
    pub kind: String,
    /// Source value: path for local, URL for git.
    pub value: String,
}

// ---------------------------------------------------------------------------
// Store scope
// ---------------------------------------------------------------------------

/// The scope of a package store: global (user config) or project (workspace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageStoreScope {
    /// User-level global store rooted in the config directory.
    Global { user_config_dir: PathBuf },
    /// Project-level store rooted in the workspace `.opi/` directory.
    Project { workspace_root: PathBuf },
}

impl PackageStoreScope {
    /// Path to the `packages.toml` declaration file.
    pub fn config_path(&self) -> PathBuf {
        match self {
            PackageStoreScope::Global { user_config_dir } => user_config_dir.join("packages.toml"),
            PackageStoreScope::Project { workspace_root } => {
                workspace_root.join(".opi").join("packages.toml")
            }
        }
    }

    /// Path to the `package-lock.toml` lock file.
    pub fn lock_path(&self) -> PathBuf {
        match self {
            PackageStoreScope::Global { user_config_dir } => {
                user_config_dir.join("package-lock.toml")
            }
            PackageStoreScope::Project { workspace_root } => {
                workspace_root.join(".opi").join("package-lock.toml")
            }
        }
    }

    /// Path to the package cache directory.
    pub fn cache_dir(&self) -> PathBuf {
        match self {
            PackageStoreScope::Global { user_config_dir } => user_config_dir.join("package-cache"),
            PackageStoreScope::Project { workspace_root } => {
                workspace_root.join(".opi").join("package-cache")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Declaration types (packages.toml)
// ---------------------------------------------------------------------------

/// A package declaration entry from `packages.toml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PackageDeclaration {
    /// Source string for the package (local path or git URL).
    pub source: String,
    /// Optional resource filters.
    #[serde(default)]
    pub filters: PackageFilters,
}

/// Resource filters for a package declaration.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PackageFilters {
    /// Include only these extensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    /// Include only these skills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    /// Include only these prompt fragments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragments: Option<Vec<String>>,
    /// Include only these themes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub themes: Option<Vec<String>>,
}

/// Top-level TOML structure for `packages.toml`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct PackagesFile {
    #[serde(default, rename = "package", skip_serializing_if = "Vec::is_empty")]
    packages: Vec<PackageDeclaration>,
}

// ---------------------------------------------------------------------------
// Lock entry types (package-lock.toml)
// ---------------------------------------------------------------------------

/// A lock entry recording the resolved state of an installed package.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PackageLockEntry {
    /// Source kind: `"local"` or `"git"`.
    pub identity_kind: String,
    /// Source value: path or URL.
    pub identity_value: String,
    /// Original source string from the declaration.
    pub source: String,
    /// Absolute path to the resolved package root.
    pub package_root: PathBuf,
    /// Optional path to the local cache directory (for git sources).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_path: Option<PathBuf>,
    /// Git commit SHA (for git sources).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_commit: Option<String>,
    /// SHA-256 hash of the package manifest (`package.toml`).
    pub manifest_sha256: String,
}

/// Top-level TOML structure for `package-lock.toml`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct PackageLockFile {
    #[serde(default, rename = "lock", skip_serializing_if = "Vec::is_empty")]
    locks: Vec<PackageLockEntry>,
}

// ---------------------------------------------------------------------------
// Package store
// ---------------------------------------------------------------------------

/// The package store, providing read/write access to declarations and lock
/// files within a given scope.
#[derive(Debug, Clone)]
pub struct PackageStore {
    scope: PackageStoreScope,
}

impl PackageStore {
    /// Create a new store for the given scope.
    pub fn new(scope: PackageStoreScope) -> Self {
        Self { scope }
    }

    /// Convenience constructor for a project-scoped store.
    pub fn project(workspace_root: PathBuf) -> Self {
        Self::new(PackageStoreScope::Project { workspace_root })
    }

    /// Convenience constructor for a global-scoped store.
    pub fn global(user_config_dir: PathBuf) -> Self {
        Self::new(PackageStoreScope::Global { user_config_dir })
    }

    /// Read package declarations from `packages.toml`.
    ///
    /// Returns an empty vector if the file does not exist.
    pub fn read_declarations(&self) -> Result<Vec<PackageDeclaration>, PackageStoreError> {
        let path = self.scope.config_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let file: PackagesFile = toml::from_str(&content)?;
        Ok(file.packages)
    }

    /// Write package declarations to `packages.toml`, creating parent
    /// directories as needed.
    pub fn write_declarations(
        &self,
        declarations: &[PackageDeclaration],
    ) -> Result<(), PackageStoreError> {
        let path = self.scope.config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = PackagesFile {
            packages: declarations.to_vec(),
        };
        let content = toml::to_string_pretty(&file)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Read lock entries from `package-lock.toml`.
    ///
    /// Returns an empty vector if the file does not exist.
    pub fn read_lock(&self) -> Result<Vec<PackageLockEntry>, PackageStoreError> {
        let path = self.scope.lock_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let file: PackageLockFile = toml::from_str(&content)?;
        Ok(file.locks)
    }

    /// Write lock entries to `package-lock.toml`, creating parent directories
    /// as needed.
    pub fn write_lock(&self, entries: &[PackageLockEntry]) -> Result<(), PackageStoreError> {
        let path = self.scope.lock_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = PackageLockFile {
            locks: entries.to_vec(),
        };
        let content = toml::to_string_pretty(&file)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Return the cache directory path for this store's scope.
    pub fn cache_dir(&self) -> PathBuf {
        self.scope.cache_dir()
    }

    /// Clone a git repository and check out a specific commit.
    ///
    /// Uses `git clone` and `git checkout` via `std::process::Command`.
    /// Does not add `git2` or `gix` dependencies.
    pub fn git_clone(
        &self,
        url: &str,
        refspec: &str,
        target: &Path,
    ) -> Result<(), PackageStoreError> {
        // Validate that the target is within the store's cache directory.
        let cache_dir = self.scope.cache_dir();
        // Canonicalize cache_dir (must already exist or be creatable).
        // Since target may not exist yet, canonicalize the parent and verify
        // the final component does not escape via `..`.
        let target_normalized = normalize_path(target);
        let cache_normalized = normalize_path(&cache_dir);
        if !target_normalized.starts_with(&cache_normalized) {
            return Err(PackageStoreError::Git(format!(
                "clone target {target:?} is outside the store cache directory {cache_dir:?}"
            )));
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Clone the repository
        let output = std::process::Command::new("git")
            .args(["clone", url])
            .arg(target)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| PackageStoreError::Git(format!("failed to execute git clone: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PackageStoreError::Git(format!(
                "git clone failed: {stderr}"
            )));
        }

        // Checkout the specific ref
        let checkout_output = std::process::Command::new("git")
            .args(["checkout", refspec])
            .current_dir(target)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| PackageStoreError::Git(format!("failed to execute git checkout: {e}")))?;

        if !checkout_output.status.success() {
            let stderr = String::from_utf8_lossy(&checkout_output.stderr);
            return Err(PackageStoreError::Git(format!(
                "git checkout {refspec} failed: {stderr}"
            )));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Path detection helpers
// ---------------------------------------------------------------------------

/// Determine if a source string looks like a local filesystem path.
///
/// Recognizes:
/// - Relative paths starting with `.`, `./`, `.\`, `..`
/// - Unix absolute paths starting with `/`
/// - Windows UNC paths starting with `\\`
/// - Windows drive-letter paths like `C:\` or `D:/`
///
/// Rejects anything with a `:` that isn't a single-character drive letter
/// followed by a path separator (so `npm:foo`, `http://...` etc. are not
/// treated as local paths).
fn is_local_path(s: &str) -> bool {
    if s.starts_with('.') || s.starts_with('/') {
        return true;
    }
    // Windows UNC
    if s.starts_with('\\') {
        return true;
    }
    // Windows drive letter: exactly one ASCII letter, then ':', then '/' or '\'
    let bytes = s.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        return true;
    }
    false
}

/// Normalize a path by resolving `.` and `..` components without touching the
/// filesystem. This avoids `canonicalize` which requires the path to exist.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if components
                    .last()
                    .is_some_and(|c| !matches!(c, std::path::Component::ParentDir))
                {
                    components.pop();
                } else {
                    components.push(comp);
                }
            }
            _ => components.push(comp),
        }
    }
    components.iter().collect()
}
