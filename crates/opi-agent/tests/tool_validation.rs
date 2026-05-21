//! Behavioral tests for tool trait and schema validation (task 1.5).
//!
//! DoD: "invalid args become error tool result"

use std::future::Future;
use std::pin::Pin;

use opi_agent::ToolDef;
use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_agent::validation::{self, ValidationError};
use serde_json::json;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Test tool implementations
// ---------------------------------------------------------------------------

/// A tool with a schema requiring a `name` string property.
struct GreetTool;

impl Tool for GreetTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "greet".into(),
            description: "Greet someone by name.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"]
            }),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        arguments: serde_json::Value,
        _signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        let name = arguments
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("world")
            .to_owned();
        Box::pin(async move {
            Ok(ToolResult {
                content: vec![opi_ai::message::OutputContent::Text {
                    text: format!("Hello, {name}!"),
                }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }
}

/// A tool with an empty object schema (accepts anything).
struct EchoTool;

impl Tool for EchoTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "echo".into(),
            description: "Echoes input.".into(),
            input_schema: json!({ "type": "object" }),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        _arguments: serde_json::Value,
        _signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        Box::pin(async move {
            Ok(ToolResult {
                content: vec![opi_ai::message::OutputContent::Text {
                    text: "echo".into(),
                }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sequential
    }
}

// ---------------------------------------------------------------------------
// Schema validation
// ---------------------------------------------------------------------------

#[test]
fn valid_args_pass_validation() {
    let schema = json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"]
    });
    let args = json!({ "name": "Alice" });
    assert!(validation::validate(&schema, &args).is_ok());
}

#[test]
fn missing_required_field_fails_validation() {
    let schema = json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"]
    });
    let args = json!({});
    let result = validation::validate(&schema, &args);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.errors.is_empty());
}

#[test]
fn wrong_type_fails_validation() {
    let schema = json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"]
    });
    let args = json!({ "name": 123 });
    let result = validation::validate(&schema, &args);
    assert!(result.is_err());
}

#[test]
fn extra_properties_allowed_by_default() {
    let schema = json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"]
    });
    let args = json!({ "name": "Alice", "extra": true });
    assert!(validation::validate(&schema, &args).is_ok());
}

#[test]
fn empty_schema_accepts_any_object() {
    let schema = json!({ "type": "object" });
    let args = json!({ "anything": "goes" });
    assert!(validation::validate(&schema, &args).is_ok());
}

#[test]
fn empty_object_passes_empty_schema() {
    let schema = json!({ "type": "object" });
    let args = json!({});
    assert!(validation::validate(&schema, &args).is_ok());
}

// ---------------------------------------------------------------------------
// Validation → error ToolResult
// ---------------------------------------------------------------------------

#[test]
fn validation_error_produces_error_tool_result() {
    let err = ValidationError {
        errors: vec!["'name' is required".into()],
    };
    let result = ToolResult::from_validation_error(err);
    assert!(result.is_error);
    assert!(!result.terminate);
    let text = result.content.iter().find_map(|c| match c {
        opi_ai::message::OutputContent::Text { text } => Some(text.as_str()),
        _ => None,
    });
    assert!(text.is_some());
    assert!(text.unwrap().contains("'name' is required"));
}

// ---------------------------------------------------------------------------
// Tool definition
// ---------------------------------------------------------------------------

#[test]
fn tool_definition_returns_correct_schema() {
    let tool = GreetTool;
    let def = tool.definition();
    assert_eq!(def.name, "greet");
    assert_eq!(def.description, "Greet someone by name.");
    assert_eq!(def.input_schema["type"], "object");
    assert!(def.input_schema["required"].is_array());
}

// ---------------------------------------------------------------------------
// ExecutionMode
// ---------------------------------------------------------------------------

#[test]
fn default_execution_mode_is_parallel() {
    let tool = GreetTool;
    assert_eq!(tool.execution_mode(), ExecutionMode::Parallel);
}

#[test]
fn tool_can_override_execution_mode() {
    let tool = EchoTool;
    assert_eq!(tool.execution_mode(), ExecutionMode::Sequential);
}

// ---------------------------------------------------------------------------
// Full flow: validate then execute
// ---------------------------------------------------------------------------

#[tokio::test]
async fn valid_args_execute_successfully() {
    let tool = GreetTool;
    let args = json!({ "name": "World" });
    let schema = &tool.definition().input_schema;

    validation::validate(schema, &args).unwrap();

    let result = tool
        .execute("call-1", args, CancellationToken::new(), None)
        .await
        .unwrap();
    assert!(!result.is_error);
    let text = result.content.iter().find_map(|c| match c {
        opi_ai::message::OutputContent::Text { text } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(text.unwrap(), "Hello, World!");
}

#[tokio::test]
async fn invalid_args_become_error_tool_result() {
    let tool = GreetTool;
    let args = json!({});
    let schema = &tool.definition().input_schema;

    let validation_result = validation::validate(schema, &args);
    assert!(validation_result.is_err());

    // Agent loop would convert validation error to error ToolResult
    let result = ToolResult::from_validation_error(validation_result.unwrap_err());
    assert!(result.is_error);
    assert!(!result.terminate);
}
