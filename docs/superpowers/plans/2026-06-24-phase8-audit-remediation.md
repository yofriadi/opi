# Phase 8 Audit Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the confirmed Phase 8 audit gaps without widening the runtime contract beyond the audited surface.

**Architecture:** Keep the fixes inside the existing `opi-agent` runtime contracts and `opi-coding-agent` documentation guards. Treat malformed tool-call arguments as the same class of normal runtime validation failure already used for schema failures, and make API surface classification exhaustive for crate-root re-exports.

**Tech Stack:** Rust 2024, `tokio`, `serde_json`, existing `opi-agent` diagnostics/trace APIs, existing integration tests.

## Global Constraints

- Do not commit unless the user explicitly asks.
- Do not use `git add -A` or `git add .`; if staging is later requested, stage only files changed by that execution session.
- After code changes, run `cargo clippy --workspace --all-targets -- -D warnings`.
- If a test file is created or modified, run that exact test target and iterate until it passes.
- Keep docs synchronized between `crates/opi-agent/README.md` and `crates/opi-agent/README.zh.md`.
- Do not add a new crate, shared `opi-types` crate, web UI, OAuth flow, MCP runtime, or workflow-heavy core feature.

---

## Verified Audit Status

Primary verification performed before this plan:

- `cargo test -p opi-agent --test tool_validation phase8_tool_validation_failure_contract`
  - Passed: existing schema-invalid JSON validation contract remains green.
- `cargo test -p opi-agent --test tool_validation empty_schema_accepts_any_object`
  - Passed: a permissive `{ "type": "object" }` schema accepts `{}`.
- `cargo test -p opi-coding-agent --test runtime_contract_docs phase8_api_surface_classification`
  - Passed: current guard only proves selected classification rows, not exhaustive crate-root re-export coverage.
- `cargo test -p opi-agent --test agent_loop_semantics phase8_cancellation_contract_during_stream_discards_partial_and_emits_agent_end`
  - Passed: current cancel contract test does not pin `TurnEnded`/`TurnEnd` pairing.
- `cargo test -p opi-agent --test diagnostics phase8_real_format_redaction_contract`
  - Passed: current redaction contract covers Anthropic/OpenAI/GitHub/credentialed URL/JWT patterns, not AWS access key IDs or Azure opaque keys.

Confirmed primary gaps:

1. `crates/opi-agent/src/agent_loop.rs` parses tool-call JSON with `serde_json::from_str(...).unwrap_or(json!({}))` in both sequential and parallel paths. A malformed argument string can become `{}` and execute under a permissive schema.
2. `crates/opi-agent/src/lib.rs` re-exports more crate-root public API than the `crates/opi-agent/README.md` API classification table covers, and `crates/opi-coding-agent/tests/runtime_contract_docs.rs` does not enumerate all crate-root re-exported names.
3. Provider-stream cancellation leaves the trace turn open today, but Phase 8 tests only pin `AgentEnd` and partial-message discard. The open-turn behavior should be documented and tested so future changes are intentional.

Confirmed lower-priority residuals to split into later issues:

- Default redaction does not value-match AWS access key IDs. This is security hardening, not a Phase 8 runtime-contract blocker.
- `observe()` remains convention-enforced rather than structurally guarded. This needs a separate observability-maintenance design.
- RPC `run_summary` remains an ad-hoc JSON event separate from `AgentSessionEvent::SessionSummary`. This is a future SDK/RPC wire-shape decision.
- `adapter_host_mock` remains a `harness = false` test binary. Fixing the workspace `cargo test --workspace --all-targets` exit behavior is useful but independent of the Phase 8 runtime contracts.

---

## File Structure

- Modify `crates/opi-agent/src/agent_loop.rs`
  - Replace malformed JSON fallback with explicit parse-failure runtime handling.
  - Preserve existing schema-validation, hook, tool execution, diagnostic, trace, and persistence semantics.
- Modify `crates/opi-agent/tests/tool_validation.rs`
  - Add regression coverage proving malformed JSON does not run a permissive tool and does not call `before_tool_call`.
- Modify `crates/opi-agent/tests/trace_envelope.rs`
  - Add malformed JSON trace/diagnostic coverage.
  - Add cancel-open-turn trace coverage.
- Modify `crates/opi-agent/README.md`
  - Classify every crate-root re-export or grouped re-export family.
  - Clarify provider-failure/provider-stream-cancel open-turn trace behavior.
- Modify `crates/opi-agent/README.zh.md`
  - Mirror the English README classification and cancellation wording.
- Modify `crates/opi-coding-agent/tests/runtime_contract_docs.rs`
  - Parse crate-root `pub use` statements and fail if any re-exported name is missing from EN or ZH API classification docs.

---

### Task 1: Reject Malformed Tool Arguments Before Hooks Or Execution

**Files:**
- Modify: `crates/opi-agent/src/agent_loop.rs`
- Test: `crates/opi-agent/tests/tool_validation.rs`
- Test: `crates/opi-agent/tests/trace_envelope.rs`

**Interfaces:**
- Consumes: `ToolCall.arguments: String`, `DiagnosticSink`, `TraceCollector`, `CODE_TOOL_VALIDATION_FAILED`.
- Produces: malformed JSON tool calls become error `ToolResult` values with `is_error = true`, `terminate = false`, no hook call, no `Tool::execute` call, `ToolExecutionEnd(is_error = true)`, `ToolCallFailed`, and a `tool_validation_failed` diagnostic.

- [ ] **Step 1: Add the permissive-tool regression test**

Add this test support to `crates/opi-agent/tests/tool_validation.rs` near `ProbeTool`:

```rust
/// A permissive tool that records whether malformed JSON reached execute.
struct PermissiveProbeTool {
    executed: Arc<Mutex<bool>>,
}

impl Tool for PermissiveProbeTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "echo".into(),
            description: "permissive probe tool".into(),
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
        let executed = self.executed.clone();
        Box::pin(async move {
            *executed.lock().unwrap() = true;
            Ok(ToolResult {
                content: vec![OutputContent::Text {
                    text: "execute must not run on malformed arguments".into(),
                }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }
}
```

Add this test near `phase8_tool_validation_failure_contract`:

```rust
#[tokio::test]
async fn phase8_malformed_tool_arguments_do_not_execute_permissive_tool() {
    use opi_agent::diagnostic::code::CODE_TOOL_VALIDATION_FAILED;
    use opi_agent::diagnostic_sink::RecordingSink;
    use opi_agent::event::AgentEvent;

    let executed = Arc::new(Mutex::new(false));
    let before_called = Arc::new(Mutex::new(false));
    let diagnostic_sink = Arc::new(RecordingSink::new());
    let start_args = Arc::new(Mutex::new(Vec::new()));
    let end_errors = Arc::new(Mutex::new(Vec::new()));

    let provider = ScriptedProvider::new(vec![
        tool_call_response("call-1", "echo", "{not-json"),
        text_response("done"),
    ]);
    let call_count = provider.call_count.clone();

    let tools: Vec<Box<dyn Tool>> = vec![Box::new(PermissiveProbeTool {
        executed: executed.clone(),
    })];
    let hooks = ProbeHooks {
        before_called: before_called.clone(),
    };

    let context = AgentLoopContext {
        provider: Box::new(provider),
        tools,
        messages: vec![AgentMessage::Llm(Message::User(
            opi_ai::message::UserMessage {
                content: vec![InputContent::Text { text: "hi".into() }],
                timestamp_ms: 0,
            },
        ))],
        model: "mock".into(),
        system: None,
        steering_queue: None,
        follow_up_queue: None,
        diagnostic_sink: Some(diagnostic_sink.clone()),
        trace: None,
    };

    let messages = opi_agent::agent_loop(
        context,
        AgentLoopConfig::default(),
        &hooks,
        Box::new({
            let start_args = start_args.clone();
            let end_errors = end_errors.clone();
            move |event| match event {
                AgentEvent::ToolExecutionStart { args, .. } => {
                    start_args.lock().unwrap().push(args);
                }
                AgentEvent::ToolExecutionEnd { is_error, .. } => {
                    end_errors.lock().unwrap().push(is_error);
                }
                _ => {}
            }
        }),
        CancellationToken::new(),
    )
    .await
    .expect("malformed tool arguments are a normal runtime outcome");

    assert_eq!(*call_count.lock().unwrap(), 2);
    assert!(!*executed.lock().unwrap());
    assert!(!*before_called.lock().unwrap());
    assert_eq!(start_args.lock().unwrap().as_slice(), &[serde_json::Value::Null]);
    assert_eq!(end_errors.lock().unwrap().as_slice(), &[true]);
    assert!(
        diagnostic_sink
            .snapshot()
            .iter()
            .any(|d| d.code == CODE_TOOL_VALIDATION_FAILED)
    );

    let error_result = messages
        .iter()
        .find_map(|m| match m {
            AgentMessage::Llm(Message::ToolResult(trm)) if trm.tool_call_id == "call-1" => {
                Some(trm.clone())
            }
            _ => None,
        })
        .expect("malformed arguments produce a persisted tool result");
    assert!(error_result.is_error);
    assert!(
        error_result
            .content
            .iter()
            .any(|c| matches!(c, OutputContent::Text { text } if text.contains("tool arguments were not valid JSON")))
    );
}
```

- [ ] **Step 2: Run the new test and confirm it fails before the implementation**

Run:

```powershell
cargo test -p opi-agent --test tool_validation phase8_malformed_tool_arguments_do_not_execute_permissive_tool
```

Expected before implementation: fail because `executed` becomes `true` and the start args are `{}` instead of `null`.

- [ ] **Step 3: Add parse-failure helpers in `agent_loop.rs`**

Add imports:

```rust
use opi_ai::message::{AssistantContent, InputContent, Message, ToolCall, ToolResultMessage, UserMessage};
```

Add private helpers near `execute_tool`:

```rust
#[derive(Clone)]
struct ParsedToolCall {
    tool_call: ToolCall,
    args_for_event: serde_json::Value,
    parsed_args: Result<serde_json::Value, String>,
}

fn parse_tool_call_arguments(tool_call: ToolCall) -> ParsedToolCall {
    match serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
        Ok(args) => ParsedToolCall {
            tool_call,
            args_for_event: args.clone(),
            parsed_args: Ok(args),
        },
        Err(err) => ParsedToolCall {
            tool_call,
            args_for_event: serde_json::Value::Null,
            parsed_args: Err(err.to_string()),
        },
    }
}

fn malformed_tool_arguments_result(
    tool_name: &str,
    parse_error: &str,
    sink: &Option<Arc<dyn DiagnosticSink>>,
    trace: &Option<Arc<TraceCollector>>,
    turn_id: &str,
) -> ToolResult {
    trace_tool(trace, TraceKind::ToolCallStarted, tool_name, turn_id);
    observe(
        sink,
        trace,
        tool_diagnostic(
            CODE_TOOL_VALIDATION_FAILED,
            tool_name,
            "tool arguments were not valid JSON",
        ),
    );
    trace_tool(trace, TraceKind::ToolCallFailed, tool_name, turn_id);
    ToolResult {
        content: vec![opi_ai::message::OutputContent::Text {
            text: format!("tool arguments were not valid JSON: {parse_error}"),
        }],
        details: None,
        is_error: true,
        terminate: false,
    }
}
```

- [ ] **Step 4: Replace sequential `unwrap_or(json!({}))`**

Replace the sequential parse block in `agent_loop.rs` with this shape:

```rust
for tc in &tool_calls {
    let parsed = parse_tool_call_arguments(tc.clone());

    events(AgentEvent::ToolExecutionStart {
        tool_call_id: parsed.tool_call.id.clone(),
        tool_name: parsed.tool_call.name.clone(),
        args: parsed.args_for_event.clone(),
    });

    let result = match parsed.parsed_args {
        Ok(args) => {
            execute_tool(
                &parsed.tool_call.id,
                &parsed.tool_call.name,
                &args,
                &tools_map,
                hooks,
                &messages,
                cancel.clone(),
                &diagnostic_sink,
                &trace,
                &turn_id,
            )
            .await
        }
        Err(parse_error) => malformed_tool_arguments_result(
            &parsed.tool_call.name,
            &parse_error,
            &diagnostic_sink,
            &trace,
            &turn_id,
        ),
    };

    let is_error = result.is_error;
    let details = result.details.clone();
    terminate_flags.push(result.terminate);
    events(AgentEvent::ToolExecutionEnd {
        tool_call_id: parsed.tool_call.id.clone(),
        tool_name: parsed.tool_call.name.clone(),
        result: serde_json::json!(&result.content),
        details,
        is_error,
    });

    let trm = ToolResultMessage {
        tool_call_id: parsed.tool_call.id,
        tool_name: parsed.tool_call.name,
        content: result.content,
        details: result.details,
        is_error,
        timestamp_ms: 0,
    };
    tool_results.push(trm.clone());
    messages.push(AgentMessage::Llm(Message::ToolResult(trm)));
}
```

- [ ] **Step 5: Replace parallel `unwrap_or(json!({}))`**

Replace the `tc_args` construction with parsed calls:

```rust
let parsed_calls: Vec<_> = tool_calls
    .iter()
    .map(|tc| {
        let parsed = parse_tool_call_arguments(tc.clone());
        events(AgentEvent::ToolExecutionStart {
            tool_call_id: parsed.tool_call.id.clone(),
            tool_name: parsed.tool_call.name.clone(),
            args: parsed.args_for_event.clone(),
        });
        parsed
    })
    .collect();
```

Replace the parallel futures mapping with this shape:

```rust
let futures: Vec<_> = parsed_calls
    .iter()
    .map(|parsed| {
        let tools_map = &tools_map;
        let messages = &messages;
        let cancel = cancel.clone();
        let diagnostic_sink = diagnostic_sink.clone();
        let trace = trace.clone();
        let turn_id = turn_id.clone();
        async move {
            match parsed.parsed_args.clone() {
                Ok(args) => {
                    execute_tool(
                        &parsed.tool_call.id,
                        &parsed.tool_call.name,
                        &args,
                        tools_map,
                        hooks,
                        messages,
                        cancel,
                        &diagnostic_sink,
                        &trace,
                        &turn_id,
                    )
                    .await
                }
                Err(parse_error) => malformed_tool_arguments_result(
                    &parsed.tool_call.name,
                    &parse_error,
                    &diagnostic_sink,
                    &trace,
                    &turn_id,
                ),
            }
        }
    })
    .collect();
```

Use `parsed_calls.iter().zip(results.into_iter())` when emitting `ToolExecutionEnd` and persisting `ToolResultMessage`, replacing references to `tc_args`.

- [ ] **Step 6: Add trace coverage for malformed arguments**

In `crates/opi-agent/tests/trace_envelope.rs`, add a permissive tool if one is not already present in that file:

```rust
struct PermissiveTool;

impl Tool for PermissiveTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "permissive".into(),
            description: "accepts any object".into(),
            input_schema: serde_json::json!({ "type": "object" }),
        }
    }

    fn execute(
        &self,
        _call_id: &str,
        _arguments: serde_json::Value,
        _signal: tokio_util::sync::CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        Box::pin(async {
            Ok(ToolResult {
                content: vec![opi_ai::message::OutputContent::Text {
                    text: "unexpected".into(),
                }],
                details: None,
                is_error: false,
                terminate: false,
            })
        })
    }
}
```

Add the test:

```rust
#[tokio::test]
async fn phase8_runtime_contract_failure_trace_malformed_tool_arguments() {
    let provider = MockProvider::new(
        "mock",
        vec![
            test_support::tool_call_response("tc-1", "permissive", "{not-json"),
            test_support::text_response("done"),
        ],
    );
    let diag = Arc::new(RecordingSink::new());
    let trace_sink = Arc::new(RecordingTraceSink::new());
    let trace = collector(trace_sink.clone(), diag.clone());

    let result = agent_loop(
        ctx(provider, diag.clone(), Some(trace), vec![Box::new(PermissiveTool)]),
        config(),
        &NoopHooks,
        null_event_sink(),
        tokio_util::sync::CancellationToken::new(),
    )
    .await;
    assert!(result.is_ok(), "{:?}", result.err());

    let kinds = kinds_of(&trace_sink);
    assert!(kinds.contains(&TraceKind::ToolCallStarted));
    assert!(kinds.contains(&TraceKind::ToolCallFailed));
    assert!(!kinds.contains(&TraceKind::ToolCallCompleted));
    assert!(
        trace_sink
            .snapshot()
            .iter()
            .any(|r| r.kind == TraceKind::DiagnosticLinked
                && r.diagnostic_code == Some(CODE_TOOL_VALIDATION_FAILED))
    );
    assert!(
        diag.snapshot()
            .iter()
            .any(|d| d.code == CODE_TOOL_VALIDATION_FAILED)
    );
}
```

- [ ] **Step 7: Run targeted verification**

Run:

```powershell
cargo test -p opi-agent --test tool_validation phase8_malformed_tool_arguments_do_not_execute_permissive_tool
cargo test -p opi-agent --test tool_validation phase8_tool_validation_failure_contract
cargo test -p opi-agent --test trace_envelope phase8_runtime_contract_failure_trace_malformed_tool_arguments
```

Expected after implementation: all three commands pass.

---

### Task 2: Make API Surface Classification Exhaustive For Crate-Root Re-Exports

**Files:**
- Modify: `crates/opi-agent/README.md`
- Modify: `crates/opi-agent/README.zh.md`
- Test: `crates/opi-coding-agent/tests/runtime_contract_docs.rs`

**Interfaces:**
- Consumes: crate-root `pub use` names from `crates/opi-agent/src/lib.rs`.
- Produces: documentation and guard test coverage requiring each crate-root re-exported name to be classified or grouped in the API classification section.

- [ ] **Step 1: Expand EN API classification rows**

In `crates/opi-agent/README.md`, replace the current API surface table rows with this classification:

```markdown
| Surface | Tier | Notes |
|---|---|---|
| `Agent` | supported 0.x | Stateful loop wrapper; contract-tested. |
| `agent_loop` | supported 0.x | Core async entry point; runtime event-order contract tested. |
| `AgentHooks` | supported 0.x | Six lifecycle hooks; hook-order and failure contract tested. |
| `AgentLoopConfig`, `AgentLoopContext`, `AgentError`, `AgentMessage` | supported 0.x | Required by the supported low-level `agent_loop` entry point. |
| `Tool`, `ToolDef`, `ToolResult`, `ToolError`, `ExecutionMode` | supported 0.x | JSON-Schema tool contract plus result/error/scheduling types used by embedders. |
| `AgentEvent`, `AgentEventSink` | supported 0.x | In-process runtime event stream; `AgentEvent` is `#[non_exhaustive]` because new variants may arrive across 0.x. |
| `AgentSessionEvent` | unstable internal | `opi --json` wire protocol (`NDJSON_SCHEMA_VERSION = 2`, owned by `opi-coding-agent`); `#[non_exhaustive]`. Check the schema version. |
| `SessionEntry` | unstable internal | Session JSONL storage layout; lives at `session::SessionEntry`, not re-exported at the crate root; `#[non_exhaustive]`. |
| `Extension`, `ExtensionCommand`, `ExtensionError`, `ExtensionHookResult`, `ExtensionRegistry` | unstable internal | Extension lifecycle and composition surface; the `extension` module marks it `# Unstable`. |
| `SdkCommand`, `SdkResponse`, `SDK_SCHEMA_VERSION` | unstable internal | RPC/SDK command model (`SDK_SCHEMA_VERSION = 3`); the `sdk` module marks it unstable 0.x. |
| `StreamingProxy`, `ProxyConfig`, `ProxyEvent`, `ProxyHandler`, `SecretRedactor`, `StreamingProxyError` | unstable internal | Streaming-proxy primitives; the `streaming_proxy` module marks them unstable 0.x. |
| `Diagnostic`, `DiagnosticPayload`, `RedactionMode`, `Severity`, `redact`, `redact_text`, `DiagnosticSink`, `NullSink`, `RecordingSink` | unstable internal | Diagnostic payload and sink plumbing used by runtime surfaces; current contract is redaction/schema-version behavior, not a stable API shape. |
| `FileTraceSink`, `RecordingTraceSink`, `TRACE_SCHEMA_VERSION`, `TraceCollector`, `TraceError`, `TraceKind`, `TraceRecord`, `TraceSink` | unstable internal | Local trace envelope plumbing; the `trace` module marks it unstable 0.x and carries `TRACE_SCHEMA_VERSION = 1`. |
| `AgentState` | unstable internal | Runtime state holder exposed for crate layout and harness integration; not a supported embedder contract. |
```

Replace the paragraph after the table with:

```markdown
This review found no candidate-removal crate-root re-exports. Every crate-root
`pub use` in `src/lib.rs` is named in the table above. Public modules may expose
additional items through module paths; unless those items are named as supported
0.x surfaces here, they are unstable internal 0.x APIs.
```

- [ ] **Step 2: Mirror the classification in ZH README**

Apply the same surface names and tiers to `crates/opi-agent/README.zh.md`. Use the existing Chinese tier labels already present in that file:

```markdown
| `AgentLoopConfig`、`AgentLoopContext`、`AgentError`、`AgentMessage` | 支持的 0.x | 受支持的低层 `agent_loop` 入口需要这些类型。 |
| `Tool`、`ToolDef`、`ToolResult`、`ToolError`、`ExecutionMode` | 支持的 0.x | JSON-Schema 工具契约，以及嵌入方使用的结果、错误和调度类型。 |
| `AgentEvent`、`AgentEventSink` | 支持的 0.x | 进程内运行时事件流；`AgentEvent` 是 `#[non_exhaustive]`，0.x 内可能新增变体。 |
| `Diagnostic`、`DiagnosticPayload`、`RedactionMode`、`Severity`、`redact`、`redact_text`、`DiagnosticSink`、`NullSink`、`RecordingSink` | 不稳定内部 | 运行时表面使用的诊断 payload 和 sink plumbing；当前契约是 redaction/schema-version 行为，不是稳定 API 形状。 |
| `FileTraceSink`、`RecordingTraceSink`、`TRACE_SCHEMA_VERSION`、`TraceCollector`、`TraceError`、`TraceKind`、`TraceRecord`、`TraceSink` | 不稳定内部 | 本地 trace envelope plumbing；`trace` 模块标注为不稳定 0.x，并携带 `TRACE_SCHEMA_VERSION = 1`。 |
| `AgentState` | 不稳定内部 | 为 crate 布局和 harness 集成暴露的运行时状态持有器；不是受支持的嵌入方契约。 |
```

Keep the original ZH rows for `Agent`, `agent_loop`, `AgentHooks`, `AgentSessionEvent`, `SessionEntry`, extension, SDK, and streaming proxy, but expand grouped rows so every EN surface name appears in backticks in the ZH section too.

- [ ] **Step 3: Add exhaustive re-export guard helpers**

In `crates/opi-coding-agent/tests/runtime_contract_docs.rs`, add imports:

```rust
use std::collections::BTreeSet;
```

Add helpers near the existing doc helpers:

```rust
fn section_between<'a>(content: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = content
        .find(start)
        .unwrap_or_else(|| panic!("missing section start {start}"));
    let after_start = &content[start_index..];
    let end_index = after_start
        .find(end)
        .unwrap_or_else(|| panic!("missing section end {end}"));
    &after_start[..end_index]
}

fn backticked_names(section: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut remaining = section;
    while let Some(start) = remaining.find('`') {
        let after_start = &remaining[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let name = &after_start[..end];
        if name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            names.insert(name.to_string());
        }
        remaining = &after_start[end + 1..];
    }
    names
}

fn crate_root_reexport_names(lib: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut statement = String::new();
    let mut collecting = false;

    for line in lib.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub use ") {
            statement.clear();
            statement.push_str(trimmed);
            collecting = !trimmed.ends_with(';');
        } else if collecting {
            statement.push(' ');
            statement.push_str(trimmed);
            collecting = !trimmed.ends_with(';');
        } else {
            continue;
        }

        if !collecting {
            let rest = statement
                .trim_start_matches("pub use ")
                .trim_end_matches(';')
                .trim();
            if let Some((_, grouped)) = rest.split_once("::{") {
                let grouped = grouped.trim_end_matches('}');
                for name in grouped.split(',') {
                    let name = name.trim();
                    if !name.is_empty() {
                        names.insert(name.to_string());
                    }
                }
            } else {
                let name = rest
                    .rsplit("::")
                    .next()
                    .expect("simple pub use has a final segment")
                    .trim();
                names.insert(name.to_string());
            }
        }
    }

    names
}
```

- [ ] **Step 4: Assert EN/ZH classification exhaustiveness**

Inside `phase8_api_surface_classification`, after `let lib = read_repo_file("crates/opi-agent/src/lib.rs");`, add:

```rust
let expected_reexports = crate_root_reexport_names(&lib);
let en_api = section_between(&en, "## API Surface Classification", "## Non-Goals");
let zh_api = section_between(&zh, "## API Surface Classification", "## Non-Goals");
let en_names = backticked_names(en_api);
let zh_names = backticked_names(zh_api);

let missing_en: Vec<_> = expected_reexports.difference(&en_names).cloned().collect();
let missing_zh: Vec<_> = expected_reexports.difference(&zh_names).cloned().collect();
assert!(
    missing_en.is_empty(),
    "EN API Surface Classification missing crate-root re-exports: {missing_en:?}"
);
assert!(
    missing_zh.is_empty(),
    "ZH API Surface Classification missing crate-root re-exports: {missing_zh:?}"
);
```

Keep the existing exact-line re-export pins for the named high-value surfaces.

- [ ] **Step 5: Run targeted verification**

Run:

```powershell
cargo test -p opi-coding-agent --test runtime_contract_docs phase8_api_surface_classification
```

Expected after implementation: pass. To verify the guard is meaningful, temporarily add a local, uncommitted `pub use validation::ValidationError;` line to `crates/opi-agent/src/lib.rs`, run the same command and confirm it fails, then remove that temporary line before continuing.

---

### Task 3: Pin Provider-Stream Cancellation Open-Turn Semantics

**Files:**
- Modify: `crates/opi-agent/tests/trace_envelope.rs`
- Modify: `crates/opi-agent/README.md`
- Modify: `crates/opi-agent/README.zh.md`

**Interfaces:**
- Consumes: existing cancellation behavior in `agent_loop.rs`.
- Produces: explicit contract that provider failure and provider-stream cancellation may leave `TurnStarted` without `TurnEnded`, while still emitting `AgentEnd`/`RunEnded` and a cancellation diagnostic.

- [ ] **Step 1: Add trace test for mid-stream cancellation**

In `crates/opi-agent/tests/trace_envelope.rs`, add a hanging provider equivalent to the existing agent-loop semantics test if the file does not already have one:

```rust
struct HangingStreamProvider;

impl Provider for HangingStreamProvider {
    fn id(&self) -> &str {
        "hanging"
    }

    fn models(&self) -> &[opi_ai::provider::ModelInfo] {
        &[]
    }

    fn stream(&self, _request: Request) -> EventStream {
        let mut partial = base_msg();
        partial.content.push(AssistantContent::Text {
            text: "partial".into(),
        });
        let events: Vec<Result<AssistantStreamEvent, ProviderError>> = vec![
            Ok(AssistantStreamEvent::Start {
                partial: base_msg(),
            }),
            Ok(AssistantStreamEvent::TextDelta {
                content_index: 0,
                delta: "partial".into(),
                partial,
            }),
        ];
        Box::pin(
            futures_util::stream::iter(events)
                .chain(futures_util::stream::pending::<Result<AssistantStreamEvent, ProviderError>>()),
        )
    }
}
```

Add the test:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase8_provider_stream_cancel_trace_may_leave_turn_open() {
    let diag = Arc::new(RecordingSink::new());
    let trace_sink = Arc::new(RecordingTraceSink::new());
    let trace = collector(trace_sink.clone(), diag.clone());
    let cancel = tokio_util::sync::CancellationToken::new();
    let cancel_for_task = cancel.clone();

    let handle = tokio::spawn(async move {
        agent_loop(
            ctx(HangingStreamProvider, diag, Some(trace), vec![]),
            config(),
            &NoopHooks,
            null_event_sink(),
            cancel_for_task,
        )
        .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    cancel.cancel();

    let result = handle.await.expect("agent_loop task panicked");
    assert!(matches!(result, Err(AgentError::Cancelled)));

    let kinds = kinds_of(&trace_sink);
    assert!(kinds.contains(&TraceKind::TurnStarted));
    assert!(kinds.contains(&TraceKind::RunEnded));
    assert!(
        !kinds.contains(&TraceKind::TurnEnded),
        "provider-stream cancellation exits mid-turn; trace consumers must tolerate an open turn"
    );
    assert!(
        trace_sink
            .snapshot()
            .iter()
            .any(|r| r.kind == TraceKind::DiagnosticLinked
                && r.diagnostic_code == Some(CODE_AGENT_CANCELLED))
    );
}
```

- [ ] **Step 2: Clarify EN cancellation docs**

In `crates/opi-agent/README.md` under `## Cancellation`, add:

```markdown
Trace consumers must tolerate open turns on early provider exits. Provider
failure and provider-stream cancellation may emit `TurnStarted` without a
matching `TurnEnded`; the terminal boundary for those paths is `AgentEnd` plus
trace `RunEnded` and the linked diagnostic.
```

- [ ] **Step 3: Mirror ZH cancellation docs**

In `crates/opi-agent/README.zh.md`, add the same contract in Chinese and keep the backticked identifiers unchanged:

```markdown
Trace 消费方必须容忍早期 provider 退出留下的 open turn。Provider 失败和
provider-stream cancellation 可能发出 `TurnStarted` 但没有匹配的
`TurnEnded`；这些路径的终止边界是 `AgentEnd` 加 trace `RunEnded` 和关联诊断。
```

- [ ] **Step 4: Run targeted verification**

Run:

```powershell
cargo test -p opi-agent --test trace_envelope phase8_provider_stream_cancel_trace_may_leave_turn_open
```

Expected after implementation: pass.

---

### Task 4: Final Gates For This Remediation Set

**Files:**
- No additional files.

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: verified patch with no clippy warnings and no drift between EN/ZH Phase 8 runtime docs.

- [ ] **Step 1: Run all touched test targets**

Run:

```powershell
cargo test -p opi-agent --test tool_validation phase8
cargo test -p opi-agent --test trace_envelope phase8
cargo test -p opi-coding-agent --test runtime_contract_docs phase8
```

Expected: all selected Phase 8 tests pass.

- [ ] **Step 2: Run clippy**

Run:

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: exit 0.

- [ ] **Step 3: Run formatting check**

Run:

```powershell
cargo fmt --check --all
```

Expected: exit 0.

- [ ] **Step 4: Record known full-suite caveat only if observed**

Run:

```powershell
cargo test --workspace --all-targets
```

Expected if the historical `adapter_host_mock` behavior remains: real test binaries pass, but the harness-less mock adapter binary may prevent a clean all-targets exit in this environment. If that happens, record the exact output and do not describe the full suite as clean.

---

## Separate Follow-Up Issues

These are confirmed but intentionally split from the Phase 8 closure patch because they touch unrelated contracts.

1. **Redaction hardening:** Add value-pattern coverage and tests for AWS access key IDs such as `AKIAIOSFODNN7EXAMPLE`. Consider Azure opaque key patterns only if a low-false-positive pattern is chosen.
2. **Diagnostic/trace structural guard:** Replace convention-only `observe()` usage with a narrow helper or compile-time/test-time scan that makes new diagnostic emission sites visible.
3. **RPC summary shape:** Decide whether `run_summary` should become an `AgentSessionEvent` variant or remain an RPC-only additive event until the next SDK schema version.
4. **`adapter_host_mock` isolation:** Move or gate the harness-less adapter mock so `cargo test --workspace --all-targets` has a deterministic exit status while adapter integration tests can still spawn the mock process.

## Self-Review

- Spec coverage: This plan covers the confirmed Phase 8 contract gaps from both audit documents: malformed tool-call JSON, exhaustive API classification, and cancel open-turn semantics. Lower-priority residuals are explicitly split into separate issues.
- Placeholder scan: No task relies on unspecified implementation work. Every code-changing task includes concrete files, code shape, commands, and expected outcomes.
- Type consistency: `ParsedToolCall`, `parse_tool_call_arguments`, and `malformed_tool_arguments_result` are private to `agent_loop.rs`; tests reference only existing public test/runtime types.
