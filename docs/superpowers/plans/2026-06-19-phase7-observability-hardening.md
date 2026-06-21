# Phase 7 Observability Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the confirmed Phase 7 reliability and observability audit gaps so real runtime paths use typed diagnostics, safe redaction, counted diagnostics, and reachable local traces.

**Architecture:** Keep the shared diagnostic vocabulary in `opi-agent`, then route app-owned startup, package, adapter, session, and RPC observations through that vocabulary. Public diagnostic surfaces must serialize a redacted, owned diagnostic payload; raw local strings may stay inside internal errors but must not cross JSON/RPC/doctor/system-prompt boundaries unredacted.

**Tech Stack:** Rust 2024, existing `opi-agent`, `opi-ai`, and `opi-coding-agent` crates, tokio integration tests, existing NDJSON/RPC test harnesses.

## Global Constraints

- Do not commit unless the user explicitly asks; leave changes unstaged.
- Use workspace dependencies only; do not add crate-local dependency versions.
- After Rust changes, run `cargo clippy --workspace --all-targets -- -D warnings`.
- If a localized doc has an English counterpart, update both in the same task.
- Treat `TRACE_SCHEMA_VERSION` as unchanged unless the trace record shape changes.
- Bump `NDJSON_SCHEMA_VERSION` and `SDK_SCHEMA_VERSION` when changing startup diagnostic wire shape.
- Existing conversation event streams remain unredacted by design; this plan hardens diagnostics and trace surfaces, not user prompt echo.

---

## Verified Findings

Confirmed and in scope:

- Real hyphenated provider-key forms named by the audit do not match the current `sk-ant-[a-zA-Z0-9]{20,}` or `sk-[a-zA-Z0-9]{20,}` patterns.
- `Diagnostic::redacted_details()` intentionally does not redact `message` or `action`; provider and package paths put dynamic bodies/paths into `message`.
- Startup/resource/adapter diagnostics are `Vec<String>` and flow verbatim into NDJSON, RPC ready/session_info, and the system prompt.
- `ToolResult { is_error: true }` returned from `Ok(...)` is traced as `ToolCallCompleted` and emits no diagnostic.
- `CrashRecovery::diagnostics()` exists but production resume drops it.
- Successful compaction diagnostics are traced only, not recorded in the `RecordingSink`; manual compaction emits neither trace nor diagnostic.
- Top-level `opi --rpc` constructs an RPC runner without a trace sink, so the RPC `trace` command is unsupported on the real binary path.
- RPC trace snapshots accumulate across runs because the recording trace sink is not cleared per run.

Confirmed but not first-pass blockers:

- `SOURCE_ADAPTER` is currently only a constant/test value until typed adapter diagnostics are added.
- Bedrock doctor probes only `AWS_ACCESS_KEY_ID`, even though runtime credential resolution also accepts config/profile sources.
- Early provider/cancel exits can leave `TurnStarted` without `TurnEnded`; Task 6 documents this as current trace semantics for failed or cancelled turns.
- `let _ = turn_idx;` in `agent_loop.rs` is dead cleanup.

## File Structure

- Modify: `crates/opi-agent/src/streaming_proxy.rs`
  - Broaden secret value patterns for hyphenated provider key forms.
- Modify: `crates/opi-agent/src/diagnostic.rs`
  - Add a redacted owned diagnostic payload type, redacted text helper, extra content-sensitive keys, adapter codes, and provider diagnostic messages that keep raw bodies out of `message`.
- Modify: `crates/opi-agent/src/session_event.rs`
  - Change startup diagnostic event payload from raw strings to redacted diagnostic payloads.
- Modify: `crates/opi-agent/src/trace.rs`
  - Add `RecordingTraceSink::clear()`.
- Modify: `crates/opi-agent/src/agent_loop.rs`
  - Emit diagnostics and failed trace records for `Ok(ToolResult { is_error: true })`; remove dead `let _ = turn_idx;`.
- Modify: `crates/opi-agent/tests/diagnostics.rs`
  - Add real-format key redaction and diagnostic payload redaction tests.
- Modify: `crates/opi-agent/tests/streaming_proxy.rs`
  - Add direct `SecretRedactor` tests for hyphenated key values.
- Modify: `crates/opi-agent/tests/diagnostics_runtime.rs`
  - Add provider-body diagnostic and tool error-result emission tests.
- Modify: `crates/opi-agent/tests/trace_envelope.rs`
  - Add tool error-result trace classification and recording sink clear tests.
- Modify: `crates/opi-agent/src/sdk.rs`
  - Bump `SDK_SCHEMA_VERSION` if RPC diagnostic wire shape changes.
- Modify: `crates/opi-agent/README.md`
- Modify: `crates/opi-agent/README.zh.md`
  - Update SDK schema version documentation if bumped.
- Modify: `crates/opi-coding-agent/src/runner.rs`
  - Bump `NDJSON_SCHEMA_VERSION`; emit typed startup diagnostics.
- Modify: `crates/opi-coding-agent/src/runtime_packages.rs`
  - Return typed startup diagnostics.
- Modify: `crates/opi-coding-agent/src/adapter_extension.rs`
  - Return typed adapter diagnostics with `SOURCE_ADAPTER`.
- Modify: `crates/opi-coding-agent/src/harness.rs`
  - Store typed resource diagnostics, expose redacted payloads, avoid raw details in system prompt, record compaction diagnostics into sink and trace.
- Modify: `crates/opi-coding-agent/src/rpc.rs`
  - Enable top-level-compatible recording trace sink behavior, emit typed startup diagnostics, clear trace per run.
- Modify: `crates/opi-coding-agent/src/main.rs`
  - Carry resume diagnostics into run startup diagnostics and enable RPC trace sink on the production path.
- Modify: `crates/opi-coding-agent/src/session_cli.rs`
  - Preserve `CrashRecovery::diagnostics()` in `ResumedSession` and print redacted warnings.
- Modify: `crates/opi-coding-agent/src/diagnostic_bridge.rs`
  - Move dynamic package messages into details and use static public messages.
- Modify: `crates/opi-coding-agent/src/doctor.rs`
  - Serialize/print redacted diagnostic payloads, including message/action.
- Modify: `crates/opi-coding-agent/tests/json_mode.rs`
  - Update schema/version expectations and add startup diagnostic redaction tests.
- Modify: `crates/opi-coding-agent/tests/rpc_jsonl.rs`
  - Update schema/version expectations; add top-level-equivalent RPC trace, per-run trace, and startup diagnostic redaction tests.
- Modify: `crates/opi-coding-agent/tests/harness_resource_integration.rs`
  - Update adapter diagnostic expectations from strings to typed diagnostics.
- Modify: `crates/opi-coding-agent/tests/session_cli.rs`
  - Add corrupt and truncated recovery diagnostic tests.
- Modify: `crates/opi-coding-agent/tests/doctor_cli.rs`
  - Add message/action redaction and package message tests.
- Modify: `crates/opi-coding-agent/tests/observability_docs.rs`
  - Update guards for typed startup diagnostics and RPC trace production semantics.
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `crates/opi-coding-agent/README.md`
- Modify: `crates/opi-coding-agent/README.zh.md`
  - Synchronize local/explicit observability, schema version, startup diagnostic, and RPC trace semantics.

## Task 1: Redaction Core and Diagnostic Payload

**Files:**
- Modify: `crates/opi-agent/src/streaming_proxy.rs`
- Modify: `crates/opi-agent/src/diagnostic.rs`
- Modify: `crates/opi-agent/tests/diagnostics.rs`
- Modify: `crates/opi-agent/tests/streaming_proxy.rs`
- Modify: `crates/opi-agent/tests/diagnostics_runtime.rs`

**Interfaces:**
- Produces: `DiagnosticPayload`, `Diagnostic::redacted_payload(RedactionMode) -> DiagnosticPayload`, `redact_text(&str, RedactionMode) -> String`.
- Produces: adapter diagnostic code constants used by Task 2.
- Produces: provider diagnostics with static public `message` and raw provider text moved to redacted details.

- [ ] **Step 1: Add failing key-format redaction tests**

Add this test to `crates/opi-agent/tests/streaming_proxy.rs`:

```rust
#[test]
fn redactor_redacts_hyphenated_provider_key_values() {
    let redactor = SecretRedactor::default();
    let samples = [
        "sk-ant-api03-1234567890abcdefghijklmnopqrstuv",
        "sk-ant-api06-1234567890abcdefghijklmnopqrstuv",
        "sk-proj-1234567890abcdefghijklmnopqrstuv",
        "sk-live-1234567890abcdefghijklmnopqrstuv",
        "sk-svcacct-1234567890abcdefghijklmnopqrstuv",
    ];

    for sample in samples {
        let event = serde_json::json!({ "message": format!("credential {sample}") });
        let redacted = redactor.redact(&event);
        let text = redacted["message"].as_str().unwrap();
        assert!(
            !text.contains(sample),
            "hyphenated provider key leaked: {text}"
        );
        assert_eq!(text, "[REDACTED]");
    }
}
```

Add this test to `crates/opi-agent/tests/diagnostics.rs`:

```rust
#[test]
fn diagnostic_redacts_hyphenated_provider_keys_in_details() {
    let details = serde_json::json!({
        "anthropic": "sk-ant-api03-1234567890abcdefghijklmnopqrstuv",
        "openai": "sk-proj-1234567890abcdefghijklmnopqrstuv",
        "ordinary": "kept"
    });

    let redacted = redact(&details, RedactionMode::Summary);

    assert_eq!(redacted["anthropic"], "[REDACTED]");
    assert_eq!(redacted["openai"], "[REDACTED]");
    assert_eq!(redacted["ordinary"], "kept");
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```sh
cargo test -p opi-agent --test streaming_proxy redactor_redacts_hyphenated_provider_key_values
cargo test -p opi-agent --test diagnostics diagnostic_redacts_hyphenated_provider_keys_in_details
```

Expected: both tests fail because current value patterns reject internal hyphens.

- [ ] **Step 3: Broaden provider key patterns**

Change the default value patterns in `crates/opi-agent/src/streaming_proxy.rs`:

```rust
// Anthropic API keys, including sk-ant-api03-* and sk-ant-api06-* forms.
r"sk-ant-[a-zA-Z0-9-]{20,}".to_owned(),
// OpenAI-style API keys, including sk-proj-*, sk-live-*, and sk-svcacct-* forms.
r"sk-[a-zA-Z0-9-]{20,}".to_owned(),
```

- [ ] **Step 4: Add failing public diagnostic payload tests**

Add this test to `crates/opi-agent/tests/diagnostics.rs`:

```rust
#[test]
fn redacted_payload_scrubs_message_action_and_details() {
    let diag = Diagnostic::new(
        Severity::Warning,
        "provider_request_failed",
        "provider",
        "HTTP 500: token sk-proj-1234567890abcdefghijklmnopqrstuv at /Users/alice/body",
    )
    .action("inspect /Users/alice/.config/opi/config.toml")
    .details(serde_json::json!({
        "provider_error": "raw body with sk-ant-api03-1234567890abcdefghijklmnopqrstuv",
        "ordinary": "kept"
    }));

    let payload = diag.redacted_payload(RedactionMode::Summary);

    assert_eq!(payload.message, "[REDACTED]");
    assert_eq!(payload.action.as_deref(), Some("[REDACTED]"));
    assert_eq!(payload.details.as_ref().unwrap()["provider_error"], "[REDACTED]");
    assert_eq!(payload.details.as_ref().unwrap()["ordinary"], "kept");
}
```

Add this provider mapping test to `crates/opi-agent/tests/diagnostics_runtime.rs`:

```rust
#[test]
fn provider_error_diagnostic_uses_static_message_and_redacted_body_details() {
    let err = opi_ai::ProviderError::RequestFailed(
        "HTTP 500: body carried sk-proj-1234567890abcdefghijklmnopqrstuv".into(),
    );
    let diag = Diagnostic::from(&err);

    assert_eq!(diag.message, "provider request failed");
    assert_eq!(diag.details.as_ref().unwrap()["provider_error"].as_str().unwrap(),
        "HTTP 500: body carried sk-proj-1234567890abcdefghijklmnopqrstuv");

    let payload = diag.redacted_payload(RedactionMode::Summary);
    assert_eq!(payload.message, "provider request failed");
    assert_eq!(payload.details.as_ref().unwrap()["provider_error"], "[REDACTED]");
}
```

- [ ] **Step 5: Run tests and verify RED**

Run:

```sh
cargo test -p opi-agent --test diagnostics redacted_payload_scrubs_message_action_and_details
cargo test -p opi-agent --test diagnostics_runtime provider_error_diagnostic_uses_static_message_and_redacted_body_details
```

Expected: compile/test failures because `DiagnosticPayload`, `redacted_payload`, and provider-body details do not exist yet.

- [ ] **Step 6: Implement owned redacted diagnostic payload**

Add this type and helper methods to `crates/opi-agent/src/diagnostic.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticPayload {
    pub severity: Severity,
    pub code: String,
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

impl Diagnostic {
    pub fn redacted_payload(&self, mode: RedactionMode) -> DiagnosticPayload {
        DiagnosticPayload {
            severity: self.severity,
            code: self.code.to_owned(),
            source: self.source.to_owned(),
            message: redact_text(&self.message, mode),
            details: self.redacted_details(mode),
            action: self.action.as_ref().map(|action| redact_text(action, mode)),
        }
    }
}

pub fn redact_text(text: &str, mode: RedactionMode) -> String {
    let value = serde_json::Value::String(text.to_owned());
    match redact(&value, mode) {
        serde_json::Value::String(redacted) => redacted,
        _ => REDACTED.to_owned(),
    }
}
```

Extend `CONTENT_SENSITIVE_KEYS`:

```rust
const CONTENT_SENSITIVE_KEYS: &[&str] = &[
    "prompt",
    "prompts",
    "tool_output",
    "tool_result",
    "env",
    "environment",
    "command",
    "args",
    "cwd",
    "body",
    "request_body",
    "response_body",
    "provider_error",
    "headers",
    "stdout",
    "stderr",
];
```

Add adapter/startup codes:

```rust
pub const CODE_PACKAGE_RESOLUTION_FAILED: &str = "package_resolution_failed";
pub const CODE_ADAPTER_PROTOCOL_UNSUPPORTED: &str = "adapter_protocol_unsupported";
pub const CODE_ADAPTER_KIND_UNSUPPORTED: &str = "adapter_kind_unsupported";
pub const CODE_ADAPTER_COMMAND_INVALID: &str = "adapter_command_invalid";
pub const CODE_ADAPTER_STARTUP_FAILED: &str = "adapter_startup_failed";
pub const CODE_ADAPTER_REGISTRATION_FAILED: &str = "adapter_registration_failed";
pub const CODE_ADAPTER_HOST_DIAGNOSTIC: &str = "adapter_host_diagnostic";
```

- [ ] **Step 7: Move provider raw messages into details**

Change `impl From<&opi_ai::provider::ProviderError> for Diagnostic` in `crates/opi-agent/src/diagnostic.rs`:

```rust
ProviderError::RequestFailed(message) => Diagnostic::new(
    Severity::Error,
    code::CODE_PROVIDER_REQUEST_FAILED,
    SOURCE_PROVIDER,
    "provider request failed",
)
.details(serde_json::json!({ "provider_error": message })),
ProviderError::StreamError(message) => Diagnostic::new(
    Severity::Error,
    code::CODE_PROVIDER_STREAM_ERROR,
    SOURCE_PROVIDER,
    "provider stream failed",
)
.details(serde_json::json!({ "provider_error": message })),
ProviderError::AuthFailed(message) => Diagnostic::new(
    Severity::Error,
    code::CODE_PROVIDER_AUTH_FAILED,
    SOURCE_PROVIDER,
    "provider authentication failed",
)
.details(serde_json::json!({ "provider_error": message }))
.action(ACTION_CHECK_CREDENTIALS),
```

Change `AgentError::Provider`, `AgentError::AuthFailed`, `AgentError::Tool`, `AgentError::Hook`, and `AgentError::TraceSetup` mappings to use static messages plus details:

```rust
AgentError::Provider(message) => Diagnostic::new(
    Severity::Error,
    code::CODE_PROVIDER_ERROR,
    SOURCE_PROVIDER,
    "provider error",
)
.details(serde_json::json!({ "provider_error": message })),
AgentError::AuthFailed(message) => Diagnostic::new(
    Severity::Error,
    code::CODE_PROVIDER_AUTH_FAILED,
    SOURCE_PROVIDER,
    "provider authentication failed",
)
.details(serde_json::json!({ "provider_error": message }))
.action(ACTION_CHECK_CREDENTIALS),
AgentError::Tool(message) => Diagnostic::new(
    Severity::Error,
    code::CODE_TOOL_FAILED,
    SOURCE_TOOL,
    "tool failed",
)
.details(serde_json::json!({ "tool_error": message })),
AgentError::Hook(message) => Diagnostic::new(
    Severity::Error,
    code::CODE_HOOK_FAILED,
    SOURCE_AGENT,
    "hook failed",
)
.details(serde_json::json!({ "hook_error": message })),
AgentError::TraceSetup(message) => Diagnostic::new(
    Severity::Error,
    code::CODE_TRACE_SETUP_FAILED,
    SOURCE_AGENT,
    "trace setup failed",
)
.details(serde_json::json!({ "trace_error": message }))
.action("check the trace path is writable and its parent directory exists"),
```

Add `tool_error`, `hook_error`, and `trace_error` to `CONTENT_SENSITIVE_KEYS`.

- [ ] **Step 8: Run focused tests and verify GREEN**

Run:

```sh
cargo test -p opi-agent --test streaming_proxy redactor_redacts_hyphenated_provider_key_values
cargo test -p opi-agent --test diagnostics diagnostic_redacts_hyphenated_provider_keys_in_details
cargo test -p opi-agent --test diagnostics redacted_payload_scrubs_message_action_and_details
cargo test -p opi-agent --test diagnostics_runtime provider_error_diagnostic_uses_static_message_and_redacted_body_details
```

Expected: all pass.

## Task 2: Typed Startup, Package, Adapter, and Resource Diagnostics

**Files:**
- Modify: `crates/opi-agent/src/session_event.rs`
- Modify: `crates/opi-agent/src/sdk.rs`
- Modify: `crates/opi-agent/README.md`
- Modify: `crates/opi-agent/README.zh.md`
- Modify: `crates/opi-coding-agent/src/runner.rs`
- Modify: `crates/opi-coding-agent/src/runtime_packages.rs`
- Modify: `crates/opi-coding-agent/src/adapter_extension.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/src/rpc.rs`
- Modify: `crates/opi-coding-agent/src/diagnostic_bridge.rs`
- Modify: `crates/opi-coding-agent/src/doctor.rs`
- Modify: `crates/opi-coding-agent/tests/json_mode.rs`
- Modify: `crates/opi-coding-agent/tests/rpc_jsonl.rs`
- Modify: `crates/opi-coding-agent/tests/harness_resource_integration.rs`
- Modify: `crates/opi-coding-agent/tests/doctor_cli.rs`

**Interfaces:**
- Consumes: `DiagnosticPayload`, `Diagnostic::redacted_payload`.
- Produces: typed startup diagnostics on NDJSON, RPC ready, RPC session_info resources, and resource system-prompt summaries.
- Produces: `NDJSON_SCHEMA_VERSION = 2`, `SDK_SCHEMA_VERSION = 3`.

- [ ] **Step 1: Add failing doctor message redaction test**

Add this test to `crates/opi-coding-agent/tests/doctor_cli.rs`:

```rust
#[test]
fn doctor_json_redacts_diagnostic_message_and_action() {
    let report = DoctorReport {
        entries: vec![DoctorEntry {
            scope: DoctorScope::Package,
            diagnostic: Diagnostic::new(
                Severity::Warning,
                "package_diagnostic",
                "package",
                "read C:\\Users\\alice\\.config\\opi\\packages\\p\\package.toml: sk-proj-1234567890abcdefghijklmnopqrstuv",
            )
            .action("open C:\\Users\\alice\\.config\\opi\\config.toml"),
        }],
    };

    let json = format_json(&report);

    assert!(!json.contains("alice"), "{json}");
    assert!(!json.contains("sk-proj-1234567890abcdefghijklmnopqrstuv"), "{json}");
    assert!(json.contains("\"message\":\"[REDACTED]\""), "{json}");
    assert!(json.contains("\"action\":\"[REDACTED]\""), "{json}");
}
```

- [ ] **Step 2: Add failing startup diagnostics redaction tests**

Add this JSON-mode test to `crates/opi-coding-agent/tests/json_mode.rs`:

```rust
#[tokio::test]
async fn startup_diagnostics_are_typed_and_redacted() {
    let provider = MockProvider::new("mock", vec![test_support::text_response("done")]);
    let diagnostic = Diagnostic::new(
        Severity::Warning,
        "adapter_startup_failed",
        "adapter",
        "adapter failed to start",
    )
    .details(serde_json::json!({
        "adapter_command": "C:\\Users\\alice\\bin\\adapter.exe",
        "package_source": "https://alice:s3cr3t@example.com/o/r.git"
    }));
    let mut runner = runner_with_startup(Box::new(provider), vec![diagnostic], None);

    let result = runner.run_json("hi").await;
    assert_eq!(result.exit_code, ExitCode::Success as i32);

    let lines: Vec<serde_json::Value> = result
        .stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines[0]["schema_version"], NDJSON_SCHEMA_VERSION);
    assert_eq!(lines[1]["type"], "StartupDiagnostics");
    assert_eq!(lines[1]["diagnostics"][0]["source"], "adapter");
    assert_eq!(lines[1]["diagnostics"][0]["code"], "adapter_startup_failed");
    assert_eq!(lines[1]["diagnostics"][0]["details"]["adapter_command"], "[REDACTED]");

    let serialized = serde_json::to_string(&lines[1]).unwrap();
    assert!(!serialized.contains("alice"), "{serialized}");
    assert!(!serialized.contains("s3cr3t"), "{serialized}");
}
```

Add this RPC test to `crates/opi-coding-agent/tests/rpc_jsonl.rs`:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_startup_diagnostics_are_typed_and_redacted() {
    let diagnostic = Diagnostic::new(
        Severity::Warning,
        "adapter_command_invalid",
        "adapter",
        "adapter command invalid",
    )
    .details(serde_json::json!({
        "adapter_command": "/Users/alice/bin/adapter --token ghp_01234567890123456789012345678901234567"
    }));
    let mut runner = rpc_runner_with_startup(vec![diagnostic]);

    let (command_tx, command_rx) = unbounded_channel();
    let (output_tx, mut output_rx) = unbounded_channel();
    let task = tokio::spawn(async move { runner.run_with_channels(command_rx, output_tx).await });

    let ready = recv_rpc_line(&mut output_rx).await;
    let serialized = serde_json::to_string(&ready).unwrap();
    assert_eq!(ready["type"], "rpc_ready");
    assert_eq!(ready["startup_diagnostics"][0]["source"], "adapter");
    assert_eq!(ready["startup_diagnostics"][0]["details"]["adapter_command"], "[REDACTED]");
    assert!(!serialized.contains("alice"), "{serialized}");
    assert!(!serialized.contains("ghp_"), "{serialized}");

    command_tx.send(RpcCommand::quit { id: None }).unwrap();
    let _ = task.await;
}
```

- [ ] **Step 3: Run tests and verify RED**

Run:

```sh
cargo test -p opi-coding-agent --test doctor_cli doctor_json_redacts_diagnostic_message_and_action
cargo test -p opi-coding-agent --test json_mode startup_diagnostics_are_typed_and_redacted
cargo test -p opi-coding-agent --test rpc_jsonl rpc_startup_diagnostics_are_typed_and_redacted
```

Expected: compile/test failures because public surfaces still use `Vec<String>` and doctor output does not redact message/action.

- [ ] **Step 4: Change startup event and schema versions**

In `crates/opi-agent/src/session_event.rs`:

```rust
use crate::diagnostic::DiagnosticPayload;

StartupDiagnostics {
    diagnostics: Vec<DiagnosticPayload>,
},
```

In `crates/opi-coding-agent/src/runner.rs`:

```rust
pub const NDJSON_SCHEMA_VERSION: u32 = 2;
```

In `crates/opi-agent/src/sdk.rs`:

```rust
pub const SDK_SCHEMA_VERSION: u32 = 3;
```

Update tests that assert exact schema versions to use the constants or the new values.

- [ ] **Step 5: Convert runtime package diagnostics to `Vec<Diagnostic>`**

In `crates/opi-coding-agent/src/runtime_packages.rs`, change:

```rust
use opi_agent::diagnostic::{Diagnostic, SOURCE_PACKAGE, Severity, code::*};

pub struct RuntimePackageStartup {
    pub extension_registry: ExtensionRegistry,
    pub installed_packages: Vec<PackageResource>,
    pub diagnostics: Vec<Diagnostic>,
}
```

For package resolution failure:

```rust
diagnostics.push(
    Diagnostic::new(
        Severity::Warning,
        CODE_PACKAGE_RESOLUTION_FAILED,
        SOURCE_PACKAGE,
        "installed package resolution failed",
    )
    .details(serde_json::json!({ "package_error": e.to_string() })),
);
```

For resolver diagnostics:

```rust
diagnostics.extend(
    resolution
        .diagnostics
        .iter()
        .map(crate::diagnostic_bridge::diagnostic_from_package),
);
```

Add `package_error` to `CONTENT_SENSITIVE_KEYS`.

- [ ] **Step 6: Convert adapter diagnostics to `Vec<Diagnostic>`**

In `crates/opi-coding-agent/src/adapter_extension.rs`, change the function signature:

```rust
pub async fn start_adapters_from_packages(
    packages: &[crate::package_discovery::PackageResource],
    working_dir: &Path,
    mut registry: ExtensionRegistry,
) -> (ExtensionRegistry, Vec<Diagnostic>)
```

Add helpers:

```rust
fn adapter_diagnostic(
    code: &'static str,
    message: &'static str,
    package_name: &str,
    details: serde_json::Value,
) -> Diagnostic {
    Diagnostic::new(Severity::Warning, code, SOURCE_ADAPTER, message)
        .details(serde_json::json!({
            "package_name": package_name,
            "adapter": details,
        }))
}
```

Use static messages:

```rust
adapter_diagnostic(
    CODE_ADAPTER_PROTOCOL_UNSUPPORTED,
    "unsupported adapter protocol",
    &package.manifest.name,
    serde_json::json!({
        "protocol": adapter.protocol,
        "expected_protocol": "opi-extension-jsonl-v1",
        "adapter_command": adapter.command,
    }),
)
```

Use analogous diagnostics for unsupported kind, command invalid, host diagnostic, registration failed, and startup failed. Put exception strings under keys such as `adapter_error`; add `adapter_error` to `CONTENT_SENSITIVE_KEYS`.

- [ ] **Step 7: Store typed diagnostics in harness metadata**

In `crates/opi-coding-agent/src/harness.rs`, change:

```rust
pub struct DiscoveredResourceMetadata {
    pub extensions: Vec<ResourceMetadataEntry>,
    pub packages: Vec<ResourceMetadataEntry>,
    pub skills: Vec<ResourceMetadataEntry>,
    pub fragments: Vec<ResourceMetadataEntry>,
    pub themes: Vec<ResourceMetadataEntry>,
    pub diagnostics: Vec<Diagnostic>,
}
```

Add helpers:

```rust
fn diagnostic_payloads(diagnostics: &[Diagnostic]) -> Vec<opi_agent::diagnostic::DiagnosticPayload> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.redacted_payload(RedactionMode::Summary))
        .collect()
}

fn diagnostic_prompt_line(diagnostic: &Diagnostic) -> String {
    let payload = diagnostic.redacted_payload(RedactionMode::Summary);
    format!(
        "{} {}::{}: {}",
        payload.severity, payload.source, payload.code, payload.message
    )
}
```

Change `format_for_system_prompt()` so diagnostics include only `diagnostic_prompt_line(diagnostic)`, never details.

Change `to_rpc_json()`:

```rust
"diagnostics": diagnostic_payloads(&self.diagnostics),
```

- [ ] **Step 8: Emit typed startup diagnostics in runner and RPC**

In `crates/opi-coding-agent/src/runner.rs`:

```rust
let startup = AgentSessionEvent::StartupDiagnostics {
    diagnostics: self
        .harness
        .resource_metadata()
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.redacted_payload(RedactionMode::Summary))
        .collect(),
};
```

In `crates/opi-coding-agent/src/rpc.rs`:

```rust
let startup_diagnostics = self
    .harness
    .as_ref()
    .map(|harness| {
        harness
            .resource_metadata()
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.redacted_payload(RedactionMode::Summary))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
```

- [ ] **Step 9: Redact doctor text and JSON through payloads**

In `crates/opi-coding-agent/src/doctor.rs`, make `format_json()` serialize a local output shape with `DiagnosticPayload`:

```rust
#[derive(serde::Serialize)]
struct DoctorEntryPayload<'a> {
    scope: &'a DoctorScope,
    diagnostic: opi_agent::diagnostic::DiagnosticPayload,
}
```

Use:

```rust
let entry = DoctorEntryPayload {
    scope: &entry.scope,
    diagnostic: entry.diagnostic.redacted_payload(RedactionMode::Summary),
};
```

In `format_text()`, render from the redacted payload:

```rust
let payload = d.redacted_payload(RedactionMode::Summary);
out.push_str(&format!(
    "[{}] {}: {}::{}: {}\n",
    payload.severity,
    entry.scope.as_str(),
    payload.source,
    payload.code,
    payload.message,
));
if let Some(action) = &payload.action {
    out.push_str(&format!("    action: {action}\n"));
}
```

- [ ] **Step 10: Move package dynamic messages into details**

In `crates/opi-coding-agent/src/diagnostic_bridge.rs`, change `diagnostic_from_package()`:

```rust
Diagnostic::new(
    severity,
    CODE_PACKAGE_DIAGNOSTIC,
    SOURCE_PACKAGE,
    "package diagnostic",
)
.details(serde_json::json!({
    "package_code": pd.code,
    "package_source": pd.source,
    "package_message": pd.message,
    "scope": scope,
}))
```

Add `package_message` to `CONTENT_SENSITIVE_KEYS`.

- [ ] **Step 11: Update test helpers from strings to diagnostics**

Update `runner_with_startup`, `rpc_runner_with_startup`, `runtime_startup`, and adapter tests to accept `Vec<Diagnostic>`. String assertions become field assertions:

```rust
assert_eq!(diagnostics[0].source, SOURCE_ADAPTER);
assert_eq!(diagnostics[0].code, CODE_ADAPTER_STARTUP_FAILED);
assert_eq!(diagnostics[0].message, "adapter startup failed");
```

- [ ] **Step 12: Run focused tests and verify GREEN**

Run:

```sh
cargo test -p opi-coding-agent --test doctor_cli doctor_json_redacts_diagnostic_message_and_action
cargo test -p opi-coding-agent --test json_mode startup_diagnostics_are_typed_and_redacted
cargo test -p opi-coding-agent --test rpc_jsonl rpc_startup_diagnostics_are_typed_and_redacted
cargo test -p opi-coding-agent --test harness_resource_integration adapter
```

Expected: all pass.

## Task 3: Tool Error-Result Diagnostics and Trace Classification

**Files:**
- Modify: `crates/opi-agent/src/agent_loop.rs`
- Modify: `crates/opi-agent/tests/diagnostics_runtime.rs`
- Modify: `crates/opi-agent/tests/trace_envelope.rs`

**Interfaces:**
- Consumes: existing `ToolResult.is_error`.
- Produces: `CODE_TOOL_EXECUTION_FAILED` diagnostic and `TraceKind::ToolCallFailed` when a tool returns `Ok(ToolResult { is_error: true })`.

- [ ] **Step 1: Add failing diagnostic test**

Add a mock tool in `crates/opi-agent/tests/diagnostics_runtime.rs`:

```rust
struct ErrorResultTool;

impl Tool for ErrorResultTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "error_result".into(),
            description: "returns an error ToolResult".into(),
            input_schema: serde_json::json!({ "type": "object", "additionalProperties": false }),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        _arguments: serde_json::Value,
        _signal: tokio_util::sync::CancellationToken,
        _on_update: Option<UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        Box::pin(async {
            Ok(ToolResult {
                content: vec![opi_ai::message::OutputContent::Text {
                    text: "tool-level failure".into(),
                }],
                details: None,
                is_error: true,
                terminate: false,
            })
        })
    }
}
```

Add a test that drives `agent_loop` with a mock provider calling `error_result` and asserts `CODE_TOOL_EXECUTION_FAILED` is in the `RecordingSink`.

- [ ] **Step 2: Add failing trace test**

Add a trace test in `crates/opi-agent/tests/trace_envelope.rs` that drives the same tool and asserts:

```rust
assert!(kinds.contains(&TraceKind::ToolCallStarted));
assert!(kinds.contains(&TraceKind::ToolCallFailed));
assert!(!kinds.contains(&TraceKind::ToolCallCompleted));
assert!(
    trace_sink.snapshot().iter().any(|r| {
        r.kind == TraceKind::DiagnosticLinked
            && r.diagnostic_code == Some(CODE_TOOL_EXECUTION_FAILED)
    })
);
```

- [ ] **Step 3: Run tests and verify RED**

Run:

```sh
cargo test -p opi-agent --test diagnostics_runtime tool_error_result
cargo test -p opi-agent --test trace_envelope tool_error_result
```

Expected: tests fail because the current path traces completed and emits no diagnostic.

- [ ] **Step 4: Implement the final-result check**

Change the `Ok(result)` branch in `execute_tool()`:

```rust
let final_result = match hooks.after_tool_call(ctx).await {
    AfterToolCallResult::Keep => result,
    AfterToolCallResult::Replace(replacement) => replacement,
};
if final_result.is_error {
    observe(
        sink,
        trace,
        tool_diagnostic(
            CODE_TOOL_EXECUTION_FAILED,
            tool_name,
            "tool returned an error result",
        ),
    );
    trace_tool(trace, TraceKind::ToolCallFailed, tool_name, turn_id);
} else {
    trace_tool(trace, TraceKind::ToolCallCompleted, tool_name, turn_id);
}
final_result
```

Remove the stale comment saying `is_error` still counts as completed.

- [ ] **Step 5: Remove dead `turn_idx` no-op**

Delete this line from `agent_loop.rs`:

```rust
let _ = turn_idx;
```

- [ ] **Step 6: Run focused tests and verify GREEN**

Run:

```sh
cargo test -p opi-agent --test diagnostics_runtime tool_error_result
cargo test -p opi-agent --test trace_envelope tool_error_result
cargo test -p opi-agent --test trace_envelope phase7_tool_call_failed_for_unknown_tool
```

Expected: all pass.

## Task 4: Session Recovery and Compaction Diagnostics in Production Sinks

**Files:**
- Modify: `crates/opi-coding-agent/src/session_cli.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Modify: `crates/opi-coding-agent/src/runner.rs`
- Modify: `crates/opi-coding-agent/src/harness.rs`
- Modify: `crates/opi-coding-agent/tests/session_cli.rs`
- Modify: `crates/opi-coding-agent/tests/json_mode.rs`
- Modify: `crates/opi-coding-agent/tests/rpc_jsonl.rs`

**Interfaces:**
- Produces: `ResumedSession.diagnostics: Vec<Diagnostic>`.
- Produces: a harness helper that records one diagnostic into both the recording sink and trace collector.
- Consumes: `CompactionOutput::diagnostic()`.

- [ ] **Step 1: Add failing session recovery tests**

In `crates/opi-coding-agent/tests/session_cli.rs`, extend corrupt/truncated resume tests:

```rust
assert_eq!(result.diagnostics.len(), 1);
assert_eq!(result.diagnostics[0].source, "session");
assert_eq!(result.diagnostics[0].code, "session_truncated_line");
```

For corrupt entries:

```rust
assert_eq!(result.diagnostics[0].code, "session_corrupt_entries");
assert_eq!(
    result.diagnostics[0].details.as_ref().unwrap()["corrupt_count"],
    1
);
```

- [ ] **Step 2: Add failing JSON startup recovery test**

Add a JSON-mode resume test that builds a session file with a truncated final line, runs `opi --json --resume <id> "hi"` through the existing subprocess helper, and asserts the `StartupDiagnostics` line contains a `session_truncated_line` diagnostic.

- [ ] **Step 3: Implement `ResumedSession.diagnostics`**

In `session_cli.rs`:

```rust
pub struct ResumedSession {
    pub header: opi_agent::session::SessionHeader,
    pub entries: Vec<opi_agent::session::SessionEntry>,
    pub path: PathBuf,
    pub skipped_entries: usize,
    pub diagnostics: Vec<opi_agent::Diagnostic>,
}
```

In `resume_session()`:

```rust
let skipped_entries = recovery.corrupt_count();
let diagnostics = recovery.diagnostics();
```

In `fork_session()`, set `diagnostics: Vec::new()`.

In `handle_session_cli()`, print all redacted recovery diagnostics:

```rust
for diagnostic in &session.diagnostics {
    let payload = diagnostic.redacted_payload(opi_agent::RedactionMode::Summary);
    eprintln!(
        "opi: warning: {}::{}: {}",
        payload.source, payload.code, payload.message
    );
}
```

- [ ] **Step 4: Carry resume diagnostics into startup diagnostics**

In `main.rs`, keep a third value from the session CLI branch:

```rust
let (resumed_messages, resume_info, resume_diagnostics) = ...
```

Pass `resume_diagnostics` into `run_non_interactive`, `run_rpc`, and `run_interactive`. Extend `runtime_startup.diagnostics` before building the runner/harness:

```rust
runtime_startup.diagnostics.extend(resume_diagnostics);
```

For RPC, pass `resume_info` or at least `resume_diagnostics` into `run_rpc`; the current `run_rpc` signature drops resume metadata.

- [ ] **Step 5: Add failing compaction count tests**

In `crates/opi-coding-agent/tests/json_mode.rs`, add an automatic compaction test using a low threshold. Assert the final `session_summary.diagnostics.info` is at least 1 and the trace, when enabled, includes `session_compacted`.

In `crates/opi-coding-agent/tests/rpc_jsonl.rs`, add a manual compact command test:

```rust
command_tx.send(RpcCommand::compact {
    id: Some("compact-1".into()),
}).unwrap();
let resp = recv_response(&mut output_rx, "compact").await;
assert_eq!(resp["success"], true);
```

Then run a summary-producing prompt and assert diagnostic counts include the compaction info if the manual compact had output. If there is no compactable state, assert `compaction_nothing_to_compact` is recorded as info.

- [ ] **Step 6: Implement harness diagnostic recording helper**

In `harness.rs`, add:

```rust
fn record_harness_diagnostic(&self, diagnostic: Diagnostic) {
    if let Some(sink) = &self.diagnostics {
        sink.record(diagnostic.clone());
    }
    self.trace_diagnostic(&diagnostic);
}
```

Replace the automatic compaction hand-built diagnostic with:

```rust
self.record_harness_diagnostic(out.diagnostic());
```

In `compact()`, after `execute_compaction(reason)`:

```rust
match &result {
    Some(out) => self.record_harness_diagnostic(out.diagnostic()),
    None => self.record_harness_diagnostic(Diagnostic::from(
        &opi_agent::compaction::CompactionError::NothingToCompact,
    )),
}
```

Only emit `NothingToCompact` when `execute_compaction` was actually requested and returned `Ok(None)`.

- [ ] **Step 7: Run focused tests and verify GREEN**

Run:

```sh
cargo test -p opi-coding-agent --test session_cli recovery
cargo test -p opi-coding-agent --test json_mode session_recovery
cargo test -p opi-coding-agent --test json_mode compaction
cargo test -p opi-coding-agent --test rpc_jsonl compact
```

Expected: all pass.

## Task 5: Production RPC Trace and Per-Run Trace Semantics

**Files:**
- Modify: `crates/opi-agent/src/trace.rs`
- Modify: `crates/opi-coding-agent/src/main.rs`
- Modify: `crates/opi-coding-agent/src/rpc.rs`
- Modify: `crates/opi-coding-agent/tests/rpc_jsonl.rs`

**Interfaces:**
- Produces: top-level `opi --rpc` sessions with an in-memory redacted trace sink.
- Produces: RPC `trace` command returning only the most recent run's records.
- Keeps: no automatic trace file persistence.

- [ ] **Step 1: Add failing subprocess RPC trace test**

In `crates/opi-coding-agent/tests/rpc_jsonl.rs`, add a subprocess test using the existing `OpiProcess` helper:

```rust
#[test]
fn rpc_subprocess_trace_command_returns_envelope_after_prompt() {
    let mut proc = OpiProcess::spawn_with_args(&["--rpc", "--model", "mock:mock-model"]);
    let ready = proc.read_json_line();
    assert_eq!(ready["type"], "rpc_ready");

    proc.send(&serde_json::json!({
        "type": "prompt",
        "id": "p1",
        "message": "hello"
    }));
    let _ = proc.read_until_response("prompt");
    let _ = proc.read_until_agent_end();

    proc.send(&serde_json::json!({
        "type": "trace",
        "id": "t1"
    }));
    let resp = proc.read_until_response("trace");

    assert_eq!(resp["success"], true, "{resp}");
    assert_eq!(resp["data"]["schema_version"], opi_agent::TRACE_SCHEMA_VERSION);
    assert!(resp["data"]["records"].as_array().unwrap().len() > 0);

    proc.send(&serde_json::json!({ "type": "quit" }));
}
```

Use the same mock-provider setup pattern already used by RPC subprocess tests. If the helper does not expose `read_until_agent_end()`, add it by reusing the existing line-drain logic from in-process tests.

- [ ] **Step 2: Add failing per-run trace test**

In the existing in-process RPC phase7 module, run two prompts with a shared `RecordingTraceSink`, then call `trace`. Assert all returned records share one run id and that it is the second run id:

```rust
let run_ids: BTreeSet<_> = records
    .iter()
    .filter_map(|record| record["run_id"].as_str())
    .collect();
assert_eq!(run_ids.len(), 1, "trace response must be per-run: {records:?}");
```

- [ ] **Step 3: Run tests and verify RED**

Run:

```sh
cargo test -p opi-coding-agent --test rpc_jsonl rpc_subprocess_trace_command_returns_envelope_after_prompt
cargo test -p opi-coding-agent --test rpc_jsonl rpc_trace_response_is_latest_run_only
```

Expected: subprocess trace returns `unsupported_trace_request`; per-run test returns multiple run ids.

- [ ] **Step 4: Add `RecordingTraceSink::clear()`**

In `crates/opi-agent/src/trace.rs`:

```rust
impl RecordingTraceSink {
    pub fn clear(&self) {
        match self.records.lock() {
            Ok(mut guard) => guard.clear(),
            Err(poisoned) => poisoned.into_inner().clear(),
        }
    }
}
```

Add a unit test in `trace_envelope.rs` proving `clear()` removes prior records and accepts new records after clearing.

- [ ] **Step 5: Clear RPC trace sink at run start**

In `CodingHarness::prepare_trace_run()`, clear the trace sink before constructing a collector when the configured sink is a recording sink. The smallest change is to add an optional clear callback to `TraceConfig`:

```rust
pub struct TraceConfig {
    pub sink: Arc<dyn TraceSink>,
    pub mode: RedactionMode,
    pub clear_before_run: Option<Arc<dyn Fn() + Send + Sync>>,
}
```

When `clear_before_run` is present:

```rust
if let Some(clear) = &config.clear_before_run {
    clear();
}
```

For file traces, set `clear_before_run: None`.

For RPC recording traces, set:

```rust
let recording = trace_sink.clone();
clear_before_run: Some(Arc::new(move || recording.clear())),
```

- [ ] **Step 6: Enable production RPC recording trace sink**

In `main.rs` `run_rpc`, create an `Arc<RecordingTraceSink>` and pass it into `RpcRunner::new_with_runtime_packages` through a new constructor parameter:

```rust
let trace_sink = std::sync::Arc::new(opi_agent::RecordingTraceSink::new());
RpcRunner::new_with_runtime_packages(
    provider,
    config.defaults.model.clone(),
    config.clone(),
    workspace_root,
    allow_mutating,
    tool_selection,
    user_system_prompt,
    resumed_messages.unwrap_or_default(),
    runtime_startup,
    Some(trace_sink),
)
```

Update `RpcRunner::new_with_runtime_packages` to accept `trace_sink: Option<Arc<RecordingTraceSink>>` and pass it to `new_with_optional_extension_registry`.

This keeps trace local and in-memory. The trace envelope is emitted only when the client sends the explicit RPC `trace` command.

- [ ] **Step 7: Run focused tests and verify GREEN**

Run:

```sh
cargo test -p opi-agent --test trace_envelope recording_trace_sink_clear
cargo test -p opi-coding-agent --test rpc_jsonl rpc_subprocess_trace_command_returns_envelope_after_prompt
cargo test -p opi-coding-agent --test rpc_jsonl rpc_trace_response_is_latest_run_only
cargo test -p opi-coding-agent --test rpc_jsonl phase7_trace_request_supported_and_unsupported_paths
```

Expected: all pass. The unsupported-path test should still pass for embedded callers that explicitly construct a runner with `None`.

## Task 6: Documentation, Guards, and Low-Risk Cleanup

**Files:**
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `README.md`
- Modify: `README.zh.md`
- Modify: `crates/opi-coding-agent/README.md`
- Modify: `crates/opi-coding-agent/README.zh.md`
- Modify: `crates/opi-agent/README.md`
- Modify: `crates/opi-agent/README.zh.md`
- Modify: `crates/opi-coding-agent/tests/observability_docs.rs`
- Modify: `crates/opi-coding-agent/src/doctor.rs`
- Modify: `crates/opi-coding-agent/tests/doctor_cli.rs`

**Interfaces:**
- Consumes: schema version and behavior changes from Tasks 1-5.
- Produces: docs that no longer overclaim raw startup diagnostics, RPC trace reachability, or schema versions.

- [ ] **Step 1: Update observability docs**

In `docs/opi-spec.md` and `docs/opi-spec.zh.md`, state:

- startup diagnostics are structured diagnostic payloads redacted at public boundaries;
- RPC sessions keep the latest run's local in-memory redacted trace until the next run or quit;
- the RPC `trace` command emits that latest-run envelope explicitly;
- no trace file is persisted unless non-interactive `--trace PATH` is used.

Remove the stale Phase 7 design wording that mentions a trace config setting if editing the design spec is accepted for current-state docs. If the design spec is treated as a historical artifact, leave it unchanged and add a note in the plan execution summary.

- [ ] **Step 2: Update schema-version docs**

Update:

```text
SDK_SCHEMA_VERSION is `3`
NDJSON_SCHEMA_VERSION is `2`
```

in the affected READMEs and tests. Keep `TRACE_SCHEMA_VERSION = 1`.

- [ ] **Step 3: Add docs guard assertions**

In `observability_docs.rs`, assert the English and Chinese docs contain:

- `structured startup diagnostics`
- `latest run`
- `RPC trace command`
- `in-memory`
- `unstable 0.x`

Keep the existing non-goal guards.

- [ ] **Step 4: Fix Bedrock doctor credential presence**

Add a `doctor_cli.rs` test where selected model is `bedrock:<model>` and config has:

```toml
[providers.bedrock]
access_key_id = "CONFIG_AKID"
secret_access_key_env = "BEDROCK_SECRET"
```

Set `BEDROCK_SECRET` in the test environment and assert provider credentials are reported present without requiring `AWS_ACCESS_KEY_ID`.

Implement a Bedrock-specific helper in `doctor.rs` that reports credentials present when:

- `providers.bedrock.access_key_id` is set and the configured/default secret env var is present;
- or `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` are present;
- or `providers.bedrock.profile` is set.

Do not read shared AWS credentials files in doctor; presence-only config/profile checks are enough for a network-free health signal.

- [ ] **Step 5: Document open-turn trace semantics**

Do not change trace behavior in this task. Add one targeted test documenting the current behavior:

```rust
#[tokio::test]
async fn provider_failure_trace_may_leave_turn_open() {
    // Existing behavior: provider failure records TurnStarted, ProviderFailure,
    // DiagnosticLinked, and RunEnded, without forcing TurnEnded.
}
```

Update `docs/opi-spec.md` / `.zh.md` to say trace consumers must tolerate a `TurnStarted` without `TurnEnded` when a run exits mid-turn due to cancellation or provider failure.

- [ ] **Step 6: Run docs and low-risk tests**

Run:

```sh
cargo test -p opi-coding-agent --test observability_docs
cargo test -p opi-coding-agent --test doctor_cli bedrock
cargo test -p opi-agent --test trace_envelope provider_failure_trace_may_leave_turn_open
```

Expected: all pass.

## Task 7: Final Verification

**Files:**
- No new files beyond tasks above.

- [ ] **Step 1: Run focused Phase 7 suites**

Run:

```sh
cargo test -p opi-agent --test diagnostics
cargo test -p opi-agent --test diagnostics_runtime
cargo test -p opi-agent --test trace_envelope
cargo test -p opi-coding-agent --test diagnostics_runtime
cargo test -p opi-coding-agent --test doctor_cli
cargo test -p opi-coding-agent --test json_mode
cargo test -p opi-coding-agent --test rpc_jsonl
cargo test -p opi-coding-agent --test observability_docs
cargo test -p opi-coding-agent --test harness_resource_integration
cargo test -p opi-coding-agent --test session_cli
```

Expected: all pass.

- [ ] **Step 2: Run required Rust gate**

Run:

```sh
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both exit 0.

- [ ] **Step 3: Run workspace tests**

Run:

```sh
cargo test --workspace --all-targets --no-fail-fast
```

Expected: all real test binaries pass. If `adapter_host_mock` behaves as a harness-less subprocess binary on this platform, document the exact binary result separately instead of calling the gate clean.

- [ ] **Step 4: Run docs gate**

Run:

```sh
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Expected: exit 0.

- [ ] **Step 5: Final audit summary**

Produce a short summary that lists:

- fixed audit IDs or finding names;
- schema version changes;
- tests run with pass/fail result;
- any deliberately documented residuals, especially open-turn trace semantics.

Do not stage or commit unless the user explicitly asks.

## Self-Review

Spec coverage:

- SC1 is covered by `DiagnosticPayload` and typed startup diagnostics.
- SC3 is covered by typed startup diagnostics and existing run-summary counts.
- SC4 is covered by production RPC trace and latest-run semantics.
- SC5 is covered by tool error results, adapter diagnostics, session recovery, and compaction sink wiring.
- SC6 is covered by real-format keys, message/action redaction, provider-body details, and startup diagnostics redaction tests.
- SC7 is covered by EN/ZH documentation updates and guards.
- SC8 remains protected by existing non-goal guards.

Placeholder scan:

- This plan contains no deferred implementation placeholders in the core P1/P2 tasks.
- Open-turn trace semantics are documented as current intended behavior instead of being changed in this remediation.

Type consistency:

- Internal diagnostics use `Diagnostic`.
- Public serialized diagnostics use `DiagnosticPayload`.
- Startup events use `Vec<DiagnosticPayload>`.
- Resource metadata stores `Vec<Diagnostic>` and serializes through `redacted_payload`.
- RPC trace uses `RecordingTraceSink` with per-run clearing.
