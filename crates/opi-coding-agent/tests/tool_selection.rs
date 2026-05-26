//! Tool selection flag tests for task 3.8.
//!
//! Validates --tools allowlist, --no-tools, --no-builtin-tools flag parsing,
//! ToolSelection resolution, tool filtering through the harness, and
//! precedence/conflict behavior.

use std::fs;

use opi_ai::test_support::{MockProvider, text_response};
use opi_coding_agent::cli::Cli;
use opi_coding_agent::config::OpiConfig;
use opi_coding_agent::harness::CodingHarness;
use opi_coding_agent::policy::{
    ToolFlags, ToolSelection, filter_tool_names, resolve_tool_selection,
};

use clap::Parser;

// --- Helpers ---

fn create_temp_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join(".git")).expect("failed to create .git");
    dir
}

// --- ToolSelection resolution ---

#[test]
fn resolve_default_when_no_flags() {
    let flags = ToolFlags {
        tools: None,
        no_tools: false,
        no_builtin_tools: false,
    };
    assert_eq!(resolve_tool_selection(flags), ToolSelection::Default);
}

#[test]
fn resolve_disabled_when_no_tools() {
    let flags = ToolFlags {
        tools: None,
        no_tools: true,
        no_builtin_tools: false,
    };
    assert_eq!(resolve_tool_selection(flags), ToolSelection::Disabled);
}

#[test]
fn resolve_no_builtin_when_flag() {
    let flags = ToolFlags {
        tools: None,
        no_tools: false,
        no_builtin_tools: true,
    };
    assert_eq!(resolve_tool_selection(flags), ToolSelection::NoBuiltin);
}

#[test]
fn resolve_allowlist_when_tools_specified() {
    let flags = ToolFlags {
        tools: Some(vec!["read".into(), "glob".into()]),
        no_tools: false,
        no_builtin_tools: false,
    };
    assert_eq!(
        resolve_tool_selection(flags),
        ToolSelection::Allowlist(vec!["read".into(), "glob".into()])
    );
}

#[test]
fn no_tools_takes_precedence_over_tools() {
    let flags = ToolFlags {
        tools: Some(vec!["read".into()]),
        no_tools: true,
        no_builtin_tools: false,
    };
    assert_eq!(resolve_tool_selection(flags), ToolSelection::Disabled);
}

#[test]
fn no_tools_takes_precedence_over_no_builtin() {
    let flags = ToolFlags {
        tools: None,
        no_tools: true,
        no_builtin_tools: true,
    };
    assert_eq!(resolve_tool_selection(flags), ToolSelection::Disabled);
}

#[test]
fn tools_takes_precedence_over_no_builtin() {
    let flags = ToolFlags {
        tools: Some(vec!["read".into()]),
        no_tools: false,
        no_builtin_tools: true,
    };
    assert_eq!(
        resolve_tool_selection(flags),
        ToolSelection::Allowlist(vec!["read".into()])
    );
}

// --- Tool name filtering ---

#[test]
fn filter_default_includes_all() {
    let all = vec!["read", "write", "edit", "bash", "glob", "grep"];
    let result = filter_tool_names(&all, &ToolSelection::Default);
    assert_eq!(result, all);
}

#[test]
fn filter_disabled_excludes_all() {
    let all = vec!["read", "write", "edit", "bash", "glob", "grep"];
    let result = filter_tool_names(&all, &ToolSelection::Disabled);
    assert!(result.is_empty(), "Disabled should filter out all tools");
}

#[test]
fn filter_no_builtin_excludes_all_phase3() {
    // Phase 3: NoBuiltin behaves like Disabled since no extension tools exist yet
    let all = vec!["read", "write", "edit", "bash", "glob", "grep"];
    let result = filter_tool_names(&all, &ToolSelection::NoBuiltin);
    assert!(result.is_empty(), "NoBuiltin should exclude built-in tools");
}

#[test]
fn filter_allowlist_keeps_only_named() {
    let all = vec!["read", "write", "edit", "bash", "glob", "grep"];
    let result = filter_tool_names(
        &all,
        &ToolSelection::Allowlist(vec!["read".into(), "glob".into()]),
    );
    assert_eq!(result, vec!["read", "glob"]);
}

#[test]
fn filter_allowlist_unknown_names_excluded() {
    let all = vec!["read", "write"];
    let result = filter_tool_names(
        &all,
        &ToolSelection::Allowlist(vec!["read".into(), "nonexistent".into()]),
    );
    assert_eq!(result, vec!["read"]);
}

#[test]
fn filter_allowlist_preserves_order() {
    let all = vec!["read", "write", "edit", "bash", "glob", "grep"];
    let result = filter_tool_names(
        &all,
        &ToolSelection::Allowlist(vec!["grep".into(), "read".into()]),
    );
    // Result preserves the order from all_tools, not from the allowlist
    assert_eq!(result, vec!["read", "grep"]);
}

#[test]
fn filter_allowlist_empty_excludes_all() {
    let all = vec!["read", "write"];
    let result = filter_tool_names(&all, &ToolSelection::Allowlist(vec![]));
    assert!(result.is_empty());
}

// --- CLI flag parsing ---

#[test]
fn cli_parse_tools_flag() {
    let cli = Cli::try_parse_from(["opi", "--tools", "read,glob"]).unwrap();
    assert_eq!(cli.tools, Some(vec!["read".into(), "glob".into()]));
}

#[test]
fn cli_parse_no_tools_flag() {
    let cli = Cli::try_parse_from(["opi", "--no-tools"]).unwrap();
    assert!(cli.no_tools);
}

#[test]
fn cli_parse_no_builtin_tools_flag() {
    let cli = Cli::try_parse_from(["opi", "--no-builtin-tools"]).unwrap();
    assert!(cli.no_builtin_tools);
}

#[test]
fn cli_parse_tools_single_tool() {
    let cli = Cli::try_parse_from(["opi", "--tools", "read"]).unwrap();
    assert_eq!(cli.tools, Some(vec!["read".into()]));
}

#[test]
fn cli_parse_no_flags_defaults() {
    let cli = Cli::try_parse_from(["opi"]).unwrap();
    assert!(cli.tools.is_none());
    assert!(!cli.no_tools);
    assert!(!cli.no_builtin_tools);
}

// --- Harness tool filtering (integration) ---

#[tokio::test]
async fn harness_default_includes_all_tools() {
    let workspace = create_temp_workspace();
    let mock = MockProvider::new("mock", vec![text_response("done")]);

    let harness = CodingHarness::new(
        Box::new(mock),
        "mock:mock-model".into(),
        OpiConfig::default(),
        workspace.path().to_path_buf(),
    );

    let system = harness.system_prompt();
    for tool in &["read", "write", "edit", "bash", "glob", "grep"] {
        assert!(
            system.contains(&format!("- {tool}:")),
            "Default selection should include tool '{tool}' in system prompt"
        );
    }
}

#[tokio::test]
async fn harness_disabled_removes_all_tools() {
    let workspace = create_temp_workspace();
    let mock = MockProvider::new("mock", vec![text_response("done")]);

    let harness = CodingHarness::new_with_selection(
        Box::new(mock),
        "mock:mock-model".into(),
        OpiConfig::default(),
        workspace.path().to_path_buf(),
        ToolSelection::Disabled,
    );

    let system = harness.system_prompt();
    assert!(
        !system.contains("Available tools:"),
        "Disabled selection should not include any tools"
    );
}

#[tokio::test]
async fn harness_no_builtin_removes_all_tools_phase3() {
    let workspace = create_temp_workspace();
    let mock = MockProvider::new("mock", vec![text_response("done")]);

    let harness = CodingHarness::new_with_selection(
        Box::new(mock),
        "mock:mock-model".into(),
        OpiConfig::default(),
        workspace.path().to_path_buf(),
        ToolSelection::NoBuiltin,
    );

    let system = harness.system_prompt();
    assert!(
        !system.contains("Available tools:"),
        "NoBuiltin should not include built-in tools in system prompt"
    );
}

#[tokio::test]
async fn harness_allowlist_filters_tools() {
    let workspace = create_temp_workspace();
    let mock = MockProvider::new("mock", vec![text_response("done")]);

    let harness = CodingHarness::new_with_selection(
        Box::new(mock),
        "mock:mock-model".into(),
        OpiConfig::default(),
        workspace.path().to_path_buf(),
        ToolSelection::Allowlist(vec!["read".into(), "glob".into()]),
    );

    let system = harness.system_prompt();
    assert!(
        system.contains("- read:"),
        "Allowlist should include 'read'"
    );
    assert!(
        system.contains("- glob:"),
        "Allowlist should include 'glob'"
    );
    assert!(
        !system.contains("- write:"),
        "Allowlist should exclude 'write'"
    );
    assert!(
        !system.contains("- edit:"),
        "Allowlist should exclude 'edit'"
    );
    assert!(
        !system.contains("- bash:"),
        "Allowlist should exclude 'bash'"
    );
    assert!(
        !system.contains("- grep:"),
        "Allowlist should exclude 'grep'"
    );
}
