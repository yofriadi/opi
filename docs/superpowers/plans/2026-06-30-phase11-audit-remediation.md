# Phase 11 Audit Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the confirmed Phase 11 audit defects without broad refactors or unrequested product expansion.

**Architecture:** Treat the audit findings as four repair fronts: public redaction boundaries, bash temp/process reliability, filesystem/navigation contract consistency, and documentation/spec/test honesty. Preserve the existing `ToolResult`/`ToolDiagnostic` substrate where possible; add narrow helper APIs at the event/session/public-output boundaries instead of spreading ad hoc scrubbing through individual tools.

**Tech Stack:** Rust 2024, Cargo workspace, `tokio`, `serde_json`, `clap`, existing `opi-agent` diagnostic redaction utilities, existing `opi-coding-agent` integration tests.

## Global Constraints

- Do not commit unless the user explicitly asks; replace every "commit" checkpoint with "review git diff and stop".
- Only stage files explicitly changed in this session if the user later asks for a commit.
- Keep changes surgical; do not add new permission prompts, sandboxing, background shell infrastructure, workflow tools, or provider families.
- After code changes, run `cargo clippy --workspace --all-targets -- -D warnings`.
- After modifying a test file, run that test by name and iterate until it passes.
- Documentation changes touching `README.md`, `README.zh.md`, `docs/opi-spec.md`, or `docs/opi-spec.zh.md` must keep localized counterparts synchronized.
- Preserve the current provider-facing rule: provider request bodies receive tool result `content`, not tool result `details`, `diagnostics`, or `truncated`.

---

## Verification Summary

Confirmed from source and targeted tests:

- Public event/session redaction gap is real. `agent_loop.rs` emits raw `ToolExecutionStart.args`, `ToolExecutionEnd.details`, and `ToolExecutionEnd.diagnostics`; `ToolResultMessage.details` is serialized and then persisted through `session_coordinator.rs`.
- Bash diagnostics include raw `command` in `diagnostics[].context.command`; bash details include raw `details.command`.
- Verbose trace redaction currently skips `CONTENT_SENSITIVE_KEYS`; summary trace scrubs them.
- `opi --help` only lists flags and does not document shell/cwd/timeout/64 KiB/full-output policy.
- `write` does not pre-classify an existing directory at the target path before temp-write plus rename.
- `read` normalizes CRLF to LF in output because it uses `str::lines()` and rejoins with `\n`.
- `read` has no byte budget and reads/scans the whole file before line truncation.
- `grep` silently skips metadata/read failures and non-UTF-8 content, and uses lossy path conversion.
- `grep`/`find`/`glob`/`ls` cap output after collecting and sorting, which caps payload size but not traversal work.
- `ls` omits `omitted_count` from details when truncated.
- `docs/opi-spec.md` and `docs/opi-spec.zh.md` are stale for `ToolResult.truncated`, `ToolDiagnostic`, `ToolResultMessage.truncated`, `ToolExecutionEnd.details/truncated/diagnostics`, and Phase 11 roadmap status.
- README edit description says "first exact match"; implementation requires a unique exact match.
- `TOOL_ERROR_MARKER` is duplicated in two OpenAI provider files.

Refuted or downgraded:

- "No edit CRLF test" is refuted. `edit_crlf_preservation` and `edit_preserves_crlf_when_old_string_spans_boundary` both pass.
- "No nav cancellation tests" is refuted for current code. `grep`, `glob`, `find`, and `ls` pre-cancelled-token tests pass.
- `full_output` not being listed in `CONTENT_SENSITIVE_KEYS` is not itself a summary-trace leak because absolute-path redaction catches the value.
- Path-addressed failure `details: None` is an explicit current contract, not an implementation accident. The plan keeps diagnostics as the failure metadata channel and updates docs/tests to say that directly.

## File Structure

- Modify `crates/opi-agent/src/diagnostic.rs`: expose a public helper for public-safe structured values and make verbose trace redact structural sensitive keys.
- Modify `crates/opi-agent/src/event.rs`: add `AgentEvent::redacted_for_public()` or equivalent helpers for event args/details/diagnostics.
- Modify `crates/opi-agent/src/agent_loop.rs`: use public-safe event values for `ToolExecutionStart` and `ToolExecutionEnd`, and public-safe `ToolResultMessage.details` for persisted/session messages.
- Modify `crates/opi-agent/tests/truncated_propagation.rs` or add `crates/opi-agent/tests/tool_event_redaction.rs`: cover event/session redaction at the agent-loop boundary.
- Modify `crates/opi-coding-agent/tests/rpc_jsonl.rs`: cover RPC/NDJSON public redaction for bash command canaries.
- Modify `crates/opi-coding-agent/src/tool/bash.rs`: fix temp file permissions/name generation, kill-on-drop, wait-failed metadata, spill append behavior, and saturating output totals.
- Modify `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`: add bash regressions for command canaries, wait-failed metadata where injectable, restrictive full-output files on Unix, CRLF read behavior, write-to-directory classification, and bash reliability.
- Modify `crates/opi-coding-agent/src/tool/read.rs`: add line-ending metadata, bounded byte output, and bounded binary sampling.
- Modify `crates/opi-coding-agent/src/tool/write.rs`: classify existing directory targets before temp writes.
- Modify `crates/opi-coding-agent/src/tool/grep.rs`, `find.rs`, `glob.rs`, `ls.rs`, and `mod.rs`: add bounded traversal/result collection helpers and align skipped-file diagnostics/details.
- Modify `crates/opi-coding-agent/tests/tools_glob_grep.rs`, `find_tool.rs`, and `ls_tool.rs`: add nav bounded-work and grep skipped-file coverage.
- Modify `crates/opi-coding-agent/src/cli.rs`: add public help policy text.
- Modify `crates/opi-coding-agent/tests/non_interactive.rs` and `phase11_tooling_quality_docs.rs`: strengthen help/docs guard assertions.
- Modify `crates/opi-coding-agent/README.md`, `crates/opi-coding-agent/README.zh.md`, `docs/opi-spec.md`, `docs/opi-spec.zh.md`, and `CHANGELOG.md`: sync public contracts.
- Modify `crates/opi-ai/src/openai_chat.rs`, `openai_responses.rs`, and one shared module such as `crates/opi-ai/src/message.rs`: share `TOOL_ERROR_MARKER`.
- Modify `crates/opi-ai/tests/tool_result_wire.rs`: keep marker behavior pinned after the constant move.

### Task 1: Public Tool Event And Session Redaction

**Files:**
- Modify: `crates/opi-agent/src/diagnostic.rs`
- Modify: `crates/opi-agent/src/event.rs`
- Modify: `crates/opi-agent/src/agent_loop.rs`
- Test: `crates/opi-agent/tests/tool_event_redaction.rs`
- Test: `crates/opi-coding-agent/tests/rpc_jsonl.rs`

**Interfaces:**
- Consumes: `opi_agent::diagnostic::redact(value, RedactionMode::Summary)`.
- Produces: `pub fn redact_public_value(value: &serde_json::Value) -> serde_json::Value`.
- Produces: `impl AgentEvent { pub fn redacted_for_public(&self) -> Self }`.
- Produces: `fn public_tool_details(details: &Option<serde_json::Value>) -> Option<serde_json::Value>`.
- Produces: `fn public_tool_diagnostics(diagnostics: &[ToolDiagnostic]) -> Vec<ToolDiagnostic>`.

- [ ] **Step 1: Write failing agent-loop redaction tests**

Add `crates/opi-agent/tests/tool_event_redaction.rs` with tests that create a mock bash-like tool returning raw command details and diagnostics, then assert the public event and persisted `ToolResultMessage.details` do not contain a sentinel command secret.

```rust
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use opi_agent::diagnostic::code::CODE_TOOL_EXECUTION_FAILED;
use opi_agent::event::{AgentEvent, AgentEventSink};
use opi_agent::hooks::{
    AgentHooks, BeforeToolCallContext, BeforeToolCallResult, ShouldStopAfterTurnContext,
};
use opi_agent::loop_types::{AgentError, AgentLoopConfig, AgentLoopContext};
use opi_agent::message::AgentMessage;
use opi_agent::tool::{ExecutionMode, Tool, ToolDiagnostic, ToolError, ToolResult, result};
use opi_ai::message::{InputContent, Message, OutputContent, UserMessage};
use opi_ai::test_support::{self, MockProvider};
use serde_json::json;
use tokio_util::sync::CancellationToken;

struct SecretTool;

impl Tool for SecretTool {
    fn definition(&self) -> opi_ai::message::ToolDef {
        opi_ai::message::ToolDef {
            name: "bash".into(),
            description: "test tool".into(),
            input_schema: json!({"type":"object"}),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        _args: serde_json::Value,
        _signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<ToolResult, ToolError>> + Send>> {
        Box::pin(async {
            let mut out = result::ok(
                vec![OutputContent::Text { text: "failed".into() }],
                json!({
                    "command": "echo OPI_COMMAND_SECRET_CANARY",
                    "cwd": "C:\\Users\\private\\repo",
                    "exit_code": 1,
                    "timed_out": false,
                    "cancelled": false,
                    "truncated": false
                }),
            );
            out.is_error = true;
            out.diagnostics.push(ToolDiagnostic {
                code: CODE_TOOL_EXECUTION_FAILED.to_string(),
                message: "command exited non-zero".into(),
                context: json!({
                    "command": "echo OPI_COMMAND_SECRET_CANARY",
                    "exit_code": 1
                }),
            });
            Ok(out)
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sequential
    }
}

struct AllowHooks;

impl AgentHooks for AllowHooks {
    fn convert_to_llm(&self, messages: &[AgentMessage]) -> Result<Vec<Message>, AgentError> {
        Ok(messages
            .iter()
            .filter_map(|m| match m {
                AgentMessage::Llm(m) => Some(m.clone()),
                _ => None,
            })
            .collect())
    }

    fn should_stop_after_turn(
        &self,
        _: ShouldStopAfterTurnContext,
    ) -> Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        Box::pin(async { false })
    }

    fn before_tool_call(
        &self,
        _: BeforeToolCallContext,
    ) -> Pin<Box<dyn std::future::Future<Output = BeforeToolCallResult> + Send>> {
        Box::pin(async { BeforeToolCallResult::Allow })
    }
}

#[tokio::test]
async fn tool_event_and_session_details_redact_command_context() {
    let first = test_support::tool_call_response(
        "tc1",
        "bash",
        r#"{"command":"echo OPI_COMMAND_SECRET_CANARY"}"#,
    );
    let second = test_support::text_response("done");
    let provider = MockProvider::new("mock", vec![first, second]);

    let seen = Arc::new(Mutex::new(Vec::<AgentEvent>::new()));
    let seen_clone = seen.clone();
    let events: AgentEventSink = Box::new(move |event| {
        seen_clone.lock().unwrap().push(event);
    });

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools: vec![Box::new(SecretTool)],
        messages: vec![AgentMessage::Llm(Message::User(UserMessage {
            content: vec![InputContent::Text { text: "use bash".into() }],
            timestamp_ms: 0,
        }))],
        model: "mock-model".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: None,
        diagnostic_sink: None,
        trace: None,
    };
    let messages = opi_agent::agent_loop(
        context,
        AgentLoopConfig { max_turns: 3, ..Default::default() },
        &AllowHooks,
        events,
        CancellationToken::new(),
    )
    .await
    .expect("agent loop should finish");

    let rendered_events = serde_json::to_string(&*seen.lock().unwrap()).unwrap();
    assert!(!rendered_events.contains("OPI_COMMAND_SECRET_CANARY"), "{rendered_events}");

    let rendered_messages = serde_json::to_string(&messages).unwrap();
    assert!(!rendered_messages.contains("OPI_COMMAND_SECRET_CANARY"), "{rendered_messages}");

    assert!(messages.iter().any(|m| matches!(m, AgentMessage::Llm(Message::ToolResult(_)))));
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p opi-agent --test tool_event_redaction tool_event_and_session_details_redact_command_context`

Expected before implementation: FAIL because the rendered event or rendered messages contain `OPI_COMMAND_SECRET_CANARY`.

- [ ] **Step 3: Add public redaction helpers**

In `crates/opi-agent/src/diagnostic.rs`, add:

```rust
/// Redact a value for public event/session boundaries.
///
/// This is stricter than provider conversion and matches summary diagnostics:
/// tool arguments, paths, commands, env metadata, stdout/stderr-like fields, and
/// recognizable secret values are scrubbed before JSON/RPC/session exposure.
pub fn redact_public_value(value: &serde_json::Value) -> serde_json::Value {
    redact(value, RedactionMode::Summary)
}
```

In `crates/opi-agent/src/event.rs`, import `redact_public_value` and add:

```rust
impl AgentEvent {
    pub fn redacted_for_public(&self) -> Self {
        match self {
            AgentEvent::ToolExecutionStart { tool_call_id, tool_name, args } => {
                AgentEvent::ToolExecutionStart {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    args: redact_public_value(args),
                }
            }
            AgentEvent::ToolExecutionEnd {
                tool_call_id,
                tool_name,
                result,
                details,
                is_error,
                truncated,
                diagnostics,
            } => AgentEvent::ToolExecutionEnd {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                result: result.clone(),
                details: details.as_ref().map(redact_public_value),
                is_error: *is_error,
                truncated: *truncated,
                diagnostics: diagnostics
                    .iter()
                    .map(|d| crate::tool::ToolDiagnostic {
                        code: d.code.clone(),
                        message: d.message.clone(),
                        context: redact_public_value(&d.context),
                    })
                    .collect(),
            },
            other => other.clone(),
        }
    }
}
```

Use the correct crate-local path for `ToolDiagnostic` inside `event.rs` (`crate::tool::ToolDiagnostic`, not `opi_agent::tool::ToolDiagnostic`) when implementing.

- [ ] **Step 4: Apply helpers in the agent loop**

In `crates/opi-agent/src/agent_loop.rs`, replace `ToolExecutionStart` emissions with redacted args:

```rust
events(AgentEvent::ToolExecutionStart {
    tool_call_id: parsed.tool_call.id.clone(),
    tool_name: parsed.tool_call.name.clone(),
    args: crate::diagnostic::redact_public_value(&parsed.args_for_event),
});
```

Replace both `ToolExecutionEnd` blocks so `details` and `diagnostics` are public-safe before event emission:

```rust
let public_details = result
    .details
    .as_ref()
    .map(crate::diagnostic::redact_public_value);
let public_diagnostics: Vec<_> = result
    .diagnostics
    .iter()
    .map(|d| ToolDiagnostic {
        code: d.code.clone(),
        message: d.message.clone(),
        context: crate::diagnostic::redact_public_value(&d.context),
    })
    .collect();
events(AgentEvent::ToolExecutionEnd {
    tool_call_id: parsed.tool_call.id.clone(),
    tool_name: parsed.tool_call.name.clone(),
    result: serde_json::json!(&result.content),
    details: public_details.clone(),
    truncated,
    is_error,
    diagnostics: public_diagnostics,
});
```

Then persist `public_details`, not raw `result.details`, into `ToolResultMessage`:

```rust
let trm = ToolResultMessage {
    tool_call_id: parsed.tool_call.id,
    tool_name: parsed.tool_call.name,
    content: result.content,
    details: public_details,
    is_error,
    truncated,
    timestamp_ms: 0,
};
```

Apply the same transformation in the parallel branch.

- [ ] **Step 5: Add RPC regression for bash command canary**

In `crates/opi-coding-agent/tests/rpc_jsonl.rs`, add a variant of `tool_result_details_diagnostics_and_truncated_shape` whose command contains `OPI_RPC_COMMAND_SECRET_CANARY` and exits non-zero. Assert every emitted JSON line lacks the canary while still carrying `ToolExecutionEnd`, `is_error: true`, `truncated: false`, and a diagnostic code.

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_tool_events_redact_bash_command_canary() {
    let command = if cfg!(windows) {
        "echo OPI_RPC_COMMAND_SECRET_CANARY && exit /B 1"
    } else {
        "echo OPI_RPC_COMMAND_SECRET_CANARY; exit 1"
    };
    let args = serde_json::to_string(&serde_json::json!({ "command": command })).unwrap();
    let provider = MockProvider::new(
        "mock",
        vec![
            opi_ai::test_support::tool_call_response("tc-bash", "bash", &args),
            text_response("done"),
        ],
    );
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let runner = RpcRunner::new(
        Box::new(provider),
        "mock:mock-model".into(),
        OpiConfig::default(),
        workspace.path().to_path_buf(),
        true,
        ToolSelection::Allowlist(vec!["bash".into()]),
        None,
        Vec::new(),
    )
    .expect("rpc runner with bash tool");

    let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
    let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        let mut runner = runner;
        runner.run_with_channels(command_rx, output_tx).await
    });

    let ready = recv_rpc_line(&mut output_rx).await;
    assert!(
        !serde_json::to_string(&ready)
            .unwrap()
            .contains("OPI_RPC_COMMAND_SECRET_CANARY")
    );
    command_tx
        .send(RpcCommand::prompt {
            id: Some("bash-canary".into()),
            message: "run failing command".into(),
        })
        .unwrap();
    let accepted = recv_response(&mut output_rx, "prompt").await;
    assert_eq!(accepted["success"], true);

    let mut saw_bash_end = false;
    for _ in 0..64 {
        let line = recv_rpc_line(&mut output_rx).await;
        let rendered = serde_json::to_string(&line).unwrap();
        assert!(
            !rendered.contains("OPI_RPC_COMMAND_SECRET_CANARY"),
            "{rendered}"
        );
        if line["type"] == "ToolExecutionEnd" && line["tool_name"] == "bash" {
            saw_bash_end = true;
            assert_eq!(line["is_error"], true);
            assert_eq!(line["truncated"], false);
            assert!(
                line["diagnostics"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|d| d["code"] == "tool_execution_failed")),
                "{line}"
            );
        }
        if line["type"] == "AgentEnd" {
            break;
        }
    }
    assert!(saw_bash_end, "expected bash ToolExecutionEnd");

    command_tx.send(RpcCommand::quit { id: None }).unwrap();
    let _ = recv_response(&mut output_rx, "quit").await;
    task.await.unwrap().unwrap();
}
```

- [ ] **Step 6: Verify**

Run:

```sh
cargo test -p opi-agent --test tool_event_redaction
cargo test -p opi-coding-agent --test rpc_jsonl rpc_tool_events_redact_bash_command_canary
cargo test -p opi-coding-agent --test rpc_jsonl tool_result_details_diagnostics_and_truncated_shape
```

Expected after implementation: all pass. Existing shape tests should be updated to expect redacted command/path fields where they currently assert raw values.

- [ ] **Step 7: Review diff and stop**

Run: `git diff -- crates/opi-agent/src/diagnostic.rs crates/opi-agent/src/event.rs crates/opi-agent/src/agent_loop.rs crates/opi-agent/tests/tool_event_redaction.rs crates/opi-coding-agent/tests/rpc_jsonl.rs`

Expected: only public redaction boundary changes and tests. Do not commit.

### Task 2: Verbose Trace Structural Redaction

**Files:**
- Modify: `crates/opi-agent/src/diagnostic.rs`
- Test: `crates/opi-agent/tests/trace_envelope.rs`

**Interfaces:**
- Consumes: `CONTENT_SENSITIVE_KEYS`, `SecretRedactor`, `redact_summary`.
- Produces: `redact(value, RedactionMode::Verbose)` that still scrubs structural sensitive keys while preserving non-sensitive verbose detail.

- [ ] **Step 1: Write failing verbose trace test**

Add or adjust a test in `trace_envelope.rs`:

```rust
#[test]
fn phase7_verbose_trace_redacts_structural_tool_context() {
    let value = serde_json::json!({
        "command": "echo OPI_VERBOSE_COMMAND_SECRET",
        "cwd": "C:\\Users\\Luiz\\secret-worktree",
        "exit_code": 1,
        "safe_counter": 7
    });
    let redacted = opi_agent::diagnostic::redact(
        &value,
        opi_agent::diagnostic::RedactionMode::Verbose,
    );
    let text = serde_json::to_string(&redacted).unwrap();
    assert!(!text.contains("OPI_VERBOSE_COMMAND_SECRET"), "{text}");
    assert!(!text.contains("secret-worktree"), "{text}");
    assert_eq!(redacted["safe_counter"], 7);
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p opi-agent --test trace_envelope phase7_verbose_trace_redacts_structural_tool_context`

Expected before implementation: FAIL because verbose mode leaves `command`/`cwd` intact unless the value matches a secret regex.

- [ ] **Step 3: Implement structural verbose redaction**

In `diagnostic.rs`, split the current summary-only structural scrub into two layers:

```rust
fn redact_structural_sensitive_fields(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| {
                    if is_content_sensitive_key(k) {
                        (k.clone(), serde_json::Value::String("[REDACTED]".into()))
                    } else {
                        (k.clone(), redact_structural_sensitive_fields(v))
                    }
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items.iter().map(redact_structural_sensitive_fields).collect(),
        ),
        other => other.clone(),
    }
}
```

Then update `redact`:

```rust
pub fn redact(value: &serde_json::Value, mode: RedactionMode) -> serde_json::Value {
    let scrubbed = SecretRedactor::default().redact(value);
    let structurally_scrubbed = redact_structural_sensitive_fields(&scrubbed);
    match mode {
        RedactionMode::Summary => redact_summary_paths(&structurally_scrubbed),
        RedactionMode::Verbose => structurally_scrubbed,
    }
}
```

Use the existing function names where possible. If `redact_summary` currently handles both sensitive keys and path regexes, refactor it into a structural-key function and a summary path/value function instead of duplicating logic.

- [ ] **Step 4: Verify**

Run:

```sh
cargo test -p opi-agent --test trace_envelope phase7_verbose_trace_redacts_structural_tool_context
cargo test -p opi-agent --test trace_envelope phase7_trace_redacts_sensitive_values_in_diagnostic_linked
```

Expected: both pass.

- [ ] **Step 5: Review diff and stop**

Run: `git diff -- crates/opi-agent/src/diagnostic.rs crates/opi-agent/tests/trace_envelope.rs`

Expected: redaction refactor only. Do not commit.

### Task 3: Bash Temp Files And Process Reliability

**Files:**
- Modify: `crates/opi-coding-agent/src/tool/bash.rs`
- Test: `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`

**Interfaces:**
- Produces: `fn create_private_temp_file(path: &Path) -> std::io::Result<std::fs::File>`.
- Produces: `fn bash_output_temp_path() -> PathBuf` with random collision resistance.
- Updates: `BashTool` command uses `.kill_on_drop(true)`.
- Updates: `StreamCapture` keeps draining after spill append errors.

- [ ] **Step 1: Write failing tests for restricted full-output files**

On Unix only, add a test that runs a truncating bash command, gets `details.full_output`, and checks permissions are `0o600`.

```rust
#[cfg(unix)]
#[tokio::test]
async fn bash_full_output_file_is_private_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "private-full-output",
            serde_json::json!({
                "command": bash_oversized_stdout_command(2000),
                "timeout_secs": 30
            }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    let full = result.details.as_ref().unwrap()["full_output"].as_str().unwrap();
    let mode = std::fs::metadata(full).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "full_output must not be group/world-readable");
}
```

- [ ] **Step 2: Write failing test for command canary in bash diagnostics**

Add a direct bash tool test for failed command diagnostics:

```rust
#[tokio::test]
async fn bash_failure_diagnostics_redact_command_canary() {
    let dir = tempfile::tempdir().unwrap();
    let tool = BashTool::new(dir.path().to_path_buf());
    let command = if cfg!(windows) {
        "echo OPI_BASH_COMMAND_SECRET_CANARY && exit /B 1"
    } else {
        "echo OPI_BASH_COMMAND_SECRET_CANARY; exit 1"
    };
    let result = tool
        .execute(
            "command-canary",
            serde_json::json!({ "command": command, "timeout_secs": 30 }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(result.is_error);
    let diagnostics = serde_json::to_string(&result.diagnostics).unwrap();
    assert!(
        !diagnostics.contains("OPI_BASH_COMMAND_SECRET_CANARY"),
        "{diagnostics}"
    );
}
```

This test should fail before either bash stops putting raw command in diagnostics or the expected contract is shifted to public-boundary redaction only. Prefer removing raw command from `ToolDiagnostic.context`; public-boundary redaction from Task 1 is still required for `details.command`.

- [ ] **Step 3: Run failing tests**

Run:

```sh
cargo test -p opi-coding-agent --test tools_read_write_edit_bash bash_failure_diagnostics_redact_command_canary
cargo test -p opi-coding-agent --test tools_read_write_edit_bash bash_full_output_file_is_private_on_unix
```

Expected before implementation: command-canary test fails; Unix permissions test fails if the platform creates temp files with group/world-readable mode.

- [ ] **Step 4: Remove raw command from bash-owned diagnostics**

Change `bash_operation_diagnostic` so context keeps operational booleans/code but not the command text:

```rust
context: json!({
    "exit_code": exit_code,
    "cancelled": cancelled,
    "timed_out": timed_out,
    "truncated": truncated,
}),
```

If consumers need a command indicator, add `"command_included": false`; do not include hashes unless there is an actual consumer that needs correlation.

- [ ] **Step 5: Create private temp files**

In `bash.rs`, replace `std::fs::File::create` at spill and merged-output sites with a helper.

```rust
#[cfg(unix)]
fn create_private_temp_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private_temp_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}
```

Use `create_new(true)` so name collisions fail instead of truncating another spill.

- [ ] **Step 6: Add random suffix to bash temp paths**

Use workspace dependency policy. If `tempfile` is already in `[workspace.dependencies]`, prefer it. Otherwise avoid adding a dependency and include process id, thread id, nanos, and an atomic counter:

```rust
static BASH_TEMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn bash_output_temp_path() -> PathBuf {
    let pid = std::process::id();
    let seq = BASH_TEMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("opi-bash-output-{pid}-{nanos}-{seq}.log"))
}
```

- [ ] **Step 7: Set kill-on-drop and record kill failures**

Update command construction:

```rust
cmd.arg(flag)
    .arg(&command)
    .current_dir(&cwd)
    .kill_on_drop(true);
```

Replace ignored kill results with a local flag:

```rust
let kill_error = match child.kill().await {
    Ok(()) => None,
    Err(e) => Some(e.to_string()),
};
```

Thread `kill_error` into details or diagnostics for cancel/timeout branches without exposing command text.

- [ ] **Step 8: Keep draining after spill append failures**

Change stdout/stderr drain loops so append failure stops storing but continues reading to EOF:

```rust
let mut out_spill_error: Option<String> = None;
// inside read loop:
if out_spill_error.is_none()
    && let Err(e) = out_cap.append(&buf[..n])
{
    out_spill_error = Some(e.to_string());
}
```

Mirror for stderr. Add `"stdout_spill_error": true`/`"stderr_spill_error": true` booleans or a diagnostic without raw paths.

- [ ] **Step 9: Fix WaitFailed metadata and saturating totals**

For `Control::WaitFailed`, return through `bash_result` with stable details:

```rust
let details = with_env_policy(result::bash_operation_metadata(
    &workspace_root,
    "[REDACTED]",
    &cwd,
    shell,
    None,
    false,
    false,
    false,
    None,
));
Ok(bash_result(
    vec![OutputContent::Text { text: "failed to wait for process".to_string() }],
    details,
    true,
    false,
    "",
    None,
    false,
    false,
))
```

After Step 4, `bash_result` no longer stores command in diagnostics. Keep `details.command` redacted or omitted on this unusual branch.

Change:

```rust
let total = out_cap.total.saturating_add(err_cap.total);
```

- [ ] **Step 10: Verify**

Run:

```sh
cargo test -p opi-coding-agent --test tools_read_write_edit_bash bash_failure_diagnostics_redact_command_canary
cargo test -p opi-coding-agent --test tools_read_write_edit_bash bash_full_output_file_is_private_on_unix
cargo test -p opi-coding-agent --test tools_read_write_edit_bash bash_tool_no_secret_leakage_in_diagnostics_and_env_reporting
cargo test -p opi-coding-agent --test rpc_jsonl bash_tool_result_carries_truncation_details
```

Expected: all pass. If Windows skips Unix mode test, verify the test is `#[cfg(unix)]` and not a false pass.

- [ ] **Step 11: Review diff and stop**

Run: `git diff -- crates/opi-coding-agent/src/tool/bash.rs crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs crates/opi-coding-agent/tests/rpc_jsonl.rs`

Expected: bash reliability/security changes only. Do not commit.

### Task 4: Read, Write, And Edit Contract Corrections

**Files:**
- Modify: `crates/opi-coding-agent/src/tool/read.rs`
- Modify: `crates/opi-coding-agent/src/tool/write.rs`
- Modify: `crates/opi-coding-agent/README.md`
- Modify: `crates/opi-coding-agent/README.zh.md`
- Test: `crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs`

**Interfaces:**
- Produces: read `details.line_ending` with values `"lf"`, `"crlf"`, `"cr"`, `"mixed"`, or `"none"`.
- Produces: `const MAX_READ_OUTPUT_BYTES: usize = 64 * 1024`.
- Produces: write existing-directory target classified as `CODE_TOOL_NOT_A_FILE`.
- Keeps: edit requires a unique exact match.

- [ ] **Step 1: Write failing read CRLF metadata test**

```rust
#[tokio::test]
async fn read_tool_reports_line_ending_metadata_for_crlf() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("crlf.txt"), b"alpha\r\nbeta\r\n").unwrap();
    let tool = ReadTool::new(dir.path().to_path_buf(), PathPolicy::WorkspaceOnly);
    let result = tool
        .execute(
            "read-crlf",
            serde_json::json!({ "path": "crlf.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(!result.is_error);
    assert_eq!(result.details.as_ref().unwrap()["line_ending"], "crlf");
    assert!(tool_result_text(&result).contains("alpha\nbeta"));
}
```

- [ ] **Step 2: Write failing read byte cap test**

```rust
#[tokio::test]
async fn read_tool_caps_single_line_output_by_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let huge = "a".repeat(opi_coding_agent::tool::MAX_READ_OUTPUT_BYTES + 1024);
    std::fs::write(dir.path().join("one-line.txt"), huge).unwrap();
    let tool = ReadTool::new(dir.path().to_path_buf(), PathPolicy::WorkspaceOnly);
    let result = tool
        .execute(
            "read-huge-line",
            serde_json::json!({ "path": "one-line.txt" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(result.truncated);
    let text = tool_result_text(&result);
    assert!(text.len() <= opi_coding_agent::tool::MAX_READ_OUTPUT_BYTES + 512);
    assert_eq!(result.details.as_ref().unwrap()["truncated"], true);
    assert_eq!(result.details.as_ref().unwrap()["truncation_reason"], "byte_cap");
}
```

- [ ] **Step 3: Write failing write-to-directory classification test**

```rust
#[tokio::test]
async fn write_tool_existing_directory_returns_not_a_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("target")).unwrap();
    let tool = WriteTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "write-dir",
            serde_json::json!({ "path": "target", "content": "new" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.details.is_none());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == opi_agent::diagnostic::code::CODE_TOOL_NOT_A_FILE),
        "{:?}",
        result.diagnostics
    );
}
```

- [ ] **Step 4: Run failing tests**

Run:

```sh
cargo test -p opi-coding-agent --test tools_read_write_edit_bash read_tool_reports_line_ending_metadata_for_crlf
cargo test -p opi-coding-agent --test tools_read_write_edit_bash read_tool_caps_single_line_output_by_bytes
cargo test -p opi-coding-agent --test tools_read_write_edit_bash write_tool_existing_directory_returns_not_a_file
```

Expected before implementation: all fail.

- [ ] **Step 5: Implement line-ending detection**

In `read.rs`, add:

```rust
fn detect_line_ending(bytes: &[u8]) -> &'static str {
    let mut lf = false;
    let mut crlf = false;
    let mut cr = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                crlf = true;
                i += 2;
            }
            b'\r' => {
                cr = true;
                i += 1;
            }
            b'\n' => {
                lf = true;
                i += 1;
            }
            _ => i += 1,
        }
    }
    match (lf, crlf, cr) {
        (false, false, false) => "none",
        (true, false, false) => "lf",
        (false, true, false) => "crlf",
        (false, false, true) => "cr",
        _ => "mixed",
    }
}
```

Set `details["line_ending"] = json!(detect_line_ending(&bytes));`.

- [ ] **Step 6: Implement bounded binary sampling and byte output cap**

Add exported constants in `tool/mod.rs` or `read.rs` depending on existing test visibility:

```rust
pub const MAX_READ_OUTPUT_BYTES: usize = 64 * 1024;
const READ_BINARY_SAMPLE_BYTES: usize = 64 * 1024;
```

Replace full NUL scan for large files with a sampled scan:

```rust
let sample_len = bytes.len().min(READ_BINARY_SAMPLE_BYTES);
if bytes[..sample_len].contains(&0u8) {
    return Ok(super::fs_error_result(FsToolError::BinaryFile { path: file_path.clone() }));
}
```

After assembling `body`, cap by valid UTF-8 character boundary:

```rust
let mut truncation_reason = None;
if body.len() > MAX_READ_OUTPUT_BYTES {
    let mut end = MAX_READ_OUTPUT_BYTES;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    body.truncate(end);
    body.push_str("\n... output truncated by byte cap");
    truncation_reason = Some("byte_cap");
}
let truncated = omitted > 0 || truncation_reason.is_some();
details["truncated"] = json!(truncated);
if let Some(reason) = truncation_reason {
    details["truncation_reason"] = json!(reason);
}
```

Keep explicit line `limit` behavior but make byte cap non-optional; the byte cap protects model/payload size.

- [ ] **Step 7: Classify write target directory**

In `write.rs`, after resolving `file_path` and before temp path creation:

```rust
if let Ok(meta) = tokio::fs::metadata(&file_path).await
    && meta.is_dir()
{
    return Ok(super::fs_error_result(FsToolError::NotAFile {
        path: file_path.clone(),
    }));
}
```

If metadata returns permission denied, classify `PermissionDenied`. Keep parent `NotADirectory` classification unchanged.

- [ ] **Step 8: Fix README edit wording**

Change English row:

```markdown
| `edit` | `path`, `old_string`, `new_string` | Replaces the unique exact match and records before/after details; sequential; mutating. |
```

Change Chinese counterpart to the equivalent meaning: "替换唯一精确匹配".

- [ ] **Step 9: Verify**

Run:

```sh
cargo test -p opi-coding-agent --test tools_read_write_edit_bash read_tool_reports_line_ending_metadata_for_crlf
cargo test -p opi-coding-agent --test tools_read_write_edit_bash read_tool_caps_single_line_output_by_bytes
cargo test -p opi-coding-agent --test tools_read_write_edit_bash write_tool_existing_directory_returns_not_a_file
cargo test -p opi-coding-agent --test tools_read_write_edit_bash crlf
```

Expected: all pass.

- [ ] **Step 10: Review diff and stop**

Run: `git diff -- crates/opi-coding-agent/src/tool/read.rs crates/opi-coding-agent/src/tool/write.rs crates/opi-coding-agent/tests/tools_read_write_edit_bash.rs crates/opi-coding-agent/README.md crates/opi-coding-agent/README.zh.md`

Expected: no unrelated edit refactor. Do not commit.

### Task 5: Navigation Tool Bounded Work And Skipped Diagnostics

**Files:**
- Modify: `crates/opi-coding-agent/src/tool/mod.rs`
- Modify: `crates/opi-coding-agent/src/tool/grep.rs`
- Modify: `crates/opi-coding-agent/src/tool/find.rs`
- Modify: `crates/opi-coding-agent/src/tool/glob.rs`
- Modify: `crates/opi-coding-agent/src/tool/ls.rs`
- Test: `crates/opi-coding-agent/tests/tools_glob_grep.rs`
- Test: `crates/opi-coding-agent/tests/find_tool.rs`
- Test: `crates/opi-coding-agent/tests/ls_tool.rs`

**Interfaces:**
- Produces: `pub const MAX_NAV_VISITED_ENTRIES: usize = 10_000`.
- Produces: `pub const MAX_GREP_TOTAL_READ_BYTES: u64 = 8 * 1024 * 1024`.
- Produces details keys: `visited_entries`, `search_terminated_early`, `files_skipped_non_utf8`, `files_skipped_unreadable`, `files_skipped_permission_denied`, and `omitted_count`.

- [ ] **Step 1: Write failing grep skipped-content test**

```rust
#[tokio::test]
async fn grep_reports_non_utf8_content_skips() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("bad.txt"), [0xff, 0xfe, b'a']).unwrap();
    let tool = GrepTool::new(dir.path().to_path_buf());
    let result = tool
        .execute(
            "grep-non-utf8",
            serde_json::json!({ "pattern": "a" }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(!result.is_error);
    assert_eq!(
        result.details.as_ref().unwrap()["files_skipped_non_utf8"],
        serde_json::json!(1)
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == opi_agent::diagnostic::code::CODE_TOOL_UNSUPPORTED_ENCODING),
        "{:?}",
        result.diagnostics
    );
}
```

- [ ] **Step 2: Write failing ls omitted-count test**

```rust
#[tokio::test]
async fn ls_reports_omitted_count_when_truncated() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..(opi_coding_agent::tool::MAX_NAV_RESULTS + 3) {
        std::fs::write(dir.path().join(format!("f{i:04}.txt")), "x").unwrap();
    }
    let tool = LsTool::new(dir.path().to_path_buf(), PathPolicy::WorkspaceOnly);
    let result = tool
        .execute(
            "ls-truncated",
            serde_json::json!({ "path": "." }),
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
    assert!(result.truncated);
    assert_eq!(
        result.details.as_ref().unwrap()["omitted_count"],
        serde_json::json!(3)
    );
}
```

- [ ] **Step 3: Write bounded-work tests**

For grep, create one file with more than `MAX_NAV_RESULTS + 20` matching lines. Assert `match_count <= MAX_NAV_RESULTS + 1`, `truncated == true`, and `search_terminated_early == true`.

For find/glob/ls, create more than `MAX_NAV_RESULTS + 20` matching files. Assert returned lines are capped, `truncated == true`, and `visited_entries` is not larger than the new guard for pre-cancelled and cap-triggered cases.

- [ ] **Step 4: Run failing tests**

Run:

```sh
cargo test -p opi-coding-agent --test tools_glob_grep grep_reports_non_utf8_content_skips
cargo test -p opi-coding-agent --test ls_tool ls_reports_omitted_count_when_truncated
```

Expected before implementation: grep skip diagnostics and ls omitted count fail.

- [ ] **Step 5: Add shared nav budget constants**

In `tool/mod.rs`:

```rust
pub const MAX_NAV_VISITED_ENTRIES: usize = 10_000;
pub const MAX_GREP_TOTAL_READ_BYTES: u64 = 8 * 1024 * 1024;
```

Document that `omitted_count` is exact when traversal completes and a lower bound when `search_terminated_early` is true.

- [ ] **Step 6: Align grep path and content diagnostics**

In `grep.rs`, replace lossy path conversion:

```rust
let relative_os = path.strip_prefix(&workspace_root).unwrap_or(path);
let Some(relative) = relative_os.to_str() else {
    files_skipped_non_utf8 += 1;
    continue;
};
```

Classify read failures:

```rust
let content = match std::fs::read_to_string(path) {
    Ok(c) => c,
    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
        files_skipped_permission_denied += 1;
        continue;
    }
    Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
        files_skipped_non_utf8 += 1;
        continue;
    }
    Err(_) => {
        files_skipped_unreadable += 1;
        continue;
    }
};
```

Push diagnostics when counts are non-zero:

```rust
if files_skipped_non_utf8 > 0 {
    tool_result.diagnostics.push(
        FsToolError::UnsupportedEncoding {
            omitted_count: files_skipped_non_utf8,
        }
        .to_diagnostic(),
    );
}
```

For permission skips, use `FsToolError::PermissionDenied` only if there is a concrete path. If multiple paths were skipped, add a generic `ToolDiagnostic` with `CODE_TOOL_PERMISSION_DENIED` and context `{"omitted_count": files_skipped_permission_denied}`.

- [ ] **Step 7: Bound traversal and collection**

For each nav tool loop:

```rust
let mut visited_entries = 0usize;
let mut search_terminated_early = false;
for entry in walker.flatten() {
    visited_entries += 1;
    if visited_entries > super::MAX_NAV_VISITED_ENTRIES {
        search_terminated_early = true;
        break;
    }
    // existing matching logic
    if matched.len() > super::MAX_NAV_RESULTS {
        search_terminated_early = true;
        break;
    }
}
```

For grep, also stop when cumulative successfully read bytes exceed `MAX_GREP_TOTAL_READ_BYTES`.

Keep returned results sorted after collection. Because early stop makes total omitted results unknown, set:

```rust
details["search_terminated_early"] = json!(search_terminated_early);
details["visited_entries"] = json!(visited_entries);
```

When early termination occurred and exact omitted count is unknown, set `omitted_count` to the known lower bound:

```rust
let omitted_count = omitted_count.max(usize::from(search_terminated_early));
```

- [ ] **Step 8: Add ls `omitted_count`**

In `ls.rs` details:

```rust
"omitted_count": omitted,
"visited_entries": visited_entries,
"search_terminated_early": search_terminated_early,
```

- [ ] **Step 9: Verify**

Run:

```sh
cargo test -p opi-coding-agent --test tools_glob_grep
cargo test -p opi-coding-agent --test find_tool
cargo test -p opi-coding-agent --test ls_tool
```

Expected: all nav tests pass. Pay special attention to existing sort/order tests after early-stop behavior.

- [ ] **Step 10: Review diff and stop**

Run: `git diff -- crates/opi-coding-agent/src/tool/mod.rs crates/opi-coding-agent/src/tool/grep.rs crates/opi-coding-agent/src/tool/find.rs crates/opi-coding-agent/src/tool/glob.rs crates/opi-coding-agent/src/tool/ls.rs crates/opi-coding-agent/tests/tools_glob_grep.rs crates/opi-coding-agent/tests/find_tool.rs crates/opi-coding-agent/tests/ls_tool.rs`

Expected: nav tools only. Do not commit.

### Task 6: CLI Help And Documentation Truthfulness

**Files:**
- Modify: `crates/opi-coding-agent/src/cli.rs`
- Modify: `crates/opi-coding-agent/tests/non_interactive.rs`
- Modify: `crates/opi-coding-agent/tests/phase11_tooling_quality_docs.rs`
- Modify: `crates/opi-coding-agent/README.md`
- Modify: `crates/opi-coding-agent/README.zh.md`
- Modify: `docs/opi-spec.md`
- Modify: `docs/opi-spec.zh.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Produces: `opi --help` text that mentions tool-selection precedence, mutating opt-in, bash cwd/shell/timeout, 64 KiB truncation, `details.full_output`, and permission-popup non-goal.
- Produces: spec structs matching current code fields.

- [ ] **Step 1: Strengthen failing help assertions**

In `non_interactive.rs::phase11_cli_help_tool_policy`, assert:

```rust
for phrase in [
    "cmd /C",
    "sh -c",
    "workspace root",
    "30 seconds",
    "timeout_secs",
    "64 KiB",
    "details.full_output",
    "permission popup",
] {
    assert!(help.contains(phrase), "opi --help must mention {phrase}");
}
```

In `phase11_tooling_quality_docs.rs`, replace the loose `"subsystem."` assertion with:

```rust
assert!(
    readme.contains("tool-selection check, not a permission or sandbox subsystem"),
    "README must explain mutating-tool safety is tool-selection, not permission/sandbox"
);
```

Add `docs/opi-spec.zh.md` to the sync test and assert it includes the Chinese equivalents of `--allow-mutating`, no core permission popups, and Phase 11 completed status after docs are updated.

- [ ] **Step 2: Run failing docs/help tests**

Run:

```sh
cargo test -p opi-coding-agent --test non_interactive phase11_cli_help_tool_policy
cargo test -p opi-coding-agent --test phase11_tooling_quality_docs policy_docs_and_help_stay_in_sync
```

Expected before implementation: help phrase assertions fail.

- [ ] **Step 3: Add long help text**

In `cli.rs`, change the command attribute:

```rust
#[command(
    name = "opi",
    version,
    about = "AI coding agent",
    after_long_help = "\
Tool policy:
  Interactive mode enables read, write, edit, and bash.
  Non-interactive/RPC mode defaults to read, grep, find, ls, and glob.
  write, edit, and bash require --allow-mutating or defaults.allow_mutating_tools = true outside interactive mode.
  --no-tools disables all tools; --tools is an allowlist; --no-builtin-tools removes built-ins.

Bash policy:
  bash runs one foreground command from the workspace root.
  Windows uses cmd /C; Unix uses sh -c.
  The default timeout is 30 seconds; timeout_secs overrides it per call.
  Combined stdout/stderr are capped at 64 KiB. Larger output sets truncated and may write the complete output path in details.full_output.
  This is a tool-selection check, not a permission popup or sandbox subsystem.
"
)]
```

Use clap's exact attribute spelling accepted by the current version. If `after_long_help` is unsupported, use `after_help`.

- [ ] **Step 4: Update opi spec structs**

In both specs, update `ToolResultMessage`:

```rust
pub struct ToolResultMessage {
    pub tool_call_id: String,
    pub tool_name: String,
    pub content: Vec<OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub truncated: bool,
    pub timestamp_ms: i64,
}
```

Update `AgentEvent::ToolExecutionEnd`:

```rust
ToolExecutionEnd {
    tool_call_id: String,
    tool_name: String,
    result: serde_json::Value,
    details: Option<serde_json::Value>,
    is_error: bool,
    truncated: bool,
    diagnostics: Vec<ToolDiagnostic>,
}
```

Update `ToolResult`:

```rust
pub struct ToolResult {
    pub content: Vec<opi_ai::OutputContent>,
    pub details: Option<serde_json::Value>,
    pub is_error: bool,
    pub terminate: bool,
    pub truncated: bool,
    pub diagnostics: Vec<ToolDiagnostic>,
}
```

Add text stating error results keep `details: None`; structured failure metadata lives in `diagnostics[].context` and is redacted at public event/session boundaries.

- [ ] **Step 5: Update Phase 11 roadmap status**

In both specs, change Phase 11 status from planned to completed and list delivered items: tool result contract, filesystem taxonomy, read/write/edit/bash hardening, nav consistency, diagnostics/trace lift, provider `is_error`, docs/help guards.

- [ ] **Step 6: Update README edit row and glob framing**

Use the edit wording from Task 4. Add one sentence under relationship to pi:

```markdown
`glob` is an opi convenience tool; pi-compatible workflows should not depend on it being the only discovery path.
```

Add the Chinese counterpart in `README.zh.md`.

- [ ] **Step 7: Update CHANGELOG**

Under `## [Unreleased]`, add:

```markdown
### Changed

- Clarified Phase 11 tool-result, event, and session metadata contracts in the normative specs, including public redaction boundaries for tool details and diagnostics.

### Fixed

- Redacted command/path-sensitive tool metadata before public events and session persistence.
- Corrected CLI help and README descriptions for Phase 11 tool policy and unique-match edit behavior.
```

Adjust wording if Tasks 1-5 are not all implemented in the same branch. Do not modify released sections.

- [ ] **Step 8: Verify**

Run:

```sh
cargo test -p opi-coding-agent --test non_interactive phase11_cli_help_tool_policy
cargo test -p opi-coding-agent --test phase11_tooling_quality_docs policy_docs_and_help_stay_in_sync
cargo run -p opi-coding-agent -- --help
```

Expected: tests pass; help output visibly contains the policy text.

- [ ] **Step 9: Review diff and stop**

Run: `git diff -- crates/opi-coding-agent/src/cli.rs crates/opi-coding-agent/tests/non_interactive.rs crates/opi-coding-agent/tests/phase11_tooling_quality_docs.rs crates/opi-coding-agent/README.md crates/opi-coding-agent/README.zh.md docs/opi-spec.md docs/opi-spec.zh.md CHANGELOG.md`

Expected: docs/help/test guard changes only. Do not commit.

### Task 7: Shared OpenAI Tool Error Marker

**Files:**
- Modify: `crates/opi-ai/src/message.rs` or create `crates/opi-ai/src/tool_result_wire.rs`
- Modify: `crates/opi-ai/src/openai_chat.rs`
- Modify: `crates/opi-ai/src/openai_responses.rs`
- Test: `crates/opi-ai/tests/tool_result_wire.rs`

**Interfaces:**
- Produces: `pub(crate) const TOOL_ERROR_MARKER: &str = "[tool_error] ";`.

- [ ] **Step 1: Add shared constant**

If using `message.rs`, add:

```rust
pub(crate) const TOOL_ERROR_MARKER: &str = "[tool_error] ";
```

If using a new module, add it to `crates/opi-ai/src/lib.rs`:

```rust
mod tool_result_wire;
```

and define:

```rust
pub(crate) const TOOL_ERROR_MARKER: &str = "[tool_error] ";
```

- [ ] **Step 2: Replace duplicated constants**

In `openai_chat.rs` and `openai_responses.rs`, import the shared constant and delete local private constants.

```rust
use crate::message::TOOL_ERROR_MARKER;
```

or:

```rust
use crate::tool_result_wire::TOOL_ERROR_MARKER;
```

- [ ] **Step 3: Keep marker behavior tests**

Update `tool_result_wire.rs` imports if needed. Keep the existing tests that assert success bodies are byte-identical and error bodies start with `[tool_error] `.

- [ ] **Step 4: Verify**

Run: `cargo test -p opi-ai --test tool_result_wire`

Expected: pass.

- [ ] **Step 5: Review diff and stop**

Run: `git diff -- crates/opi-ai/src crates/opi-ai/tests/tool_result_wire.rs`

Expected: constant move only. Do not commit.

### Task 8: Final Workspace Verification

**Files:**
- No new source files unless earlier tasks created them.

**Interfaces:**
- Consumes: all previous task deliverables.
- Produces: verified, lint-clean workspace.

- [ ] **Step 1: Run focused tests from all changed areas**

Run:

```sh
cargo test -p opi-agent --test tool_event_redaction
cargo test -p opi-agent --test trace_envelope
cargo test -p opi-coding-agent --test tools_read_write_edit_bash
cargo test -p opi-coding-agent --test tools_glob_grep
cargo test -p opi-coding-agent --test find_tool
cargo test -p opi-coding-agent --test ls_tool
cargo test -p opi-coding-agent --test non_interactive phase11_cli_help_tool_policy
cargo test -p opi-coding-agent --test phase11_tooling_quality_docs
cargo test -p opi-coding-agent --test rpc_jsonl rpc_tool_events_redact_bash_command_canary
cargo test -p opi-ai --test tool_result_wire
```

Expected: all pass.

- [ ] **Step 2: Run format and clippy gates**

Run:

```sh
cargo fmt --all
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 3: Run full test gate if time permits**

Run: `cargo test --workspace --all-targets`

Expected: pass. If this is too slow for the session, record exactly which focused tests and clippy passed.

- [ ] **Step 4: Inspect working tree**

Run: `git status --short`

Expected: only files modified by these tasks plus the existing untracked audit documents, unless the user has added work concurrently.

- [ ] **Step 5: Final review**

Run: `git diff --stat` and inspect changed hunks in each modified file.

Expected: no unrelated refactors, no released changelog sections modified, no unrequested commits.

## Deferred Or Explicitly Non-Blocking Items

- Do not implement shell syntax blocking for `&`, `nohup`, `setsid`, `disown`, or `start /B` in this repair. That is a product policy change, not a Phase 11 bug fix. Document the current non-goal boundary instead.
- Do not add edit-specific diagnostic codes in this repair. Current edit semantic errors intentionally use `tool_execution_failed` plus structured context.
- Do not remove `WorkspaceRelation::Unresolved` unless a separate cleanup is requested.
- Do not change provider request bodies to include `details`, `diagnostics`, or `truncated`.
- Do not redesign session format or bump NDJSON/RPC schema versions unless a separate compatibility decision is made. This repair uses additive documentation and redaction at existing boundaries.
