//! Bridge between adapter process host and the opi-agent extension system.
//!
//! [`ProcessAdapter`] wraps an [`AdapterHost`] and
//! implements the [`Extension`] trait,
//! exposing adapter capabilities (tools, commands, hooks, events, state,
//! model overrides) through the standard extension contracts.
//!
//! [`ProcessAdapterTool`] wraps an individual tool capability advertised by the
//! adapter and implements the [`Tool`] trait, delegating
//! execution to the adapter host with cancellation bridging.
//!
//! # Bridge Mappings
//!
//! | Adapter Protocol | Extension System |
//! |-----------------|-----------------|
//! | `AdapterToolCapability` | `Extension::tools()` → `Tool::definition()` |
//! | `AdapterCommandCapability` | `Extension::on_command()` |
//! | `AdapterModelOverride` | `Extension::model_overrides()` |
//! | `Hook "before_tool_call"` | `Extension::on_before_tool_call()` |
//! | `Hook "after_tool_call"` | `Extension::on_after_tool_call()` |
//! | Hook `"event"` | `Extension::on_event()` |
//! | `StateSerialize`/`StateRestore` | `Extension::serialize_state()`/`restore_state()` |
//! | Tool call cancellation | `Tool::execute()` → `tokio::select!` with cancel signal |
//!
//! # Hook Filtering
//!
//! Hooks are only dispatched to adapters that declared them in their
//! `capabilities.hooks` list. If an adapter does not declare
//! `"before_tool_call"`, its `on_before_tool_call` implementation returns
//! `Continue` without sending a message to the adapter process.
//!
//! # Failure Semantics
//!
//! - Tool call timeout → `ToolError::ExecutionFailed`
//! - Hook timeout on `before_tool_call` → `ExtensionHookResult::Block`
//!   (fail closed)
//! - Hook timeout on `after_tool_call` → continue (fail open)
//! - Event delivery under backpressure → dropped silently
//! - State serialization failure → `ExtensionError::StateSerialization`
//!
//! # Unstable
//!
//! This module is part of the **unstable 0.x extension API**. Breaking changes
//! may occur between minor versions without a major version bump.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use opi_agent::event::AgentEvent;
use opi_agent::extension::{Extension, ExtensionCommand, ExtensionError, ExtensionHookResult};
use opi_agent::tool::{ExecutionMode, Tool, ToolError, ToolResult};
use opi_ai::message::{OutputContent, ToolDef};
use opi_ai::provider::ModelInfo;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::adapter_host::{AdapterCapabilities, AdapterHost};
use crate::adapter_protocol::AdapterHostMessage;

/// Default timeout for adapter tool calls and command dispatch.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// ProcessAdapterTool
// ---------------------------------------------------------------------------

/// A tool backed by an adapter process.
///
/// Implements [`Tool`] by delegating `execute()` to the adapter host via the
/// JSONL protocol. Cancellation is bridged through `tokio::select!`.
pub struct ProcessAdapterTool {
    name: String,
    description: String,
    schema: Value,
    host: Arc<AdapterHost>,
}

impl std::fmt::Debug for ProcessAdapterTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessAdapterTool")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl ProcessAdapterTool {
    fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        schema: Value,
        host: Arc<AdapterHost>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            schema,
            host,
        }
    }
}

impl Tool for ProcessAdapterTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.schema.clone(),
        }
    }

    fn execute(
        &self,
        call_id: &str,
        arguments: Value,
        signal: CancellationToken,
        _on_update: Option<opi_agent::tool::UpdateCallback>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult, ToolError>> + Send>>
    {
        let id = format!("{}_{}", call_id, self.host.next_id());
        let host = self.host.clone();
        let tool_name = self.name.clone();

        Box::pin(async move {
            let request = AdapterHostMessage::ToolCall {
                id: id.clone(),
                tool: tool_name,
                args: arguments,
            };

            let result = tokio::select! {
                result = host.send_request(request, REQUEST_TIMEOUT) => result,
                _ = signal.cancelled() => {
                    // Best-effort cancel
                    let _ = host.cancel(&id, "tool_cancelled").await;
                    return Err(ToolError::Cancelled);
                }
            };

            match result {
                Ok(msg) => match msg {
                    crate::adapter_protocol::AdapterProcessMessage::ToolResult {
                        content,
                        is_error,
                        ..
                    } => {
                        let output_content: Vec<OutputContent> = content
                            .into_iter()
                            .filter_map(|c| {
                                if c["type"].as_str() == Some("text") {
                                    Some(OutputContent::Text {
                                        text: c["text"].as_str().unwrap_or_default().to_string(),
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect();

                        Ok(ToolResult {
                            content: output_content,
                            details: None,
                            is_error,
                            terminate: false,
                        })
                    }
                    crate::adapter_protocol::AdapterProcessMessage::Error { message, .. } => {
                        Err(ToolError::ExecutionFailed(message))
                    }
                    other => Err(ToolError::ExecutionFailed(format!(
                        "unexpected response type for tool call: {:?}",
                        other_type(&other)
                    ))),
                },
                Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
            }
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sequential
    }
}

// ---------------------------------------------------------------------------
// ProcessAdapter
// ---------------------------------------------------------------------------

/// Bridge between an adapter process host and the opi-agent extension system.
///
/// Wraps an [`AdapterHost`] and exposes adapter capabilities (tools, commands,
/// hooks, events, state, model overrides) through the [`Extension`] trait.
///
/// # Construction
///
/// Created via [`ProcessAdapter::from_host`] after the adapter host has
/// completed the initialize/capabilities handshake.
pub struct ProcessAdapter {
    name: String,
    host: Arc<AdapterHost>,
    tools_defs: Vec<crate::adapter_protocol::AdapterToolCapability>,
    commands: BTreeSet<String>,
    hooks: BTreeSet<String>,
    model_overrides: Vec<crate::adapter_protocol::AdapterModelOverride>,
}

impl std::fmt::Debug for ProcessAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessAdapter")
            .field("name", &self.name)
            .field("tools", &self.tools_defs.len())
            .field("commands", &self.commands.len())
            .field("hooks", &self.hooks)
            .finish_non_exhaustive()
    }
}

impl ProcessAdapter {
    /// Create a `ProcessAdapter` from a running adapter host and its
    /// capabilities.
    ///
    /// The host must have already completed the initialize/capabilities
    /// handshake successfully.
    pub fn from_host(
        name: impl Into<String>,
        host: Arc<AdapterHost>,
        capabilities: AdapterCapabilities,
    ) -> Self {
        let tools_defs = capabilities.tools.clone();
        let commands = capabilities
            .commands
            .iter()
            .map(|c| c.name.clone())
            .collect();
        let hooks = capabilities.hooks.iter().cloned().collect();
        let model_overrides = capabilities.model_overrides.clone();

        Self {
            name: name.into(),
            host,
            tools_defs,
            commands,
            hooks,
            model_overrides,
        }
    }
}

impl Extension for ProcessAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn tools(&self) -> Vec<Box<dyn Tool>> {
        self.tools_defs
            .iter()
            .map(|cap| {
                Box::new(ProcessAdapterTool::new(
                    &cap.name,
                    &cap.description,
                    cap.input_schema.clone(),
                    self.host.clone(),
                )) as Box<dyn Tool>
            })
            .collect()
    }

    /// Note: The current implementation maps `AdapterModelOverride.model` as
    /// both the provider_id and the model id. This is a simplification for the
    /// Phase 5 MVP where adapter model overrides are not yet exercised. A
    /// future iteration should resolve the provider from the model spec.
    fn model_overrides(&self) -> Vec<(String, ModelInfo)> {
        self.model_overrides
            .iter()
            .map(|mo| {
                (
                    mo.model.clone(),
                    ModelInfo {
                        id: mo.model.clone(),
                        display_name: mo.model.clone(),
                        context_window: 128_000,
                        max_output_tokens: 16_384,
                        supports_images: false,
                        supports_streaming: true,
                        supports_thinking: false,
                    },
                )
            })
            .collect()
    }

    fn on_before_tool_call(
        &self,
        tool_name: &str,
        args: &Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ExtensionHookResult> + Send>> {
        if !self.hooks.contains("before_tool_call") {
            return Box::pin(async { ExtensionHookResult::Continue });
        }

        let id = self.host.next_id();
        let host = self.host.clone();
        let tool = tool_name.to_string();
        let args = args.clone();

        Box::pin(async move {
            let request = AdapterHostMessage::Hook {
                id,
                hook: "before_tool_call".to_string(),
                payload: serde_json::json!({
                    "tool": tool,
                    "args": args,
                }),
            };

            // Fail closed: timeout → block
            match host.send_request(request, REQUEST_TIMEOUT).await {
                Ok(msg) => match msg {
                    crate::adapter_protocol::AdapterProcessMessage::HookResult {
                        action,
                        data,
                        ..
                    } => {
                        if action == "block" {
                            let reason = data
                                .and_then(|d| d["reason"].as_str().map(String::from))
                                .unwrap_or_else(|| "blocked by adapter".to_string());
                            ExtensionHookResult::Block { reason }
                        } else {
                            ExtensionHookResult::Continue
                        }
                    }
                    crate::adapter_protocol::AdapterProcessMessage::Error { message, .. } => {
                        // Adapter error during hook → fail closed
                        ExtensionHookResult::Block {
                            reason: format!("adapter hook error: {message}"),
                        }
                    }
                    other => ExtensionHookResult::Block {
                        reason: format!("unexpected hook response: {:?}", other_type(&other)),
                    },
                },
                Err(e) => {
                    // Timeout or unavailable → fail closed
                    ExtensionHookResult::Block {
                        reason: format!("adapter before_tool_call failed: {e}"),
                    }
                }
            }
        })
    }

    fn on_after_tool_call(
        &self,
        tool_name: &str,
        result: &ToolResult,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        if !self.hooks.contains("after_tool_call") {
            return Box::pin(async {});
        }

        let id = self.host.next_id();
        let host = self.host.clone();
        let tool = tool_name.to_string();
        let is_error = result.is_error;
        let content_summary: Vec<String> = result
            .content
            .iter()
            .map(|c| match c {
                opi_ai::message::OutputContent::Text { text } => text.clone(),
                _ => "[content]".to_string(),
            })
            .collect();

        Box::pin(async move {
            let request = AdapterHostMessage::Hook {
                id,
                hook: "after_tool_call".to_string(),
                payload: serde_json::json!({
                    "tool": tool,
                    "is_error": is_error,
                    "content": content_summary,
                }),
            };

            // Fail open: ignore timeout/errors for after hooks
            let _ = host.send_request(request, REQUEST_TIMEOUT).await;
        })
    }

    fn on_event(&self, event: &AgentEvent) {
        if !self.hooks.contains("event") {
            return;
        }

        // Fire-and-forget: serialize and drop under backpressure
        let event_json = match serde_json::to_value(event) {
            Ok(v) => v,
            Err(_) => return,
        };

        // Since Extension::on_event is sync, try to spawn an async task
        // for fire-and-forget event delivery. If no runtime is available,
        // the event is dropped (consistent with backpressure semantics).
        let host = self.host.clone();
        let _ = tokio::runtime::Handle::try_current().map(|handle| {
            handle.spawn(async move {
                host.send_event(event_json).await;
            });
        });
    }

    fn on_command(
        &self,
        command: &ExtensionCommand,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<Value>, ExtensionError>> + Send>,
    > {
        if !self.commands.contains(&command.name) {
            return Box::pin(async { Ok(None) });
        }

        let id = self.host.next_id();
        let host = self.host.clone();
        let name = command.name.clone();
        let args = command.args.clone();

        Box::pin(async move {
            let request = AdapterHostMessage::Command { id, name, args };

            match host.send_request(request, REQUEST_TIMEOUT).await {
                Ok(msg) => match msg {
                    crate::adapter_protocol::AdapterProcessMessage::CommandResult {
                        data, ..
                    } => Ok(Some(data)),
                    crate::adapter_protocol::AdapterProcessMessage::Error { message, .. } => {
                        Err(ExtensionError::CommandError(message))
                    }
                    other => Err(ExtensionError::CommandError(format!(
                        "unexpected command response: {:?}",
                        other_type(&other)
                    ))),
                },
                Err(e) => Err(ExtensionError::CommandError(e.to_string())),
            }
        })
    }

    /// Serialize adapter state for session persistence.
    ///
    /// **Runtime requirement:** This method uses `tokio::task::block_in_place`
    /// internally and requires a multi-threaded Tokio runtime. Calling from a
    /// current-thread runtime will panic. The opi binary uses a multi-threaded
    /// runtime by default.
    fn serialize_state(&self) -> Result<Option<Value>, ExtensionError> {
        // State serialization needs to be synchronous but the host is async.
        // Use tokio::runtime::Handle to block_on.
        let id = self.host.next_id();
        let host = self.host.clone();

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                host.send_request(AdapterHostMessage::StateSerialize { id }, REQUEST_TIMEOUT)
                    .await
            })
        });

        match result {
            Ok(msg) => match msg {
                crate::adapter_protocol::AdapterProcessMessage::StateResult { state, .. } => {
                    Ok(Some(state))
                }
                crate::adapter_protocol::AdapterProcessMessage::Error { message, .. } => {
                    Err(ExtensionError::StateSerialization {
                        name: self.name.clone(),
                        reason: message,
                    })
                }
                other => Err(ExtensionError::StateSerialization {
                    name: self.name.clone(),
                    reason: format!("unexpected state response: {:?}", other_type(&other)),
                }),
            },
            Err(e) => Err(ExtensionError::StateSerialization {
                name: self.name.clone(),
                reason: e.to_string(),
            }),
        }
    }

    /// Restore adapter state from session persistence.
    ///
    /// **Runtime requirement:** Same as [`serialize_state`](Self::serialize_state) —
    /// requires a multi-threaded Tokio runtime.
    fn restore_state(&self, state: Value) -> Result<(), ExtensionError> {
        let id = self.host.next_id();
        let host = self.host.clone();

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                host.send_request(
                    AdapterHostMessage::StateRestore { id, state },
                    REQUEST_TIMEOUT,
                )
                .await
            })
        });

        match result {
            Ok(msg) => match msg {
                crate::adapter_protocol::AdapterProcessMessage::StateResult { .. } => Ok(()),
                crate::adapter_protocol::AdapterProcessMessage::Error { message, .. } => {
                    Err(ExtensionError::StateRestoration {
                        name: self.name.clone(),
                        reason: message,
                    })
                }
                other => Err(ExtensionError::StateRestoration {
                    name: self.name.clone(),
                    reason: format!("unexpected state response: {:?}", other_type(&other)),
                }),
            },
            Err(e) => Err(ExtensionError::StateRestoration {
                name: self.name.clone(),
                reason: e.to_string(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get the type name of an AdapterProcessMessage for error messages.
fn other_type(msg: &crate::adapter_protocol::AdapterProcessMessage) -> &'static str {
    match msg {
        crate::adapter_protocol::AdapterProcessMessage::Capabilities { .. } => "capabilities",
        crate::adapter_protocol::AdapterProcessMessage::ToolResult { .. } => "tool_result",
        crate::adapter_protocol::AdapterProcessMessage::CommandResult { .. } => "command_result",
        crate::adapter_protocol::AdapterProcessMessage::HookResult { .. } => "hook_result",
        crate::adapter_protocol::AdapterProcessMessage::StateResult { .. } => "state_result",
        crate::adapter_protocol::AdapterProcessMessage::Error { .. } => "error",
    }
}
