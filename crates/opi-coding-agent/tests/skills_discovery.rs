//! Integration tests for skills progressive discovery (task 4.7.1).
//!
//! Covers: precedence, invalid metadata, missing paths, duplicate names,
//! disable-model-invocation handling, progressive disclosure, and
//! localized documentation updates.

use std::fs;

use opi_coding_agent::resource::DiscoveryLayer;
use opi_coding_agent::skill::{SkillDiscoveryError, SkillManifest, SkillRegistry, discover_skills};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a skill directory with a SKILL.md file in `parent`.
fn write_skill(parent: &std::path::Path, dir_name: &str, frontmatter: &str, body: &str) {
    let dir = parent.join(dir_name);
    fs::create_dir_all(&dir).unwrap();
    let content = format!("---\n{frontmatter}\n---\n{body}");
    fs::write(dir.join("SKILL.md"), content).unwrap();
}

/// Build a single discovery layer at `root/subdirectory` with given precedence.
fn layer(root: &std::path::Path, subdirectory: &str, precedence: u32) -> DiscoveryLayer {
    DiscoveryLayer {
        root: root.to_path_buf(),
        subdirectory: Some(subdirectory.to_string()),
        precedence,
    }
}

// ---------------------------------------------------------------------------
// 1. Manifest parsing
// ---------------------------------------------------------------------------

#[test]
fn test_parse_valid_frontmatter() {
    let content = "---\nname: my-skill\ndescription: Does things.\n---\nBody here.";
    let manifest = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap();
    assert_eq!(manifest.name, "my-skill");
    assert_eq!(manifest.description, "Does things.");
    assert!(!manifest.disable_model_invocation);
}

#[test]
fn test_parse_with_disable_model_invocation() {
    let content =
        "---\nname: manual-only\ndescription: Manual only.\ndisable-model-invocation: true\n---\n";
    let manifest = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap();
    assert!(manifest.disable_model_invocation);
}

#[test]
fn test_parse_disable_model_invocation_false_explicit() {
    let content =
        "---\nname: auto-skill\ndescription: Auto.\ndisable-model-invocation: false\n---\n";
    let manifest = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap();
    assert!(!manifest.disable_model_invocation);
}

#[test]
fn test_parse_missing_name() {
    let content = "---\ndescription: No name.\n---\nBody";
    let err = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap_err();
    match err {
        SkillDiscoveryError::MissingField { field, .. } => assert_eq!(field, "name"),
        other => panic!("expected MissingField, got: {other}"),
    }
}

#[test]
fn test_parse_missing_description() {
    let content = "---\nname: no-desc\n---\nBody";
    let err = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap_err();
    match err {
        SkillDiscoveryError::MissingField { field, .. } => assert_eq!(field, "description"),
        other => panic!("expected MissingField, got: {other}"),
    }
}

#[test]
fn test_parse_empty_name() {
    let content = "---\nname: \"\"\ndescription: Empty name.\n---\n";
    let err = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap_err();
    match err {
        SkillDiscoveryError::MissingField { field, .. } => assert_eq!(field, "name"),
        other => panic!("expected MissingField, got: {other}"),
    }
}

#[test]
fn test_parse_invalid_name_characters() {
    let content = "---\nname: Invalid Name!\ndescription: Bad chars.\n---\n";
    let err = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap_err();
    match err {
        SkillDiscoveryError::InvalidName { .. } => {}
        other => panic!("expected InvalidName, got: {other}"),
    }
}

#[test]
fn test_parse_name_too_long() {
    let long_name = "a".repeat(65);
    let content = format!("---\nname: {long_name}\ndescription: Too long.\n---\n");
    let err = SkillManifest::from_skill_md(&content, std::path::Path::new("SKILL.md")).unwrap_err();
    match err {
        SkillDiscoveryError::InvalidName { .. } => {}
        other => panic!("expected InvalidName, got: {other}"),
    }
}

#[test]
fn test_parse_name_at_max_length() {
    let name = "a".repeat(64);
    let content = format!("---\nname: {name}\ndescription: Max length ok.\n---\n");
    let manifest =
        SkillManifest::from_skill_md(&content, std::path::Path::new("SKILL.md")).unwrap();
    assert_eq!(manifest.name.len(), 64);
}

#[test]
fn test_parse_description_too_long() {
    let long_desc = "x".repeat(1025);
    let content = format!("---\nname: ok\ndescription: {long_desc}\n---\n");
    let err = SkillManifest::from_skill_md(&content, std::path::Path::new("SKILL.md")).unwrap_err();
    match err {
        SkillDiscoveryError::InvalidDescription { .. } => {}
        other => panic!("expected InvalidDescription, got: {other}"),
    }
}

#[test]
fn test_parse_description_at_max_length() {
    let desc = "x".repeat(1024);
    let content = format!("---\nname: ok\ndescription: {desc}\n---\n");
    let manifest =
        SkillManifest::from_skill_md(&content, std::path::Path::new("SKILL.md")).unwrap();
    assert_eq!(manifest.description.len(), 1024);
}

#[test]
fn test_parse_no_frontmatter_delimiters() {
    let content = "Just some markdown content without frontmatter.";
    let err = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap_err();
    match err {
        SkillDiscoveryError::InvalidFrontmatter { .. } => {}
        other => panic!("expected InvalidFrontmatter, got: {other}"),
    }
}

#[test]
fn test_parse_valid_name_with_hyphens_and_digits() {
    let content = "---\nname: my-skill-v2\ndescription: Valid name.\n---\n";
    let manifest = SkillManifest::from_skill_md(content, std::path::Path::new("SKILL.md")).unwrap();
    assert_eq!(manifest.name, "my-skill-v2");
}

// ---------------------------------------------------------------------------
// 2. Discovery — basic
// ---------------------------------------------------------------------------

#[test]
fn test_discover_single_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    write_skill(
        &skills_dir,
        "my-skill",
        "name: my-skill\ndescription: A skill.",
        "Do the thing.",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.name, "my-skill");
    assert_eq!(resources[0].manifest.description, "A skill.");
}

#[test]
fn test_discover_multiple_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    write_skill(
        &skills_dir,
        "alpha",
        "name: alpha\ndescription: First.",
        "A",
    );
    write_skill(&skills_dir, "beta", "name: beta\ndescription: Second.", "B");

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();

    // Results are sorted by name.
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].manifest.name, "alpha");
    assert_eq!(resources[1].manifest.name, "beta");
}

#[test]
fn test_discover_skips_dirs_without_skill_md() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    // Create a directory without SKILL.md — should be skipped.
    fs::create_dir_all(skills_dir.join("no-skill")).unwrap();
    // Create a valid skill.
    write_skill(
        &skills_dir,
        "real-skill",
        "name: real-skill\ndescription: Real.",
        "R",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.name, "real-skill");
}

#[test]
fn test_discover_skips_files() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    // A regular file in the scan dir should be skipped.
    fs::write(skills_dir.join("readme.txt"), "not a skill").unwrap();
    write_skill(
        &skills_dir,
        "valid",
        "name: valid\ndescription: Valid.",
        "V",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();

    assert_eq!(resources.len(), 1);
}

// ---------------------------------------------------------------------------
// 3. Discovery — precedence
// ---------------------------------------------------------------------------

#[test]
fn test_discover_precedence_higher_wins() {
    let tmp = tempfile::tempdir().unwrap();

    // User layer (precedence 0) has "tool-a" with description "user version"
    let user_dir = tmp.path().join("user-skills");
    fs::create_dir_all(&user_dir).unwrap();
    write_skill(
        &user_dir,
        "tool-a",
        "name: tool-a\ndescription: user version",
        "User body.",
    );

    // Project layer (precedence 1) has "tool-a" with description "project version"
    let proj_dir = tmp.path().join("proj-skills");
    fs::create_dir_all(&proj_dir).unwrap();
    write_skill(
        &proj_dir,
        "tool-a",
        "name: tool-a\ndescription: project version",
        "Project body.",
    );

    let layers = vec![
        layer(tmp.path(), "user-skills", 0),
        layer(tmp.path(), "proj-skills", 1),
    ];
    let resources = discover_skills(&layers).unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].manifest.description, "project version");
    assert_eq!(resources[0].layer_precedence, 1);
}

#[test]
fn test_discover_precedence_mixed_names() {
    let tmp = tempfile::tempdir().unwrap();

    // Layer 0: skill-a, skill-b
    let low = tmp.path().join("low");
    fs::create_dir_all(&low).unwrap();
    write_skill(&low, "skill-a", "name: skill-a\ndescription: low-a", "A");
    write_skill(&low, "skill-b", "name: skill-b\ndescription: low-b", "B");

    // Layer 1: skill-b (overrides), skill-c (new)
    let high = tmp.path().join("high");
    fs::create_dir_all(&high).unwrap();
    write_skill(&high, "skill-b", "name: skill-b\ndescription: high-b", "B2");
    write_skill(&high, "skill-c", "name: skill-c\ndescription: high-c", "C");

    let layers = vec![layer(tmp.path(), "low", 0), layer(tmp.path(), "high", 1)];
    let resources = discover_skills(&layers).unwrap();

    // 3 unique names: skill-a from low, skill-b from high, skill-c from high.
    assert_eq!(resources.len(), 3);
    let names: Vec<&str> = resources.iter().map(|r| r.manifest.name.as_str()).collect();
    assert_eq!(names, vec!["skill-a", "skill-b", "skill-c"]);
    // skill-b is from high precedence layer.
    let skill_b = resources
        .iter()
        .find(|r| r.manifest.name == "skill-b")
        .unwrap();
    assert_eq!(skill_b.manifest.description, "high-b");
}

#[test]
fn test_discover_duplicate_name_same_layer_returns_error() {
    let tmp = tempfile::tempdir().unwrap();

    let dir = tmp.path().join("skills");
    fs::create_dir_all(&dir).unwrap();
    write_skill(
        &dir,
        "first",
        "name: shared-skill\ndescription: First",
        "First body.",
    );
    write_skill(
        &dir,
        "second",
        "name: shared-skill\ndescription: Second",
        "Second body.",
    );

    let err = discover_skills(&[layer(tmp.path(), "skills", 0)]).unwrap_err();
    assert!(matches!(
        err,
        SkillDiscoveryError::DuplicateName { ref name, .. } if name == "shared-skill"
    ));
}

// ---------------------------------------------------------------------------
// 4. Discovery — missing / invalid resources
// ---------------------------------------------------------------------------

#[test]
fn test_discover_missing_scan_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let layers = vec![layer(tmp.path(), "nonexistent", 0)];
    let resources = discover_skills(&layers).unwrap();
    assert!(resources.is_empty());
}

#[test]
fn test_discover_invalid_manifest_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    // Write a SKILL.md with invalid frontmatter.
    let bad_dir = skills_dir.join("broken");
    fs::create_dir_all(&bad_dir).unwrap();
    fs::write(bad_dir.join("SKILL.md"), "No frontmatter here.").unwrap();

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let err = discover_skills(&layers).unwrap_err();
    match err {
        SkillDiscoveryError::InvalidFrontmatter { .. } => {}
        other => panic!("expected InvalidFrontmatter, got: {other}"),
    }
}

// ---------------------------------------------------------------------------
// 5. Progressive disclosure
// ---------------------------------------------------------------------------

#[test]
fn test_progressive_disclosure_metadata_without_body() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    write_skill(
        &skills_dir,
        "lazy",
        "name: lazy\ndescription: Load on demand.",
        "# Full Instructions\n\nDo the complex thing step by step.",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();
    assert_eq!(resources.len(), 1);

    // Metadata is available.
    assert_eq!(resources[0].manifest.name, "lazy");
    assert_eq!(resources[0].manifest.description, "Load on demand.");

    // Body is NOT eagerly loaded into manifest.
    // The body can be loaded on demand.
    let body = resources[0].load_body().unwrap();
    assert!(body.contains("# Full Instructions"));
    assert!(body.contains("Do the complex thing step by step."));
}

#[test]
fn test_progressive_disclosure_load_body_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    write_skill(
        &skills_dir,
        "ephemeral",
        "name: ephemeral\ndescription: Will be deleted.",
        "Original body.",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();

    // Delete the SKILL.md after discovery.
    fs::remove_file(skills_dir.join("ephemeral").join("SKILL.md")).unwrap();

    // Loading body should fail gracefully.
    let result = resources[0].load_body();
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 6. SkillRegistry
// ---------------------------------------------------------------------------

#[test]
fn test_registry_from_discovered_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    write_skill(
        &skills_dir,
        "summarize",
        "name: summarize\ndescription: Summarize text.",
        "Summarize.",
    );
    write_skill(
        &skills_dir,
        "translate",
        "name: translate\ndescription: Translate text.",
        "Translate.",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();
    let registry = SkillRegistry::from_resources(resources);

    // List names — sorted alphabetically.
    let names = registry.names();
    assert_eq!(names, vec!["summarize", "translate"]);

    // Get metadata.
    let meta = registry.get("summarize").unwrap();
    assert_eq!(meta.manifest.description, "Summarize text.");
    assert!(!meta.manifest.disable_model_invocation);

    // Missing skill returns None.
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_registry_format_for_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    write_skill(
        &skills_dir,
        "review",
        "name: review\ndescription: Review code.",
        "Review code for bugs.",
    );
    write_skill(
        &skills_dir,
        "manual",
        "name: manual\ndescription: Manual only.\ndisable-model-invocation: true",
        "For human use only.",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();
    let registry = SkillRegistry::from_resources(resources);

    // Format for prompt should include all skill metadata.
    let prompt = registry.format_for_prompt();
    assert!(prompt.contains("review"));
    assert!(prompt.contains("Review code."));
    assert!(prompt.contains("manual"));
    assert!(prompt.contains("Manual only."));
}

#[test]
fn test_registry_disable_model_invocation_excluded_from_auto_listing() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    write_skill(
        &skills_dir,
        "auto-skill",
        "name: auto-skill\ndescription: Auto invoked.",
        "Auto.",
    );
    write_skill(
        &skills_dir,
        "manual-skill",
        "name: manual-skill\ndescription: Manual only.\ndisable-model-invocation: true",
        "Manual.",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();
    let registry = SkillRegistry::from_resources(resources);

    // auto_invocable() returns only skills without disable-model-invocation.
    let auto = registry.auto_invocable();
    let auto_names: Vec<&str> = auto.iter().map(|s| s.manifest.name.as_str()).collect();
    assert_eq!(auto_names, vec!["auto-skill"]);

    // all() still returns everything.
    assert_eq!(registry.names().len(), 2);
}

#[test]
fn test_registry_empty() {
    let registry = SkillRegistry::from_resources(vec![]);
    assert!(registry.names().is_empty());
    assert!(registry.format_for_prompt().is_empty());
}

// ---------------------------------------------------------------------------
// 7. Integration — load body through registry
// ---------------------------------------------------------------------------

#[test]
fn test_registry_load_body() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    write_skill(
        &skills_dir,
        "deep-skill",
        "name: deep-skill\ndescription: Has deep instructions.",
        "# Deep Skill\n\nStep 1: Analyze.\nStep 2: Execute.",
    );

    let layers = vec![layer(tmp.path(), "skills", 0)];
    let resources = discover_skills(&layers).unwrap();
    let registry = SkillRegistry::from_resources(resources);

    let body = registry.load_body("deep-skill").unwrap().unwrap();
    assert!(body.contains("# Deep Skill"));
    assert!(body.contains("Step 1: Analyze."));
}

#[test]
fn test_registry_load_body_unknown_skill() {
    let registry = SkillRegistry::from_resources(vec![]);
    let result = registry.load_body("nonexistent");
    assert!(result.is_none());
}
