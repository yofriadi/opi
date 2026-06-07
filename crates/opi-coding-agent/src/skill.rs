//! Skill progressive discovery and registry.
//!
//! Provides the discovery and registry system for skills that are progressively
//! loaded from project, user, explicit, and package resources. Skill metadata
//! (name, description) is available without loading the full skill body, which
//! is loaded on demand when needed.
//!
//! # Skill Format
//!
//! Each skill is a directory containing a `SKILL.md` file with YAML frontmatter:
//!
//! ```markdown
//! ---
//! name: my-skill
//! description: What this skill does and when to use it.
//! disable-model-invocation: false   # optional, defaults to false
//! ---
//!
//! Full skill instructions go here.
//! ```
//!
//! # Name Validation
//!
//! Skill names must consist of lowercase ASCII letters (`a-z`), digits (`0-9`),
//! and hyphens (`-`), with a maximum length of 64 characters.
//!
//! # Description Validation
//!
//! Descriptions must be non-empty and at most 1024 characters.
//!
//! # Progressive Disclosure
//!
//! Discovery returns [`SkillResource`] entries containing only the parsed
//! frontmatter metadata. The full skill body (everything after the frontmatter)
//! can be loaded on demand via [`SkillResource::load_body`]. This keeps the
//! initial context small while allowing rich instructions when a skill is
//! actually invoked.
//!
//! # Discovery Precedence
//!
//! Skills are discovered from multiple layers using the same precedence model
//! as extensions (see [`crate::resource`]). Higher precedence values override
//! lower ones when skill names collide across layers.
//!
//! # Unstable
//!
//! This module is part of the **unstable 0.x extension API**. Breaking changes
//! may occur between minor versions without a major version bump.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from skill discovery and manifest parsing.
#[derive(Debug, thiserror::Error)]
pub enum SkillDiscoveryError {
    /// The SKILL.md file has no valid YAML frontmatter delimiters (`---`).
    #[error("invalid frontmatter in {path}: {reason}")]
    InvalidFrontmatter { path: PathBuf, reason: String },
    /// A required field is missing or empty in the frontmatter.
    #[error("missing required field '{field}' in skill at {path}")]
    MissingField { field: String, path: PathBuf },
    /// Two skills in the same precedence layer use the same name.
    #[error("duplicate skill name '{name}' in discovery layer at {path}")]
    DuplicateName { name: String, path: PathBuf },
    /// The skill name contains invalid characters or exceeds the length limit.
    #[error("invalid skill name in {path}: {reason}")]
    InvalidName { path: PathBuf, reason: String },
    /// The description is empty or exceeds the length limit.
    #[error("invalid description in skill at {path}: {reason}")]
    InvalidDescription { path: PathBuf, reason: String },
    /// An I/O error occurred during discovery or body loading.
    #[error("I/O error discovering skills: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum allowed length for a skill name.
const MAX_NAME_LEN: usize = 64;

/// Maximum allowed length for a skill description.
const MAX_DESCRIPTION_LEN: usize = 1024;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Parsed skill manifest from `SKILL.md` frontmatter.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillManifest {
    /// Skill name. Required, non-empty. Lowercase ASCII letters, digits,
    /// and hyphens. Maximum 64 characters.
    pub name: String,
    /// Human-readable description. Required, non-empty. Maximum 1024
    /// characters.
    pub description: String,
    /// When `true`, the model should not automatically invoke this skill.
    /// The skill is still available for human-triggered use. Defaults to
    /// `false`.
    pub disable_model_invocation: bool,
}

impl SkillManifest {
    /// Parse a manifest from the full content of a `SKILL.md` file.
    ///
    /// The content must contain YAML frontmatter between `---` delimiters.
    /// Only the frontmatter is parsed; the body is ignored.
    pub fn from_skill_md(content: &str, path: &Path) -> Result<Self, SkillDiscoveryError> {
        let fm = extract_frontmatter(content, path)?;

        let name = parse_field(fm, "name")
            .map(strip_yaml_quotes)
            .filter(|n| !n.is_empty())
            .ok_or_else(|| SkillDiscoveryError::MissingField {
                field: "name".into(),
                path: path.to_path_buf(),
            })?;

        validate_name(name, path)?;

        let description = parse_field(fm, "description")
            .map(strip_yaml_quotes)
            .filter(|d| !d.is_empty())
            .ok_or_else(|| SkillDiscoveryError::MissingField {
                field: "description".into(),
                path: path.to_path_buf(),
            })?;

        validate_description(description, path)?;

        let disable_model_invocation = parse_field(fm, "disable-model-invocation")
            .map(|v| strip_yaml_quotes(v).eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Ok(Self {
            name: name.to_string(),
            description: description.to_string(),
            disable_model_invocation,
        })
    }
}

// ---------------------------------------------------------------------------
// Frontmatter parsing helpers
// ---------------------------------------------------------------------------

/// Extract the text between the first two `---` delimiters.
fn extract_frontmatter<'a>(content: &'a str, path: &Path) -> Result<&'a str, SkillDiscoveryError> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(SkillDiscoveryError::InvalidFrontmatter {
            path: path.to_path_buf(),
            reason: "SKILL.md must start with '---' frontmatter delimiter".into(),
        });
    }

    // Skip the opening --- and any trailing whitespace/newline.
    let after_open = trimmed.get(3..).unwrap_or("");
    let after_open = after_open.trim_start_matches(['\r', '\n']);

    // Find the closing ---.
    let close_pos = after_open
        .find("\n---")
        .or_else(|| after_open.find("\r\n---"));

    let frontmatter = match close_pos {
        Some(pos) => &after_open[..pos],
        None => {
            return Err(SkillDiscoveryError::InvalidFrontmatter {
                path: path.to_path_buf(),
                reason: "SKILL.md frontmatter is missing closing '---' delimiter".into(),
            });
        }
    };

    Ok(frontmatter)
}

/// Parse a `key: value` field from frontmatter text.
///
/// Handles simple single-line `key: value` pairs. Returns `None` if the key
/// is not found.
fn parse_field<'a>(frontmatter: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}:");
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            return Some(rest.trim());
        }
    }
    None
}

/// Strip surrounding single or double quotes from a YAML scalar value.
///
/// Handles `""`, `''`, and bare strings. Returns the inner content without
/// quotes.
fn strip_yaml_quotes(value: &str) -> &str {
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        &value[1..value.len().saturating_sub(1)]
    } else {
        value
    }
}

/// Validate that a skill name contains only allowed characters and is within
/// length bounds.
fn validate_name(name: &str, path: &Path) -> Result<(), SkillDiscoveryError> {
    if name.len() > MAX_NAME_LEN {
        return Err(SkillDiscoveryError::InvalidName {
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
            return Err(SkillDiscoveryError::InvalidName {
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

/// Validate that a description is non-empty and within length bounds.
fn validate_description(desc: &str, path: &Path) -> Result<(), SkillDiscoveryError> {
    if desc.len() > MAX_DESCRIPTION_LEN {
        return Err(SkillDiscoveryError::InvalidDescription {
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

/// A discovered skill resource with its manifest, filesystem path, and layer
/// precedence.
///
/// The manifest metadata is available immediately. The full skill body can be
/// loaded on demand via [`load_body`](SkillResource::load_body).
#[derive(Debug, Clone)]
pub struct SkillResource {
    /// The parsed skill manifest (metadata only).
    pub manifest: SkillManifest,
    /// Absolute path to the skill directory (containing `SKILL.md`).
    pub path: PathBuf,
    /// Path to the `SKILL.md` file itself, for on-demand body loading.
    pub skill_md_path: PathBuf,
    /// Precedence value of the discovery layer that produced this resource.
    pub layer_precedence: u32,
}

impl SkillResource {
    /// Load the full skill body (everything after the frontmatter) on demand.
    ///
    /// This reads the `SKILL.md` file from disk, strips the frontmatter, and
    /// returns the remaining content. This is the "progressive disclosure"
    /// mechanism: metadata is always available, but the full instructions are
    /// only loaded when the skill is actually invoked.
    pub fn load_body(&self) -> Result<String, SkillDiscoveryError> {
        let content = std::fs::read_to_string(&self.skill_md_path)?;
        Ok(extract_body(&content))
    }
}

/// Extract the body (everything after the closing `---`) from a SKILL.md.
fn extract_body(content: &str) -> String {
    let trimmed = content.trim_start();
    // Skip opening ---.
    let after_open = trimmed.get(3..).unwrap_or("");
    let after_open = after_open.trim_start_matches(['\r', '\n']);

    // Find closing ---.
    let close_pos = after_open
        .find("\n---")
        .or_else(|| after_open.find("\r\n---"));

    match close_pos {
        Some(pos) => {
            // Skip past the closing --- and any trailing whitespace/newline.
            let after_close = &after_open[pos..];
            // Skip the newline + ---.
            let delimiter_end = after_close.find("---").map(|i| i + 3).unwrap_or(pos + 4);
            let body_start = after_close.get(delimiter_end..).unwrap_or("");
            body_start.trim_start_matches(['\r', '\n']).to_string()
        }
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover skills across multiple layers with precedence-based deduplication.
///
/// Each layer's scan directory is enumerated for subdirectories containing
/// `SKILL.md` files. When multiple layers produce skills with the same name,
/// the one with the highest `precedence` value is kept. Duplicate names within
/// the same precedence layer are reported as an error.
///
/// Returns the deduplicated list of discovered skill resources, sorted by name.
/// Missing scan directories are silently skipped.
pub fn discover_skills(
    layers: &[crate::resource::DiscoveryLayer],
) -> Result<Vec<SkillResource>, SkillDiscoveryError> {
    let mut seen: std::collections::HashMap<String, SkillResource> =
        std::collections::HashMap::new();

    for layer in layers {
        let scan_dir = layer.scan_dir();
        if !scan_dir.is_dir() {
            continue;
        }

        if scan_dir.join("SKILL.md").exists() {
            discover_skill_dir(&scan_dir, layer, &mut seen)?;
            continue;
        }

        let entries = match std::fs::read_dir(&scan_dir) {
            Ok(entries) => entries,
            Err(e) => return Err(SkillDiscoveryError::Io(e)),
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only process directories.
            if !path.is_dir() {
                continue;
            }

            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            discover_skill_dir(&path, layer, &mut seen)?;
        }
    }

    let mut resources: Vec<SkillResource> = seen.into_values().collect();
    resources.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(resources)
}

fn discover_skill_dir(
    path: &Path,
    layer: &crate::resource::DiscoveryLayer,
    seen: &mut std::collections::HashMap<String, SkillResource>,
) -> Result<(), SkillDiscoveryError> {
    let skill_md = path.join("SKILL.md");
    let content = std::fs::read_to_string(&skill_md)?;
    let manifest = SkillManifest::from_skill_md(&content, &skill_md)?;

    let canonical = path.canonicalize()?;

    match seen.get(&manifest.name) {
        Some(existing) if layer.precedence == existing.layer_precedence => {
            return Err(SkillDiscoveryError::DuplicateName {
                name: manifest.name,
                path: canonical,
            });
        }
        Some(existing) if layer.precedence < existing.layer_precedence => return Ok(()),
        Some(_) | None => {
            seen.insert(
                manifest.name.clone(),
                SkillResource {
                    manifest,
                    path: canonical,
                    skill_md_path: skill_md,
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

/// A registry of discovered skills supporting progressive disclosure.
///
/// Built from a list of [`SkillResource`] entries, the registry provides:
/// - Metadata lookup by name (no body loading)
/// - Full skill body loading on demand
/// - Listing for prompt integration (auto-invocable vs all)
/// - Prompt-formatted skill summaries
pub struct SkillRegistry {
    resources: Vec<SkillResource>,
}

impl SkillRegistry {
    /// Build a registry from discovered skill resources.
    pub fn from_resources(resources: Vec<SkillResource>) -> Self {
        Self { resources }
    }

    /// Return sorted list of all skill names.
    pub fn names(&self) -> Vec<&str> {
        self.resources
            .iter()
            .map(|r| r.manifest.name.as_str())
            .collect()
    }

    /// Look up a skill by name, returning its resource (metadata only).
    pub fn get(&self, name: &str) -> Option<&SkillResource> {
        self.resources.iter().find(|r| r.manifest.name == name)
    }

    /// Return skills that may be automatically invoked by the model.
    ///
    /// Excludes skills with `disable-model-invocation: true`.
    pub fn auto_invocable(&self) -> Vec<&SkillResource> {
        self.resources
            .iter()
            .filter(|r| !r.manifest.disable_model_invocation)
            .collect()
    }

    /// Load the full body of a skill by name.
    ///
    /// Returns `None` if the skill is not found or `Some(Err(...))` if the
    /// file cannot be read.
    pub fn load_body(&self, name: &str) -> Option<Result<String, SkillDiscoveryError>> {
        self.get(name).map(|r| r.load_body())
    }

    /// Format all skill metadata as a string suitable for inclusion in a
    /// system prompt or command listing.
    ///
    /// Each skill is represented as a brief entry with name and description.
    pub fn format_for_prompt(&self) -> String {
        if self.resources.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();
        for r in &self.resources {
            let flag = if r.manifest.disable_model_invocation {
                " [manual-only]"
            } else {
                ""
            };
            parts.push(format!(
                "- {}: {}{}",
                r.manifest.name, r.manifest.description, flag
            ));
        }
        parts.join("\n")
    }
}
