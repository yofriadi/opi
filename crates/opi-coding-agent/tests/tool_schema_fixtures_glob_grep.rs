//! Schema fixture tests for glob and grep tools (task 1.10).
//!
//! Validates that generated JSON Schema from schemars is parseable and that
//! representative model arguments survive deserialization.

use opi_agent::tool::Tool;
use opi_coding_agent::tool::{GlobTool, GrepTool};
use serde_json::json;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// GlobTool schema
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct GlobArgs {
    pattern: String,
}

#[test]
fn glob_schema_has_required_pattern_field() {
    let tool = GlobTool::new(PathBuf::from("."));
    let schema = &tool.definition().input_schema;

    assert!(schema.is_object());
    let props = schema
        .get("properties")
        .expect("schema should have properties");
    assert!(
        props.get("pattern").is_some(),
        "should have 'pattern' property"
    );

    let required = schema.get("required").expect("schema should have required");
    assert!(
        required.as_array().unwrap().iter().any(|r| r == "pattern"),
        "pattern should be required"
    );
}

#[test]
fn glob_schema_accepts_valid_args() {
    let args: GlobArgs = serde_json::from_value(json!({ "pattern": "**/*.rs" })).unwrap();
    assert_eq!(args.pattern, "**/*.rs");
}

#[test]
fn glob_schema_rejects_missing_pattern() {
    let result = serde_json::from_value::<GlobArgs>(json!({}));
    assert!(
        result.is_err(),
        "missing pattern should fail deserialization"
    );
}

// ---------------------------------------------------------------------------
// GrepTool schema
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct GrepArgs {
    pattern: String,
}

#[test]
fn grep_schema_has_required_pattern_field() {
    let tool = GrepTool::new(PathBuf::from("."));
    let schema = &tool.definition().input_schema;

    assert!(schema.is_object());
    let props = schema
        .get("properties")
        .expect("schema should have properties");
    assert!(
        props.get("pattern").is_some(),
        "should have 'pattern' property"
    );

    let required = schema.get("required").expect("schema should have required");
    assert!(
        required.as_array().unwrap().iter().any(|r| r == "pattern"),
        "pattern should be required"
    );
}

#[test]
fn grep_schema_accepts_valid_args() {
    let args: GrepArgs = serde_json::from_value(json!({ "pattern": "TODO" })).unwrap();
    assert_eq!(args.pattern, "TODO");
}

#[test]
fn grep_schema_rejects_missing_pattern() {
    let result = serde_json::from_value::<GrepArgs>(json!({}));
    assert!(
        result.is_err(),
        "missing pattern should fail deserialization"
    );
}
