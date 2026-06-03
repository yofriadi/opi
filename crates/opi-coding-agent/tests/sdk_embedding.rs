//! SDK embedding surface integration tests (task 4.2).
//!
//! Tests that opi-coding-agent re-uses SDK types from opi-agent::sdk
//! without duplicating protocol logic. Verifies type compatibility
//! between RPC mode and the SDK surface.

use opi_agent::sdk::{SDK_SCHEMA_VERSION, SdkCommand, SdkResponse};
use opi_coding_agent::rpc::{RPC_SCHEMA_VERSION, RpcCommand};

// ---------------------------------------------------------------------------
// Type compatibility: rpc.rs uses sdk types
// ---------------------------------------------------------------------------

#[test]
fn rpc_command_is_sdk_command() {
    // Verify that RpcCommand is a re-export of SdkCommand
    // by parsing the same JSON through both.
    let json = r#"{"type":"prompt","message":"hello","id":"1"}"#;

    let sdk_cmd: SdkCommand = serde_json::from_str(json).unwrap();
    let rpc_cmd: RpcCommand = serde_json::from_str(json).unwrap();

    assert_eq!(sdk_cmd.id(), rpc_cmd.id());
    assert_eq!(sdk_cmd.command_name(), rpc_cmd.command_name());
}

#[test]
fn rpc_schema_version_matches_sdk() {
    assert_eq!(
        SDK_SCHEMA_VERSION, RPC_SCHEMA_VERSION,
        "RPC and SDK must share the same schema version"
    );
}

#[test]
fn sdk_response_compatible_with_rpc_format() {
    // Build an SDK response and verify it matches the RPC wire format
    let resp = SdkResponse::success(Some("1"), "prompt");
    let val = serde_json::to_value(&resp).unwrap();
    assert_eq!(val["type"], "response");
    assert_eq!(val["success"], true);
    assert_eq!(val["id"], "1");
    assert_eq!(val["command"], "prompt");
}

#[test]
fn sdk_error_response_compatible_with_rpc_format() {
    let resp = SdkResponse::error(None, "parse", "invalid json");
    let val = serde_json::to_value(&resp).unwrap();
    assert_eq!(val["type"], "response");
    assert_eq!(val["success"], false);
    assert_eq!(val["error"], "invalid json");
}

#[test]
fn all_rpc_commands_parse_as_sdk() {
    let commands = vec![
        r#"{"type":"prompt","message":"hi"}"#,
        r#"{"type":"continue","message":"more"}"#,
        r#"{"type":"steer","message":"s"}"#,
        r#"{"type":"follow_up","message":"f"}"#,
        r#"{"type":"abort"}"#,
        r#"{"type":"set_model","model":"m"}"#,
        r#"{"type":"set_thinking_level","level":"low"}"#,
        r#"{"type":"compact"}"#,
        r#"{"type":"session_info"}"#,
        r#"{"type":"quit"}"#,
    ];
    for json in commands {
        let sdk: SdkCommand = serde_json::from_str(json).unwrap();
        let rpc: RpcCommand = serde_json::from_str(json).unwrap();
        assert_eq!(sdk.command_name(), rpc.command_name());
    }
}
