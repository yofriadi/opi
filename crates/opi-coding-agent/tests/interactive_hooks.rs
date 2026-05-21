//! Unit tests for InteractiveCodingHooks (C3/H8).
//!
//! Directly exercises the before_tool_call hook to verify the safety policy:
//! mutating tools (write, edit, bash) are blocked unless allow_mutating=true;
//! read-only tools (read, glob, grep) are always allowed.

use opi_agent::hooks::{AgentHooks, BeforeToolCallContext, BeforeToolCallResult};
use opi_coding_agent::harness::InteractiveCodingHooks;

fn make_ctx(tool_name: &str) -> BeforeToolCallContext {
    BeforeToolCallContext {
        tool_call_id: "tc-1".into(),
        tool_name: tool_name.into(),
        args: serde_json::json!({}),
        messages: vec![],
    }
}

async fn assert_allowed(hooks: &InteractiveCodingHooks, tool_name: &str) {
    let result = hooks.before_tool_call(make_ctx(tool_name)).await;
    assert!(
        matches!(result, BeforeToolCallResult::Allow),
        "{tool_name} should be allowed"
    );
}

async fn assert_denied(hooks: &InteractiveCodingHooks, tool_name: &str) {
    let result = hooks.before_tool_call(make_ctx(tool_name)).await;
    match result {
        BeforeToolCallResult::Deny { reason } => {
            assert!(
                reason.contains(tool_name),
                "denial reason should mention tool: {reason}"
            );
            assert!(
                reason.contains("interactive mode"),
                "denial reason should mention interactive mode: {reason}"
            );
        }
        BeforeToolCallResult::Allow => {
            panic!("{tool_name} should be denied, but was allowed")
        }
        _ => {
            panic!("{tool_name} should be denied, got unexpected result")
        }
    }
}

#[tokio::test]
async fn interactive_write_blocked_by_default() {
    let hooks = InteractiveCodingHooks::new(false);
    assert_denied(&hooks, "write").await;
}

#[tokio::test]
async fn interactive_edit_blocked_by_default() {
    let hooks = InteractiveCodingHooks::new(false);
    assert_denied(&hooks, "edit").await;
}

#[tokio::test]
async fn interactive_bash_blocked_by_default() {
    let hooks = InteractiveCodingHooks::new(false);
    assert_denied(&hooks, "bash").await;
}

#[tokio::test]
async fn interactive_read_allowed_by_default() {
    let hooks = InteractiveCodingHooks::new(false);
    assert_allowed(&hooks, "read").await;
}

#[tokio::test]
async fn interactive_glob_allowed_by_default() {
    let hooks = InteractiveCodingHooks::new(false);
    assert_allowed(&hooks, "glob").await;
}

#[tokio::test]
async fn interactive_grep_allowed_by_default() {
    let hooks = InteractiveCodingHooks::new(false);
    assert_allowed(&hooks, "grep").await;
}

#[tokio::test]
async fn interactive_all_mutating_tools_allowed_when_opted_in() {
    let hooks = InteractiveCodingHooks::new(true);
    assert_allowed(&hooks, "write").await;
    assert_allowed(&hooks, "edit").await;
    assert_allowed(&hooks, "bash").await;
    assert_allowed(&hooks, "read").await;
}

#[tokio::test]
async fn interactive_unknown_tool_allowed_by_default() {
    let hooks = InteractiveCodingHooks::new(false);
    assert_allowed(&hooks, "custom_search").await;
}
