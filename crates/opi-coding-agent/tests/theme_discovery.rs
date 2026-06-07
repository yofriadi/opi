//! Theme progressive discovery behavioral tests (task 4.7.3).
//!
//! Covers: manifest parsing, color validation against the TUI theme token
//! schema, discovery with precedence, invalid/missing theme files, active
//! theme lookup, progressive disclosure, and ThemeRegistry.

use std::path::Path;

use opi_coding_agent::resource::DiscoveryLayer;
use opi_coding_agent::theme_discovery::{
    ThemeDiscoveryError, ThemeManifest, ThemeRegistry, discover_themes,
};
use opi_tui::{THEME_TOKENS, Theme, is_valid_token, parse_color};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_theme(dir: &Path, name: &str, toml_content: &str) -> std::path::PathBuf {
    let theme_dir = dir.join(name);
    std::fs::create_dir_all(&theme_dir).unwrap();
    let path = theme_dir.join("theme.toml");
    std::fs::write(&path, toml_content).unwrap();
    path
}

fn layer(root: &Path, subdirectory: Option<&str>, precedence: u32) -> DiscoveryLayer {
    DiscoveryLayer {
        root: root.to_path_buf(),
        subdirectory: subdirectory.map(String::from),
        precedence,
    }
}

/// A minimal valid theme.toml with all color tokens specified.
fn full_theme_toml(name: &str, description: &str) -> String {
    format!(
        r#"
name = "{name}"
description = "{description}"

[colors]
role_user = "Green"
role_assistant = "Cyan"
role_system = "Yellow"
role_tool = "Magenta"
status_bg = "DarkGray"
status_idle = "White"
status_thinking = "Yellow"
status_streaming = "Green"
status_tool = "Magenta"
status_tokens = "DarkGray"
editor_title = "Yellow"
editor_placeholder = "DarkGray"
code_title = "Yellow"
code_content = "Gray"
heading_h1 = "Cyan"
heading_h2 = "Yellow"
heading_h3 = "White"
italic = "Cyan"
diff_border = "Cyan"
diff_header = "Blue"
diff_context = "Gray"
diff_added = "Green"
diff_removed = "Red"
diff_no_changes = "DarkGray"
tool_running = "Yellow"
tool_success = "Green"
tool_error = "Red"
picker_title = "Cyan"
picker_selected_bg = "DarkGray"
picker_selected_fg = "White"
picker_filter = "Yellow"
picker_metadata = "DarkGray"
picker_empty = "DarkGray"
"#
    )
}

/// A theme.toml with only two color tokens (partial theme).
fn partial_theme_toml(name: &str, description: &str) -> String {
    format!(
        r##"
name = "{name}"
description = "{description}"

[colors]
role_user = "Red"
status_bg = "#1a1a2e"
"##
    )
}

// ===========================================================================
// 1. ThemeManifest parsing
// ===========================================================================

mod manifest_parsing {
    use super::*;

    #[test]
    fn parse_valid_minimal_manifest() {
        let toml = r#"
name = "my-theme"
description = "A test theme."
"#;
        let path = Path::new("my-theme/theme.toml");
        let manifest = ThemeManifest::from_toml(toml, path).unwrap();
        assert_eq!(manifest.name, "my-theme");
        assert_eq!(manifest.description, "A test theme.");
    }

    #[test]
    fn parse_manifest_with_colors_section() {
        let toml = r#"
name = "ocean"
description = "Ocean blues."

[colors]
role_user = "Cyan"
role_assistant = "Blue"
"#;
        let path = Path::new("ocean/theme.toml");
        let manifest = ThemeManifest::from_toml(toml, path).unwrap();
        assert_eq!(manifest.name, "ocean");
        assert_eq!(manifest.description, "Ocean blues.");
    }

    #[test]
    fn parse_manifest_missing_name() {
        let toml = r#"
description = "No name."
"#;
        let path = Path::new("x/theme.toml");
        let err = ThemeManifest::from_toml(toml, path).unwrap_err();
        assert!(matches!(
            err,
            ThemeDiscoveryError::MissingField { ref field, .. } if field == "name"
        ));
    }

    #[test]
    fn parse_manifest_missing_description() {
        let toml = r#"
name = "no-desc"
"#;
        let path = Path::new("x/theme.toml");
        let err = ThemeManifest::from_toml(toml, path).unwrap_err();
        assert!(matches!(
            err,
            ThemeDiscoveryError::MissingField { ref field, .. } if field == "description"
        ));
    }

    #[test]
    fn parse_manifest_empty_name() {
        let toml = r#"
name = ""
description = "Empty name."
"#;
        let path = Path::new("x/theme.toml");
        let err = ThemeManifest::from_toml(toml, path).unwrap_err();
        assert!(matches!(err, ThemeDiscoveryError::MissingField { .. }));
    }

    #[test]
    fn parse_manifest_invalid_toml() {
        let toml = "this is not valid toml [[[";
        let path = Path::new("x/theme.toml");
        let err = ThemeManifest::from_toml(toml, path).unwrap_err();
        assert!(matches!(err, ThemeDiscoveryError::InvalidManifest { .. }));
    }

    #[test]
    fn parse_manifest_name_validation_too_long() {
        let long_name = "a".repeat(65);
        let toml = format!(
            r#"
name = "{long_name}"
description = "Too long."
"#
        );
        let path = Path::new("x/theme.toml");
        let err = ThemeManifest::from_toml(&toml, path).unwrap_err();
        assert!(matches!(err, ThemeDiscoveryError::InvalidName { .. }));
    }

    #[test]
    fn parse_manifest_name_validation_invalid_chars() {
        let toml = r#"
name = "My Theme!"
description = "Bad chars."
"#;
        let path = Path::new("x/theme.toml");
        let err = ThemeManifest::from_toml(toml, path).unwrap_err();
        assert!(matches!(err, ThemeDiscoveryError::InvalidName { .. }));
    }

    #[test]
    fn parse_manifest_description_too_long() {
        let long_desc = "x".repeat(1025);
        let toml = format!(
            r#"
name = "ok-name"
description = "{long_desc}"
"#
        );
        let path = Path::new("x/theme.toml");
        let err = ThemeManifest::from_toml(&toml, path).unwrap_err();
        assert!(matches!(
            err,
            ThemeDiscoveryError::InvalidDescription { .. }
        ));
    }
}

// ===========================================================================
// 2. Color parsing
// ===========================================================================

mod color_parsing {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn parse_named_color() {
        assert_eq!(parse_color("Red").unwrap(), Color::Red);
        assert_eq!(parse_color("Green").unwrap(), Color::Green);
        assert_eq!(parse_color("Cyan").unwrap(), Color::Cyan);
        assert_eq!(parse_color("DarkGray").unwrap(), Color::DarkGray);
        assert_eq!(parse_color("LightCyan").unwrap(), Color::LightCyan);
        assert_eq!(parse_color("White").unwrap(), Color::White);
    }

    #[test]
    fn parse_hex_color() {
        assert_eq!(parse_color("#ff6600").unwrap(), Color::Rgb(255, 102, 0));
        assert_eq!(parse_color("#000000").unwrap(), Color::Rgb(0, 0, 0));
        assert_eq!(parse_color("#ffffff").unwrap(), Color::Rgb(255, 255, 255));
        assert_eq!(parse_color("#a6e22e").unwrap(), Color::Rgb(166, 226, 46));
    }

    #[test]
    fn parse_hex_color_case_insensitive() {
        assert_eq!(parse_color("#FF6600").unwrap(), Color::Rgb(255, 102, 0));
        assert_eq!(parse_color("#Ff66Aa").unwrap(), Color::Rgb(255, 102, 170));
    }

    #[test]
    fn parse_color_invalid() {
        assert!(parse_color("NotAColor").is_err());
        assert!(parse_color("#gggggg").is_err());
        assert!(parse_color("#12345").is_err()); // too short
        assert!(parse_color("").is_err());
    }
}

// ===========================================================================
// 3. Theme token schema
// ===========================================================================

mod token_schema {
    use super::*;

    #[test]
    fn theme_tokens_contains_all_known_fields() {
        // These are the 27 color fields from Theme struct (minus `name`)
        let expected = [
            "role_user",
            "role_assistant",
            "role_system",
            "role_tool",
            "status_bg",
            "status_idle",
            "status_thinking",
            "status_streaming",
            "status_tool",
            "status_tokens",
            "editor_title",
            "editor_placeholder",
            "code_title",
            "code_content",
            "heading_h1",
            "heading_h2",
            "heading_h3",
            "italic",
            "diff_border",
            "diff_header",
            "diff_context",
            "diff_added",
            "diff_removed",
            "diff_no_changes",
            "tool_running",
            "tool_success",
            "tool_error",
            "picker_title",
            "picker_selected_bg",
            "picker_selected_fg",
            "picker_filter",
            "picker_metadata",
            "picker_empty",
        ];
        for token in &expected {
            assert!(
                THEME_TOKENS.contains(token),
                "THEME_TOKENS missing: {token}"
            );
        }
    }

    #[test]
    fn theme_tokens_rejects_unknown() {
        assert!(!is_valid_token("nonexistent_token"));
        assert!(!is_valid_token("name"));
    }
}

// ===========================================================================
// 4. Discovery basic
// ===========================================================================

mod discovery_basic {
    use super::*;

    #[test]
    fn discover_from_single_layer() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(
            &themes_dir,
            "ocean",
            &full_theme_toml("ocean", "Ocean blues"),
        );

        let layers = vec![layer(&themes_dir, None, 0)];
        let resources = discover_themes(&layers).unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].manifest.name, "ocean");
    }

    #[test]
    fn discover_multiple_themes() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(&themes_dir, "alpha", &full_theme_toml("alpha", "A"));
        write_theme(&themes_dir, "beta", &full_theme_toml("beta", "B"));

        let layers = vec![layer(&themes_dir, None, 0)];
        let resources = discover_themes(&layers).unwrap();
        assert_eq!(resources.len(), 2);
        // Sorted by name
        assert_eq!(resources[0].manifest.name, "alpha");
        assert_eq!(resources[1].manifest.name, "beta");
    }

    #[test]
    fn discover_skips_non_theme_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        // A directory without theme.toml
        let other_dir = themes_dir.join("not-a-theme");
        std::fs::create_dir_all(&other_dir).unwrap();

        // A file at top level (not a directory)
        std::fs::write(themes_dir.join("readme.txt"), "not a theme").unwrap();

        write_theme(
            &themes_dir,
            "real-theme",
            &full_theme_toml("real-theme", "Real"),
        );

        let layers = vec![layer(&themes_dir, None, 0)];
        let resources = discover_themes(&layers).unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].manifest.name, "real-theme");
    }

    #[test]
    fn discover_missing_scan_dir_returns_empty() {
        let layers = vec![layer(Path::new("/nonexistent/path"), None, 0)];
        let resources = discover_themes(&layers).unwrap();
        assert!(resources.is_empty());
    }
}

// ===========================================================================
// 5. Discovery precedence
// ===========================================================================

mod discovery_precedence {
    use super::*;

    #[test]
    fn higher_precedence_wins_on_name_collision() {
        let tmp = tempfile::tempdir().unwrap();

        let user_dir = tmp.path().join("user-themes");
        let project_dir = tmp.path().join("project-themes");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();

        write_theme(&user_dir, "ocean", &full_theme_toml("ocean", "User ocean"));
        write_theme(
            &project_dir,
            "ocean",
            &full_theme_toml("ocean", "Project ocean"),
        );

        let layers = vec![layer(&user_dir, None, 0), layer(&project_dir, None, 1)];
        let resources = discover_themes(&layers).unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].manifest.description, "Project ocean");
        assert_eq!(resources[0].layer_precedence, 1);
    }

    #[test]
    fn lower_precedence_kept_when_no_collision() {
        let tmp = tempfile::tempdir().unwrap();

        let user_dir = tmp.path().join("user-themes");
        let project_dir = tmp.path().join("project-themes");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();

        write_theme(
            &user_dir,
            "user-only",
            &full_theme_toml("user-only", "User theme"),
        );
        write_theme(
            &project_dir,
            "project-only",
            &full_theme_toml("project-only", "Project theme"),
        );

        let layers = vec![layer(&user_dir, None, 0), layer(&project_dir, None, 1)];
        let resources = discover_themes(&layers).unwrap();
        assert_eq!(resources.len(), 2);
    }

    #[test]
    fn duplicate_name_in_same_layer_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(&themes_dir, "first", &full_theme_toml("shared", "First"));
        write_theme(&themes_dir, "second", &full_theme_toml("shared", "Second"));

        let err = discover_themes(&[layer(&themes_dir, None, 0)]).unwrap_err();
        assert!(matches!(
            err,
            ThemeDiscoveryError::DuplicateName { ref name, .. } if name == "shared"
        ));
    }
}

// ===========================================================================
// 6. Discovery errors
// ===========================================================================

mod discovery_errors {
    use super::*;

    #[test]
    fn discover_invalid_theme_toml_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(&themes_dir, "bad", "this is not valid toml [[[");

        let layers = vec![layer(&themes_dir, None, 0)];
        let result = discover_themes(&layers);
        assert!(result.is_err());
    }

    #[test]
    fn load_theme_with_invalid_color_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(
            &themes_dir,
            "bad-color",
            r#"
name = "bad-color"
description = "Has invalid color"

[colors]
role_user = "NotARealColor"
"#,
        );

        let layers = vec![layer(&themes_dir, None, 0)];
        // Discovery succeeds (metadata is valid)
        let resources = discover_themes(&layers).unwrap();
        // Loading fails (color is invalid)
        let result = resources[0].load_theme();
        assert!(result.is_err());
    }

    #[test]
    fn load_theme_with_unknown_token_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(
            &themes_dir,
            "unknown-token",
            r#"
name = "unknown-token"
description = "Has unknown token"

[colors]
nonexistent_token = "Red"
"#,
        );

        let layers = vec![layer(&themes_dir, None, 0)];
        // Discovery succeeds (metadata is valid)
        let resources = discover_themes(&layers).unwrap();
        // Loading fails (token is unknown)
        let result = resources[0].load_theme();
        assert!(result.is_err());
    }
}

// ===========================================================================
// 7. Progressive disclosure
// ===========================================================================

mod progressive_disclosure {
    use super::*;

    #[test]
    fn metadata_available_without_loading_colors() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(
            &themes_dir,
            "ocean",
            &full_theme_toml("ocean", "Ocean theme"),
        );

        let layers = vec![layer(&themes_dir, None, 0)];
        let resources = discover_themes(&layers).unwrap();
        let resource = &resources[0];

        // Metadata is available immediately
        assert_eq!(resource.manifest.name, "ocean");
        assert_eq!(resource.manifest.description, "Ocean theme");
    }

    #[test]
    fn load_theme_on_demand() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(
            &themes_dir,
            "ocean",
            &full_theme_toml("ocean", "Ocean theme"),
        );

        let layers = vec![layer(&themes_dir, None, 0)];
        let resources = discover_themes(&layers).unwrap();
        let resource = &resources[0];

        // Full theme loaded on demand
        let theme = resource.load_theme().unwrap();
        assert_eq!(theme.name, "ocean");
        assert_eq!(theme.role_user, ratatui::style::Color::Green);
    }

    #[test]
    fn partial_theme_fills_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(
            &themes_dir,
            "partial",
            &partial_theme_toml("partial", "Only two tokens"),
        );

        let layers = vec![layer(&themes_dir, None, 0)];
        let resources = discover_themes(&layers).unwrap();
        let resource = &resources[0];

        let theme = resource.load_theme().unwrap();
        assert_eq!(theme.name, "partial");
        // Specified tokens override
        assert_eq!(theme.role_user, ratatui::style::Color::Red);
        assert_eq!(theme.status_bg, ratatui::style::Color::Rgb(26, 26, 46));
        // Unspecified tokens inherit from default
        let default = Theme::default();
        assert_eq!(theme.role_assistant, default.role_assistant);
        assert_eq!(theme.status_idle, default.status_idle);
        assert_eq!(theme.diff_added, default.diff_added);
    }
}

// ===========================================================================
// 8. ThemeRegistry
// ===========================================================================

mod theme_registry {
    use super::*;

    fn setup_registry() -> (tempfile::TempDir, ThemeRegistry) {
        let tmp = tempfile::tempdir().unwrap();
        let themes_dir = tmp.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();

        write_theme(
            &themes_dir,
            "alpha",
            &full_theme_toml("alpha", "First theme"),
        );
        write_theme(
            &themes_dir,
            "beta",
            &partial_theme_toml("beta", "Second theme"),
        );

        let layers = vec![layer(&themes_dir, None, 0)];
        let resources = discover_themes(&layers).unwrap();
        let registry = ThemeRegistry::from_resources(resources);
        (tmp, registry)
    }

    #[test]
    fn registry_names_returns_sorted() {
        let (_tmp, registry) = setup_registry();
        let names = registry.names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn registry_get_returns_metadata() {
        let (_tmp, registry) = setup_registry();
        let resource = registry.get("alpha").unwrap();
        assert_eq!(resource.manifest.name, "alpha");
        assert_eq!(resource.manifest.description, "First theme");
    }

    #[test]
    fn registry_get_missing_returns_none() {
        let (_tmp, registry) = setup_registry();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn registry_load_theme() {
        let (_tmp, registry) = setup_registry();
        let theme = registry.load_theme("alpha").unwrap().unwrap();
        assert_eq!(theme.name, "alpha");
    }

    #[test]
    fn registry_resolve_theme_found() {
        let (_tmp, registry) = setup_registry();
        let theme = registry.resolve_theme("beta").unwrap();
        assert_eq!(theme.name, "beta");
    }

    #[test]
    fn registry_resolve_theme_falls_back_to_default() {
        let (_tmp, registry) = setup_registry();
        let theme = registry.resolve_theme("nonexistent").unwrap();
        assert_eq!(theme.name, "default");
    }

    #[test]
    fn registry_resolve_theme_built_in_monokai() {
        let (_tmp, registry) = setup_registry();
        let theme = registry.resolve_theme("monokai").unwrap();
        assert_eq!(theme.name, "monokai");
    }

    #[test]
    fn registry_format_for_prompt() {
        let (_tmp, registry) = setup_registry();
        let prompt = registry.format_for_prompt();
        assert!(prompt.contains("alpha"));
        assert!(prompt.contains("First theme"));
        assert!(prompt.contains("beta"));
        assert!(prompt.contains("Second theme"));
    }

    #[test]
    fn registry_empty_format_returns_empty_string() {
        let registry = ThemeRegistry::from_resources(vec![]);
        assert!(registry.format_for_prompt().is_empty());
    }
}
