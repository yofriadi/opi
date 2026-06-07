use std::path::Path;

use opi_ai::test_support::MockProvider;
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::harness::CodingHarness;
use opi_coding_agent::policy::{RunMode, ToolRuntimeConfig};

fn write_package_with_resources(pkg_dir: &Path) {
    std::fs::create_dir_all(pkg_dir).unwrap();
    std::fs::write(
        pkg_dir.join("package.toml"),
        r#"
name = "metadata-suite"
description = "Metadata package."
version = "1.2.3"
"#,
    )
    .unwrap();

    let ext_dir = pkg_dir.join("extensions").join("metadata-ext");
    std::fs::create_dir_all(&ext_dir).unwrap();
    std::fs::write(
        ext_dir.join("extension.toml"),
        r#"[extension]
name = "metadata-ext"
version = "0.1.0"
description = "Metadata extension."
"#,
    )
    .unwrap();

    let skill_dir = pkg_dir.join("skills").join("metadata-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: metadata-skill
description: Metadata skill.
---
FULL SKILL BODY SHOULD NOT LOAD
"#,
    )
    .unwrap();

    let fragment_dir = pkg_dir.join("fragments").join("metadata-fragment");
    std::fs::create_dir_all(&fragment_dir).unwrap();
    std::fs::write(
        fragment_dir.join("FRAGMENT.md"),
        r#"---
name: metadata-fragment
description: Metadata fragment.
arguments: text
---
FULL FRAGMENT BODY SHOULD NOT LOAD
"#,
    )
    .unwrap();

    let theme_dir = pkg_dir.join("themes").join("metadata-theme");
    std::fs::create_dir_all(&theme_dir).unwrap();
    std::fs::write(
        theme_dir.join("theme.toml"),
        r#"
name = "metadata-theme"
description = "Metadata theme."
"#,
    )
    .unwrap();
}

#[test]
fn harness_system_prompt_includes_configured_package_resource_metadata_only() {
    let workspace = tempfile::tempdir().unwrap();
    let global_config = tempfile::tempdir().unwrap();
    let package_dir = workspace.path().join("vendor").join("metadata-suite");
    write_package_with_resources(&package_dir);

    let mut config = OpiConfig::default();
    config.packages.paths = vec![package_dir.strip_prefix(workspace.path()).unwrap().into()];

    let provider = MockProvider::new("mock", Vec::new());
    let harness = CodingHarness::new_with_global_config_dir_tool_config(
        Box::new(provider),
        "mock:mock-model".into(),
        config,
        workspace.path().to_path_buf(),
        Box::new(opi_coding_agent::harness::CodingAgentHooks),
        None,
        Vec::new(),
        None,
        ToolRuntimeConfig {
            run_mode: RunMode::Interactive,
            active_tool_names: Vec::new(),
        },
        Some(global_config.path().to_path_buf()),
    );

    let prompt = harness.system_prompt();
    assert!(prompt.contains("metadata-suite"));
    assert!(prompt.contains("Metadata package."));
    assert!(prompt.contains("metadata-ext"));
    assert!(prompt.contains("Metadata extension."));
    assert!(prompt.contains("metadata-skill"));
    assert!(prompt.contains("Metadata skill."));
    assert!(prompt.contains("metadata-fragment"));
    assert!(prompt.contains("Metadata fragment."));
    assert!(prompt.contains("metadata-theme"));
    assert!(prompt.contains("Metadata theme."));
    assert!(!prompt.contains("FULL SKILL BODY SHOULD NOT LOAD"));
    assert!(!prompt.contains("FULL FRAGMENT BODY SHOULD NOT LOAD"));

    let metadata = harness.resource_metadata();
    assert_eq!(metadata.packages[0].name, "metadata-suite");
    assert_eq!(metadata.skills[0].name, "metadata-skill");

    let theme = harness
        .resolve_theme("metadata-theme")
        .expect("configured package theme should resolve");
    assert_eq!(theme.name, "metadata-theme");
}
