//! Theme progressive discovery, registry, and loading.
//!
//! Provides the discovery and registry system for themes that are progressively
//! loaded from project, user, explicit, and package resources. Theme metadata
//! (name, description) is available without parsing all color tokens; the full
//! [`Theme`] is constructed on demand when needed.
//!
//! # Theme File Format
//!
//! Each theme is a directory containing a `theme.toml` file:
//!
//! ```toml
//! name = "my-theme"
//! description = "A warm theme for late-night coding."
//!
//! [colors]
//! role_user = "Green"
//! role_assistant = "#66d9ef"
//! status_bg = "#1a1a2e"
//! ```
//!
//! Colors may be specified as named colors (`"Red"`, `"DarkGray"`, etc.) or
//! hex RGB (`"#rrggbb"`). Unspecified tokens inherit from the default theme.
//!
//! # Discovery Precedence
//!
//! Themes use the same precedence-based discovery as extensions and skills
//! (see [`crate::resource`]). Higher precedence values override lower ones
//! when theme names collide.
//!
//! # Unstable
//!
//! This module is part of the **unstable 0.x extension API**. Breaking changes
//! may occur between minor versions without a major version bump.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use opi_tui::theme::{Theme, is_valid_token, parse_color};
use ratatui::style::Color;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from theme discovery, manifest parsing, and loading.
#[derive(Debug, thiserror::Error)]
pub enum ThemeDiscoveryError {
    /// The theme.toml file could not be parsed as valid TOML.
    #[error("invalid theme manifest at {path}: {reason}")]
    InvalidManifest { path: PathBuf, reason: String },
    /// A required field is missing or empty in the manifest.
    #[error("missing required field '{field}' in theme at {path}")]
    MissingField { field: String, path: PathBuf },
    /// Two themes in the same precedence layer use the same name.
    #[error("duplicate theme name '{name}' in discovery layer at {path}")]
    DuplicateName { name: String, path: PathBuf },
    /// The theme name is invalid (bad characters or too long).
    #[error("invalid theme name in {path}: {reason}")]
    InvalidName { path: PathBuf, reason: String },
    /// The description is invalid (too long).
    #[error("invalid description in theme at {path}: {reason}")]
    InvalidDescription { path: PathBuf, reason: String },
    /// A color token value is not a valid color.
    #[error("invalid color for token '{token}' in theme at {path}: {reason}")]
    InvalidColor {
        token: String,
        path: PathBuf,
        reason: String,
    },
    /// A color token name is not recognized.
    #[error("unknown color token '{token}' in theme at {path}")]
    UnknownToken { token: String, path: PathBuf },
    /// An I/O error occurred during discovery or loading.
    #[error("I/O error discovering themes: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum allowed length for a theme name.
const MAX_NAME_LEN: usize = 64;

/// Maximum allowed length for a theme description.
const MAX_DESCRIPTION_LEN: usize = 1024;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Parsed theme manifest from `theme.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeManifest {
    /// Theme name. Required, non-empty. Lowercase ASCII letters, digits,
    /// and hyphens. Maximum 64 characters.
    pub name: String,
    /// Human-readable description. Required, non-empty. Maximum 1024
    /// characters.
    pub description: String,
}

/// Top-level TOML structure for theme files.
#[derive(Debug, Clone, Deserialize)]
struct TomlThemeFile {
    name: Option<String>,
    description: Option<String>,
    colors: Option<HashMap<String, String>>,
}

impl ThemeManifest {
    /// Parse a manifest from TOML content, validating required fields.
    ///
    /// Only validates metadata (name, description); color tokens are not
    /// parsed at this stage (progressive disclosure).
    pub fn from_toml(content: &str, path: &Path) -> Result<Self, ThemeDiscoveryError> {
        let file: TomlThemeFile =
            toml::from_str(content).map_err(|e| ThemeDiscoveryError::InvalidManifest {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?;

        let name = file.name.filter(|n| !n.trim().is_empty()).ok_or_else(|| {
            ThemeDiscoveryError::MissingField {
                field: "name".into(),
                path: path.to_path_buf(),
            }
        })?;

        validate_theme_name(&name, path)?;

        let description = file
            .description
            .filter(|d| !d.trim().is_empty())
            .ok_or_else(|| ThemeDiscoveryError::MissingField {
                field: "description".into(),
                path: path.to_path_buf(),
            })?;

        validate_description(&description, path)?;

        Ok(Self { name, description })
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validate that a theme name contains only allowed characters and is within
/// length bounds.
fn validate_theme_name(name: &str, path: &Path) -> Result<(), ThemeDiscoveryError> {
    if name.len() > MAX_NAME_LEN {
        return Err(ThemeDiscoveryError::InvalidName {
            path: path.to_path_buf(),
            reason: format!(
                "name exceeds maximum length of {MAX_NAME_LEN} characters ({} found)",
                name.len()
            ),
        });
    }

    for ch in name.chars() {
        let valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !valid {
            return Err(ThemeDiscoveryError::InvalidName {
                path: path.to_path_buf(),
                reason: format!(
                    "name contains invalid character '{ch}': \
                     only lowercase a-z, 0-9, and hyphens are allowed"
                ),
            });
        }
    }

    Ok(())
}

/// Validate that a description is within length bounds.
fn validate_description(desc: &str, path: &Path) -> Result<(), ThemeDiscoveryError> {
    if desc.len() > MAX_DESCRIPTION_LEN {
        return Err(ThemeDiscoveryError::InvalidDescription {
            path: path.to_path_buf(),
            reason: format!(
                "description exceeds maximum length of {MAX_DESCRIPTION_LEN} characters \
                 ({} found)",
                desc.len()
            ),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Discovery types
// ---------------------------------------------------------------------------

/// A discovered theme resource with its manifest, filesystem path, and layer
/// precedence.
///
/// The manifest metadata is available immediately. The full [`Theme`] can be
/// constructed on demand via [`load_theme`](ThemeResource::load_theme).
#[derive(Debug, Clone)]
pub struct ThemeResource {
    /// The parsed theme manifest (metadata only).
    pub manifest: ThemeManifest,
    /// Absolute path to the theme directory.
    pub path: PathBuf,
    /// Path to the `theme.toml` file for on-demand color loading.
    pub theme_toml_path: PathBuf,
    /// Precedence value of the discovery layer that produced this resource.
    pub layer_precedence: u32,
}

impl ThemeResource {
    /// Load the full theme from the TOML file on demand.
    ///
    /// Reads the theme.toml, parses all color tokens, validates against the
    /// theme token schema, and constructs a [`Theme`]. Missing tokens inherit
    /// from the default theme.
    pub fn load_theme(&self) -> Result<Theme, ThemeDiscoveryError> {
        let content = std::fs::read_to_string(&self.theme_toml_path)?;
        let file: TomlThemeFile =
            toml::from_str(&content).map_err(|e| ThemeDiscoveryError::InvalidManifest {
                path: self.theme_toml_path.clone(),
                reason: e.to_string(),
            })?;

        let mut colors: HashMap<String, Color> = HashMap::new();
        if let Some(raw_colors) = file.colors {
            for (token, value) in &raw_colors {
                if !is_valid_token(token) {
                    return Err(ThemeDiscoveryError::UnknownToken {
                        token: token.clone(),
                        path: self.theme_toml_path.clone(),
                    });
                }

                let color = parse_color(value).map_err(|e| ThemeDiscoveryError::InvalidColor {
                    token: token.clone(),
                    path: self.theme_toml_path.clone(),
                    reason: e.to_string(),
                })?;

                colors.insert(token.clone(), color);
            }
        }

        Theme::from_color_map(self.manifest.name.clone(), &colors).map_err(|e| {
            ThemeDiscoveryError::InvalidColor {
                token: String::new(),
                path: self.theme_toml_path.clone(),
                reason: e.to_string(),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover themes across multiple layers with precedence-based
/// deduplication.
///
/// Each layer's scan directory is enumerated for subdirectories containing
/// `theme.toml` files. When multiple layers produce themes with the same
/// name, the one with the highest `precedence` value is kept. Duplicate names
/// within the same precedence layer are reported as an error.
///
/// Returns the deduplicated list of discovered theme resources, sorted by
/// name. Missing scan directories are silently skipped.
pub fn discover_themes(
    layers: &[crate::resource::DiscoveryLayer],
) -> Result<Vec<ThemeResource>, ThemeDiscoveryError> {
    let mut seen: HashMap<String, ThemeResource> = HashMap::new();

    for layer in layers {
        let scan_dir = layer.scan_dir();
        if !scan_dir.is_dir() {
            continue;
        }

        if scan_dir.join("theme.toml").exists() {
            discover_theme_dir(&scan_dir, layer, &mut seen)?;
            continue;
        }

        let entries = match std::fs::read_dir(&scan_dir) {
            Ok(entries) => entries,
            Err(e) => return Err(ThemeDiscoveryError::Io(e)),
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let theme_toml = path.join("theme.toml");
            if !theme_toml.exists() {
                continue;
            }

            discover_theme_dir(&path, layer, &mut seen)?;
        }
    }

    let mut resources: Vec<ThemeResource> = seen.into_values().collect();
    resources.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(resources)
}

fn discover_theme_dir(
    path: &Path,
    layer: &crate::resource::DiscoveryLayer,
    seen: &mut HashMap<String, ThemeResource>,
) -> Result<(), ThemeDiscoveryError> {
    let theme_toml = path.join("theme.toml");
    let content = std::fs::read_to_string(&theme_toml)?;
    let manifest = ThemeManifest::from_toml(&content, &theme_toml)?;

    let canonical = path.canonicalize()?;

    match seen.get(&manifest.name) {
        Some(existing) if layer.precedence == existing.layer_precedence => {
            return Err(ThemeDiscoveryError::DuplicateName {
                name: manifest.name,
                path: canonical,
            });
        }
        Some(existing) if layer.precedence < existing.layer_precedence => return Ok(()),
        Some(_) | None => {
            seen.insert(
                manifest.name.clone(),
                ThemeResource {
                    manifest,
                    path: canonical,
                    theme_toml_path: theme_toml,
                    layer_precedence: layer.precedence,
                },
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// A registry of discovered themes supporting progressive disclosure and
/// active theme resolution.
pub struct ThemeRegistry {
    resources: Vec<ThemeResource>,
}

impl ThemeRegistry {
    /// Build a registry from discovered theme resources.
    pub fn from_resources(resources: Vec<ThemeResource>) -> Self {
        Self { resources }
    }

    /// Return sorted list of all theme names.
    pub fn names(&self) -> Vec<&str> {
        self.resources
            .iter()
            .map(|r| r.manifest.name.as_str())
            .collect()
    }

    /// Look up a theme by name, returning its resource (metadata only).
    pub fn get(&self, name: &str) -> Option<&ThemeResource> {
        self.resources.iter().find(|r| r.manifest.name == name)
    }

    /// Load the full theme by name.
    ///
    /// Returns `None` if the theme is not found, or `Some(Err(...))` if the
    /// theme file cannot be loaded or parsed.
    pub fn load_theme(&self, name: &str) -> Option<Result<Theme, ThemeDiscoveryError>> {
        self.get(name).map(|r| r.load_theme())
    }

    /// Resolve a theme by name, checking discovered themes first, then
    /// built-in themes ("default", "monokai"), then falling back to default.
    pub fn resolve_theme(&self, name: &str) -> Result<Theme, ThemeDiscoveryError> {
        // Check discovered themes first
        if let Some(result) = self.load_theme(name) {
            return result;
        }

        // Fall back to built-in themes
        Ok(opi_tui::theme::resolve_theme(name))
    }

    /// Format all theme metadata as a string suitable for inclusion in a
    /// system prompt or command listing.
    pub fn format_for_prompt(&self) -> String {
        if self.resources.is_empty() {
            return String::new();
        }

        let parts: Vec<String> = self
            .resources
            .iter()
            .map(|r| format!("- {}: {}", r.manifest.name, r.manifest.description))
            .collect();
        parts.join("\n")
    }
}
