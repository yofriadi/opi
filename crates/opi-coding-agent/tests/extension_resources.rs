//! Extension resource discovery tests (task 4.5).
//!
//! Tests verify the resource loading strategy discovers extension manifests
//! from project, user, and explicit paths with correct precedence, path
//! normalization, duplicate handling, and structured error reporting.
//! All tests use temp directories — no real user runtime paths are read.

use std::fs;

use opi_coding_agent::resource::{
    DiscoveryLayer, ResourceDiscoveryError, discover_extension_resources,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create an extension.toml manifest in the given directory.
fn write_manifest(dir: &std::path::Path, name: &str, version: &str, description: &str) {
    fs::create_dir_all(dir).unwrap();
    let content = format!(
        r#"[extension]
name = "{name}"
version = "{version}"
description = "{description}"
"#
    );
    fs::write(dir.join("extension.toml"), content).unwrap();
}

/// Create a minimal valid manifest with just a name.
fn write_minimal_manifest(dir: &std::path::Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    let content = format!(
        r#"[extension]
name = "{name}"
"#
    );
    fs::write(dir.join("extension.toml"), content).unwrap();
}

/// Write an invalid TOML file to the given path.
fn write_invalid_manifest(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("extension.toml"), "not valid toml {{{{").unwrap();
}

/// Write a manifest with missing required name field.
fn write_manifest_missing_name(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("extension.toml"),
        r#"[extension]
version = "1.0.0"
"#,
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// 1. Basic discovery from single layers
// ---------------------------------------------------------------------------

#[test]
fn discover_from_project_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let project_ext_dir = tmp.path().join(".opi").join("extensions");
    write_manifest(
        &project_ext_dir.join("my-ext"),
        "my-ext",
        "1.0.0",
        "A test extension",
    );

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some(".opi/extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.name, "my-ext");
    assert_eq!(resources[0].manifest.version.as_deref(), Some("1.0.0"));
    assert_eq!(
        resources[0].manifest.description.as_deref(),
        Some("A test extension")
    );
    assert_eq!(resources[0].layer_precedence, 0);
}

#[test]
fn discover_from_user_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let user_ext_dir = tmp.path().join("extensions");
    write_manifest(
        &user_ext_dir.join("user-ext"),
        "user-ext",
        "2.0.0",
        "User extension",
    );

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some("extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.name, "user-ext");
}

#[test]
fn discover_from_explicit_path() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("my-extensions");
    write_manifest(
        &ext_dir.join("explicit-ext"),
        "explicit-ext",
        "1.0.0",
        "Explicit",
    );

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: ext_dir,
        subdirectory: None,
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.name, "explicit-ext");
}

#[test]
fn discover_multiple_extensions_in_single_layer() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join(".opi").join("extensions");
    write_manifest(&ext_dir.join("ext-a"), "ext-a", "1.0.0", "A");
    write_manifest(&ext_dir.join("ext-b"), "ext-b", "1.0.0", "B");
    write_manifest(&ext_dir.join("ext-c"), "ext-c", "1.0.0", "C");

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some(".opi/extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 3);
    let names: Vec<&str> = resources.iter().map(|r| r.manifest.name.as_str()).collect();
    assert!(names.contains(&"ext-a"));
    assert!(names.contains(&"ext-b"));
    assert!(names.contains(&"ext-c"));
}

// ---------------------------------------------------------------------------
// 2. Precedence model
// ---------------------------------------------------------------------------

#[test]
fn higher_precedence_overrides_lower() {
    let user_tmp = tempfile::tempdir().unwrap();
    let project_tmp = tempfile::tempdir().unwrap();

    // Same extension name in both user and project dirs.
    let user_ext_dir = user_tmp.path().join("extensions");
    write_manifest(
        &user_ext_dir.join("shared"),
        "shared",
        "1.0.0",
        "User version",
    );

    let proj_ext_dir = project_tmp.path().join(".opi").join("extensions");
    write_manifest(
        &proj_ext_dir.join("shared"),
        "shared",
        "2.0.0",
        "Project version",
    );

    let resources = discover_extension_resources(&[
        DiscoveryLayer {
            root: user_tmp.path().to_path_buf(),
            subdirectory: Some("extensions".into()),
            precedence: 0, // lower
        },
        DiscoveryLayer {
            root: project_tmp.path().to_path_buf(),
            subdirectory: Some(".opi/extensions".into()),
            precedence: 1, // higher
        },
    ])
    .unwrap();

    // Should have exactly one entry (deduplicated by name).
    assert_eq!(resources.len(), 1);
    // Higher precedence wins, so we get the project version.
    assert_eq!(resources[0].manifest.version.as_deref(), Some("2.0.0"));
    assert_eq!(
        resources[0].manifest.description.as_deref(),
        Some("Project version")
    );
}

#[test]
fn explicit_path_has_highest_precedence() {
    let user_tmp = tempfile::tempdir().unwrap();
    let project_tmp = tempfile::tempdir().unwrap();
    let explicit_tmp = tempfile::tempdir().unwrap();

    // Same extension name across all three layers.
    let user_ext_dir = user_tmp.path().join("extensions");
    write_manifest(&user_ext_dir.join("shared"), "shared", "1.0.0", "User");

    let proj_ext_dir = project_tmp.path().join(".opi").join("extensions");
    write_manifest(&proj_ext_dir.join("shared"), "shared", "2.0.0", "Project");

    let explicit_dir = explicit_tmp.path().join("ext");
    write_manifest(&explicit_dir.join("shared"), "shared", "3.0.0", "Explicit");

    let resources = discover_extension_resources(&[
        DiscoveryLayer {
            root: user_tmp.path().to_path_buf(),
            subdirectory: Some("extensions".into()),
            precedence: 0,
        },
        DiscoveryLayer {
            root: project_tmp.path().to_path_buf(),
            subdirectory: Some(".opi/extensions".into()),
            precedence: 1,
        },
        DiscoveryLayer {
            root: explicit_dir,
            subdirectory: None,
            precedence: 2,
        },
    ])
    .unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.version.as_deref(), Some("3.0.0"));
    assert_eq!(
        resources[0].manifest.description.as_deref(),
        Some("Explicit")
    );
}

// ---------------------------------------------------------------------------
// 3. Missing resources
// ---------------------------------------------------------------------------

#[test]
fn missing_directory_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let nonexistent = tmp.path().join("does-not-exist");

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: nonexistent,
        subdirectory: Some("extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert!(resources.is_empty());
}

#[test]
fn empty_directory_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("extensions");
    fs::create_dir_all(&ext_dir).unwrap();

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some("extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert!(resources.is_empty());
}

#[test]
fn directory_without_manifest_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join(".opi").join("extensions");
    // Create a directory without extension.toml
    fs::create_dir_all(ext_dir.join("no-manifest")).unwrap();
    // Create a valid one alongside
    write_manifest(&ext_dir.join("valid"), "valid", "1.0.0", "Valid");

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some(".opi/extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.name, "valid");
}

// ---------------------------------------------------------------------------
// 4. Invalid manifests
// ---------------------------------------------------------------------------

#[test]
fn invalid_toml_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("extensions");
    write_invalid_manifest(&ext_dir.join("bad-ext"));

    let result = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some("extensions".into()),
        precedence: 0,
    }]);

    assert!(result.is_err());
    match result.unwrap_err() {
        ResourceDiscoveryError::InvalidManifest { path, .. } => {
            assert!(path.to_string_lossy().contains("bad-ext"));
        }
        other => panic!("expected InvalidManifest, got: {other}"),
    }
}

#[test]
fn manifest_missing_name_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("extensions");
    write_manifest_missing_name(&ext_dir.join("nameless"));

    let result = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some("extensions".into()),
        precedence: 0,
    }]);

    assert!(result.is_err());
    match result.unwrap_err() {
        ResourceDiscoveryError::MissingField { field, path } => {
            assert_eq!(field, "name");
            assert!(path.to_string_lossy().contains("nameless"));
        }
        other => panic!("expected MissingField, got: {other}"),
    }
}

// ---------------------------------------------------------------------------
// 5. Path normalization
// ---------------------------------------------------------------------------

#[test]
fn paths_are_normalized_to_canonical() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join(".opi").join("extensions");
    write_manifest(&ext_dir.join("norm-ext"), "norm-ext", "1.0.0", "Normalized");

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some(".opi/extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    // The path field should be the resolved directory path.
    assert!(resources[0].path.is_absolute());
}

#[test]
fn empty_name_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("extensions");
    fs::create_dir_all(ext_dir.join("empty-name")).unwrap();
    fs::write(
        ext_dir.join("empty-name").join("extension.toml"),
        r#"[extension]
name = ""
"#,
    )
    .unwrap();

    let result = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some("extensions".into()),
        precedence: 0,
    }]);

    assert!(result.is_err());
    match result.unwrap_err() {
        ResourceDiscoveryError::MissingField { field, .. } => {
            assert_eq!(field, "name");
        }
        other => panic!("expected MissingField, got: {other}"),
    }
}

// ---------------------------------------------------------------------------
// 6. Minimal manifest (only name required)
// ---------------------------------------------------------------------------

#[test]
fn minimal_manifest_with_only_name_is_valid() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("extensions");
    write_minimal_manifest(&ext_dir.join("minimal"), "minimal");

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some("extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.name, "minimal");
    assert!(resources[0].manifest.version.is_none());
    assert!(resources[0].manifest.description.is_none());
}

// ---------------------------------------------------------------------------
// 7. ExtensionResource structure
// ---------------------------------------------------------------------------

#[test]
fn resource_tracks_source_path_and_precedence() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("ext");
    write_manifest(&ext_dir.join("tracked"), "tracked", "1.0.0", "Tracked");

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: ext_dir,
        subdirectory: None,
        precedence: 42,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    assert!(resources[0].path.ends_with("tracked"));
    assert_eq!(resources[0].layer_precedence, 42);
}

// ---------------------------------------------------------------------------
// 8. Integration with ExtensionManifest fields
// ---------------------------------------------------------------------------

#[test]
fn manifest_parses_all_optional_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("ext");
    fs::create_dir_all(ext_dir.join("full-ext")).unwrap();
    fs::write(
        ext_dir.join("full-ext").join("extension.toml"),
        r#"[extension]
name = "full-ext"
version = "2.3.1"
description = "A fully specified extension"
"#,
    )
    .unwrap();

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: ext_dir,
        subdirectory: None,
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    let m = &resources[0].manifest;
    assert_eq!(m.name, "full-ext");
    assert_eq!(m.version.as_deref(), Some("2.3.1"));
    assert_eq!(
        m.description.as_deref(),
        Some("A fully specified extension")
    );
}

// ---------------------------------------------------------------------------
// 9. No layers returns empty
// ---------------------------------------------------------------------------

#[test]
fn no_layers_returns_empty() {
    let resources = discover_extension_resources(&[]).unwrap();
    assert!(resources.is_empty());
}

// ---------------------------------------------------------------------------
// 10. Files (non-directories) in extension dir are skipped
// ---------------------------------------------------------------------------

#[test]
fn non_directory_entries_are_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let ext_dir = tmp.path().join("extensions");
    fs::create_dir_all(&ext_dir).unwrap();
    // A plain file, not a directory
    fs::write(ext_dir.join("readme.md"), "not an extension").unwrap();
    // A valid extension
    write_manifest(&ext_dir.join("real-ext"), "real-ext", "1.0.0", "Real");

    let resources = discover_extension_resources(&[DiscoveryLayer {
        root: tmp.path().to_path_buf(),
        subdirectory: Some("extensions".into()),
        precedence: 0,
    }])
    .unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.name, "real-ext");
}
