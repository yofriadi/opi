//! Prompt fragment/template progressive discovery, registry, and expansion.
//!
//! Provides the discovery and registry system for prompt fragments (templates)
//! that are progressively loaded from project, user, explicit, and package
//! resources. Fragment metadata (name, description, arguments) is available
//! without loading the full fragment body, which is loaded on demand when
//! needed for expansion.
//!
//! # Fragment Format
//!
//! Each fragment is a directory containing a `FRAGMENT.md` file with YAML
//! frontmatter:
//!
//! ```markdown
//! ---
//! name: translate
//! description: Translate text between languages.
//! arguments: text, from=en, to=fr
//! ---
//!
//! Translate {{text}} from {{from}} to {{to}}.
//! ```
//!
//! # Name Validation
//!
//! Fragment names follow the same rules as skill names: lowercase ASCII
//! letters (`a-z`), digits (`0-9`), and hyphens (`-`), with a maximum
//! length of 64 characters.
//!
//! # Argument Declaration
//!
//! Arguments are declared as a comma-separated list in the frontmatter.
//! Each argument is either:
//!
//! - **Required**: just the name (e.g. `text`)
//! - **Optional with default**: `name=default_value` (e.g. `format=markdown`)
//!
//! In the fragment body, arguments are referenced as `{{name}}` placeholders.
//!
//! # Progressive Disclosure
//!
//! Discovery returns [`FragmentResource`] entries containing only the parsed
//! frontmatter metadata. The full fragment body can be loaded on demand via
//! [`FragmentResource::load_body`]. Argument expansion is performed by
//! [`expand_fragment_body`] or [`FragmentRegistry::expand`].
//!
//! # Discovery Precedence
//!
//! Fragments are discovered from multiple layers using the same precedence
//! model as extensions and skills (see [`crate::resource`]). Higher
//! precedence values override lower ones when fragment names collide.
//!
//! # Unstable
//!
//! This module is part of the **unstable 0.x extension API**. Breaking changes
//! may occur between minor versions without a major version bump.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from fragment discovery, manifest parsing, and expansion.
#[derive(Debug, thiserror::Error)]
pub enum FragmentDiscoveryError {
    /// The FRAGMENT.md file has no valid YAML frontmatter delimiters (`---`).
    #[error("invalid frontmatter in {path}: {reason}")]
    InvalidFrontmatter { path: PathBuf, reason: String },
    /// A required field is missing or empty in the frontmatter.
    #[error("missing required field '{field}' in fragment at {path}")]
    MissingField { field: String, path: PathBuf },
    /// Two fragments in the same precedence layer use the same name.
    #[error("duplicate fragment name '{name}' in discovery layer at {path}")]
    DuplicateName { name: String, path: PathBuf },
    /// The fragment name contains invalid characters or exceeds the length limit.
    #[error("invalid fragment name in {path}: {reason}")]
    InvalidName { path: PathBuf, reason: String },
    /// The description is empty or exceeds the length limit.
    #[error("invalid description in fragment at {path}: {reason}")]
    InvalidDescription { path: PathBuf, reason: String },
    /// An argument name is invalid.
    #[error("invalid argument name in fragment at {path}: {reason}")]
    InvalidArgument { path: PathBuf, reason: String },
    /// A required argument was not provided during expansion.
    #[error("missing required argument '{argument}' for fragment '{fragment}'")]
    MissingArgument { fragment: String, argument: String },
    /// An I/O error occurred during discovery or body loading.
    #[error("I/O error discovering fragments: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum allowed length for a fragment name.
const MAX_NAME_LEN: usize = 64;

/// Maximum allowed length for a fragment description.
const MAX_DESCRIPTION_LEN: usize = 1024;

// ---------------------------------------------------------------------------
// Argument types
// ---------------------------------------------------------------------------

/// A declared fragment argument with name, requirement flag, and optional
/// default value.
#[derive(Debug, Clone, PartialEq)]
pub struct FragmentArgument {
    /// Argument name. Must be a valid identifier (lowercase a-z, 0-9, hyphens).
    pub name: String,
    /// Whether this argument must be provided during expansion.
    pub required: bool,
    /// Default value used when the argument is not provided. When `None` and
    /// `required` is `false`, the argument has no default.
    pub default: Option<String>,
}

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Parsed fragment manifest from `FRAGMENT.md` frontmatter.
#[derive(Debug, Clone, PartialEq)]
pub struct FragmentManifest {
    /// Fragment name. Required, non-empty. Lowercase ASCII letters, digits,
    /// and hyphens. Maximum 64 characters.
    pub name: String,
    /// Human-readable description. Required, non-empty. Maximum 1024
    /// characters.
    pub description: String,
    /// Declared arguments for template expansion. May be empty.
    pub arguments: Vec<FragmentArgument>,
}

impl FragmentManifest {
    /// Parse a manifest from the full content of a `FRAGMENT.md` file.
    ///
    /// The content must contain YAML frontmatter between `---` delimiters.
    /// Only the frontmatter is parsed; the body is ignored.
    pub fn from_fragment_md(content: &str, path: &Path) -> Result<Self, FragmentDiscoveryError> {
        let fm = extract_frontmatter(content, path)?;

        let name = parse_field(fm, "name")
            .map(strip_yaml_quotes)
            .filter(|n| !n.is_empty())
            .ok_or_else(|| FragmentDiscoveryError::MissingField {
                field: "name".into(),
                path: path.to_path_buf(),
            })?;

        validate_name(name, path)?;

        let description = parse_field(fm, "description")
            .map(strip_yaml_quotes)
            .filter(|d| !d.is_empty())
            .ok_or_else(|| FragmentDiscoveryError::MissingField {
                field: "description".into(),
                path: path.to_path_buf(),
            })?;

        validate_description(description, path)?;

        let arguments = match parse_field(fm, "arguments") {
            Some(args_str) => parse_arguments(args_str, path)?,
            None => Vec::new(),
        };

        Ok(Self {
            name: name.to_string(),
            description: description.to_string(),
            arguments,
        })
    }
}

// ---------------------------------------------------------------------------
// Frontmatter parsing helpers
// ---------------------------------------------------------------------------

/// Extract the text between the first two `---` delimiters.
fn extract_frontmatter<'a>(
    content: &'a str,
    path: &Path,
) -> Result<&'a str, FragmentDiscoveryError> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(FragmentDiscoveryError::InvalidFrontmatter {
            path: path.to_path_buf(),
            reason: "FRAGMENT.md must start with '---' frontmatter delimiter".into(),
        });
    }

    let after_open = trimmed.get(3..).unwrap_or("");
    let after_open = after_open.trim_start_matches(['\r', '\n']);

    let close_pos = after_open
        .find("\n---")
        .or_else(|| after_open.find("\r\n---"));

    let frontmatter = match close_pos {
        Some(pos) => &after_open[..pos],
        None => {
            return Err(FragmentDiscoveryError::InvalidFrontmatter {
                path: path.to_path_buf(),
                reason: "FRAGMENT.md frontmatter is missing closing '---' delimiter".into(),
            });
        }
    };

    Ok(frontmatter)
}

/// Parse a `key: value` field from frontmatter text.
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
fn strip_yaml_quotes(value: &str) -> &str {
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        &value[1..value.len().saturating_sub(1)]
    } else {
        value
    }
}

/// Parse a comma-separated arguments string into a list of [`FragmentArgument`].
///
/// Format: `arg1, arg2=default_value, arg3`
fn parse_arguments(
    args_str: &str,
    path: &Path,
) -> Result<Vec<FragmentArgument>, FragmentDiscoveryError> {
    let mut args = Vec::new();
    for part in args_str.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(eq_pos) = part.find('=') {
            let name = part[..eq_pos].trim();
            let default = part[eq_pos + 1..].trim();

            validate_argument_name(name, path)?;

            args.push(FragmentArgument {
                name: name.to_string(),
                required: false,
                default: Some(default.to_string()),
            });
        } else {
            validate_argument_name(part, path)?;

            args.push(FragmentArgument {
                name: part.to_string(),
                required: true,
                default: None,
            });
        }
    }
    Ok(args)
}

/// Validate that a fragment name contains only allowed characters and is within
/// length bounds.
fn validate_name(name: &str, path: &Path) -> Result<(), FragmentDiscoveryError> {
    if name.len() > MAX_NAME_LEN {
        return Err(FragmentDiscoveryError::InvalidName {
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
            return Err(FragmentDiscoveryError::InvalidName {
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
fn validate_description(desc: &str, path: &Path) -> Result<(), FragmentDiscoveryError> {
    if desc.len() > MAX_DESCRIPTION_LEN {
        return Err(FragmentDiscoveryError::InvalidDescription {
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

/// Validate that an argument name is non-empty and contains only allowed
/// characters.
fn validate_argument_name(name: &str, path: &Path) -> Result<(), FragmentDiscoveryError> {
    if name.is_empty() {
        return Err(FragmentDiscoveryError::InvalidArgument {
            path: path.to_path_buf(),
            reason: "argument name is empty".into(),
        });
    }

    for ch in name.chars() {
        let valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_';
        if !valid {
            return Err(FragmentDiscoveryError::InvalidArgument {
                path: path.to_path_buf(),
                reason: format!(
                    "argument name '{name}' contains invalid character '{ch}': \
                     only lowercase a-z, 0-9, hyphens, and underscores are allowed"
                ),
            });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Discovery types
// ---------------------------------------------------------------------------

/// A discovered fragment resource with its manifest, filesystem path, and layer
/// precedence.
///
/// The manifest metadata is available immediately. The full fragment body can
/// be loaded on demand via [`load_body`](FragmentResource::load_body).
#[derive(Debug, Clone)]
pub struct FragmentResource {
    /// The parsed fragment manifest (metadata only).
    pub manifest: FragmentManifest,
    /// Absolute path to the fragment directory (containing `FRAGMENT.md`).
    pub path: PathBuf,
    /// Path to the `FRAGMENT.md` file itself, for on-demand body loading.
    pub fragment_md_path: PathBuf,
    /// Precedence value of the discovery layer that produced this resource.
    pub layer_precedence: u32,
}

impl FragmentResource {
    /// Load the full fragment body (everything after the frontmatter) on demand.
    ///
    /// This reads the `FRAGMENT.md` file from disk, strips the frontmatter,
    /// and returns the remaining content.
    pub fn load_body(&self) -> Result<String, FragmentDiscoveryError> {
        let content = std::fs::read_to_string(&self.fragment_md_path)?;
        Ok(extract_body(&content))
    }
}

/// Extract the body (everything after the closing `---`) from a FRAGMENT.md.
fn extract_body(content: &str) -> String {
    let trimmed = content.trim_start();
    let after_open = trimmed.get(3..).unwrap_or("");
    let after_open = after_open.trim_start_matches(['\r', '\n']);

    let close_pos = after_open
        .find("\n---")
        .or_else(|| after_open.find("\r\n---"));

    match close_pos {
        Some(pos) => {
            let after_close = &after_open[pos..];
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

/// Discover fragments across multiple layers with precedence-based
/// deduplication.
///
/// Each layer's scan directory is enumerated for subdirectories containing
/// `FRAGMENT.md` files. When multiple layers produce fragments with the same
/// name, the one with the highest `precedence` value is kept. Duplicate names
/// within the same precedence layer are reported as an error.
///
/// Returns the deduplicated list of discovered fragment resources, sorted by
/// name. Missing scan directories are silently skipped.
pub fn discover_fragments(
    layers: &[crate::resource::DiscoveryLayer],
) -> Result<Vec<FragmentResource>, FragmentDiscoveryError> {
    let mut seen: HashMap<String, FragmentResource> = HashMap::new();

    for layer in layers {
        let scan_dir = layer.scan_dir();
        if !scan_dir.is_dir() {
            continue;
        }

        if scan_dir.join("FRAGMENT.md").exists() {
            discover_fragment_dir(&scan_dir, layer, &mut seen)?;
            continue;
        }

        let entries = match std::fs::read_dir(&scan_dir) {
            Ok(entries) => entries,
            Err(e) => return Err(FragmentDiscoveryError::Io(e)),
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let fragment_md = path.join("FRAGMENT.md");
            if !fragment_md.exists() {
                continue;
            }

            discover_fragment_dir(&path, layer, &mut seen)?;
        }
    }

    let mut resources: Vec<FragmentResource> = seen.into_values().collect();
    resources.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(resources)
}

fn discover_fragment_dir(
    path: &Path,
    layer: &crate::resource::DiscoveryLayer,
    seen: &mut HashMap<String, FragmentResource>,
) -> Result<(), FragmentDiscoveryError> {
    let fragment_md = path.join("FRAGMENT.md");
    let content = std::fs::read_to_string(&fragment_md)?;
    let manifest = FragmentManifest::from_fragment_md(&content, &fragment_md)?;

    let canonical = path.canonicalize()?;

    match seen.get(&manifest.name) {
        Some(existing) if layer.precedence == existing.layer_precedence => {
            return Err(FragmentDiscoveryError::DuplicateName {
                name: manifest.name,
                path: canonical,
            });
        }
        Some(existing) if layer.precedence < existing.layer_precedence => return Ok(()),
        Some(_) | None => {
            seen.insert(
                manifest.name.clone(),
                FragmentResource {
                    manifest,
                    path: canonical,
                    fragment_md_path: fragment_md,
                    layer_precedence: layer.precedence,
                },
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Expansion
// ---------------------------------------------------------------------------

/// Expand `{{arg}}` placeholders in a fragment body using declared arguments
/// and provided values.
///
/// - Required arguments that are missing produce a
///   [`FragmentDiscoveryError::MissingArgument`] error.
/// - Optional arguments use their default value when not provided.
/// - Placeholders not matching any declared argument are left as-is.
/// - Extra values not matching any argument are silently ignored.
pub fn expand_fragment_body(
    body: &str,
    arguments: &[FragmentArgument],
    values: &HashMap<String, String>,
) -> Result<String, FragmentDiscoveryError> {
    // Build a resolved map: argument name -> final value.
    let mut resolved: HashMap<&str, &str> = HashMap::new();
    for arg in arguments {
        match values.get(&arg.name) {
            Some(val) => {
                resolved.insert(&arg.name, val);
            }
            None => {
                if arg.required {
                    return Err(FragmentDiscoveryError::MissingArgument {
                        fragment: String::new(),
                        argument: arg.name.clone(),
                    });
                }
                if let Some(ref default) = arg.default {
                    resolved.insert(&arg.name, default);
                }
            }
        }
    }

    // Replace all {{name}} placeholders.
    let mut result = body.to_string();
    for arg in arguments {
        if let Some(val) = resolved.get(arg.name.as_str()) {
            let placeholder = format!("{{{{{}}}}}", arg.name);
            result = result.replace(&placeholder, val);
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// A registry of discovered fragments supporting progressive disclosure,
/// argument expansion, and prompt/RPC metadata formatting.
pub struct FragmentRegistry {
    resources: Vec<FragmentResource>,
}

impl FragmentRegistry {
    /// Build a registry from discovered fragment resources.
    pub fn from_resources(resources: Vec<FragmentResource>) -> Self {
        Self { resources }
    }

    /// Return sorted list of all fragment names.
    pub fn names(&self) -> Vec<&str> {
        self.resources
            .iter()
            .map(|r| r.manifest.name.as_str())
            .collect()
    }

    /// Look up a fragment by name, returning its resource (metadata only).
    pub fn get(&self, name: &str) -> Option<&FragmentResource> {
        self.resources.iter().find(|r| r.manifest.name == name)
    }

    /// Load the full body of a fragment by name.
    ///
    /// Returns `None` if the fragment is not found or `Some(Err(...))` if the
    /// file cannot be read.
    pub fn load_body(&self, name: &str) -> Option<Result<String, FragmentDiscoveryError>> {
        self.get(name).map(|r| r.load_body())
    }

    /// Expand a fragment by name with the provided argument values.
    ///
    /// Loads the body on demand, validates arguments, and performs placeholder
    /// substitution. Returns `None` if the fragment is not found.
    pub fn expand(
        &self,
        name: &str,
        values: &HashMap<String, String>,
    ) -> Option<Result<String, FragmentDiscoveryError>> {
        let resource = self.get(name)?;
        let body = match resource.load_body() {
            Ok(b) => b,
            Err(e) => return Some(Err(e)),
        };

        Some(expand_fragment_body(
            &body,
            &resource.manifest.arguments,
            values,
        ))
    }

    /// Format all fragment metadata as a string suitable for inclusion in a
    /// system prompt or command listing.
    ///
    /// Each fragment is represented as a brief entry with name, description,
    /// and argument summary.
    pub fn format_for_prompt(&self) -> String {
        if self.resources.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();
        for r in &self.resources {
            let args_summary = if r.manifest.arguments.is_empty() {
                String::new()
            } else {
                let arg_names: Vec<&str> = r
                    .manifest
                    .arguments
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect();
                format!(" [{}]", arg_names.join(", "))
            };
            parts.push(format!(
                "- {}: {}{}",
                r.manifest.name, r.manifest.description, args_summary
            ));
        }
        parts.join("\n")
    }

    /// Format all fragment metadata as a string suitable for RPC command
    /// metadata listing.
    ///
    /// Includes argument names, required/optional status, and default values.
    pub fn format_for_rpc_metadata(&self) -> String {
        if self.resources.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();
        for r in &self.resources {
            let mut frag_entry = format!("{}: {}", r.manifest.name, r.manifest.description);
            if !r.manifest.arguments.is_empty() {
                frag_entry.push_str(" | arguments:");
                for arg in &r.manifest.arguments {
                    if arg.required {
                        frag_entry.push_str(&format!(" {} (required)", arg.name));
                    } else {
                        let default = arg.default.as_deref().unwrap_or("");
                        frag_entry.push_str(&format!(" {} (default: {})", arg.name, default));
                    }
                }
            }
            parts.push(frag_entry);
        }
        parts.join("\n")
    }
}
