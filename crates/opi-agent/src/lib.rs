//! General-purpose agent runtime with tool calling and transport abstraction.
//!
//! Provides the foundation for building specialized agents with pluggable
//! tool systems and communication transports.

pub mod agent;
pub mod event;
pub mod hooks;
pub mod loop_types;
pub mod message;
pub mod session;
pub mod session_event;
pub mod state;
pub mod tool;
pub mod transport;
pub mod validation;

pub use agent::Agent;
pub use event::{AgentEvent, AgentEventSink};
pub use hooks::AgentHooks;
pub use loop_types::{AgentError, AgentLoopConfig, AgentLoopContext};
pub use message::AgentMessage;
pub use session_event::AgentSessionEvent;
pub use state::AgentState;
pub use tool::{ExecutionMode, Tool, ToolError, ToolResult};
pub use transport::Transport;

// Re-export provider-facing types needed at the agent boundary.
pub use opi_ai::message::ToolDef;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use hooks::{
    AfterToolCallContext, AfterToolCallResult, BeforeToolCallContext, BeforeToolCallResult,
    ShouldStopAfterTurnContext,
};
use opi_ai::message::{AssistantContent, InputContent, Message, ToolResultMessage, UserMessage};
use opi_ai::provider::Request;
use serde_json::json;
use tokio_util::sync::CancellationToken;

/// Run the agent loop until completion or cancellation.
///
/// The loop iterates: provider request → stream response → detect tool calls
/// → validate and execute tools → send tool results back → repeat until no
/// tool calls or stop condition.
pub async fn agent_loop(
    context: AgentLoopContext,
    config: AgentLoopConfig,
    hooks: &dyn AgentHooks,
    events: AgentEventSink,
    cancel: CancellationToken,
) -> Result<Vec<AgentMessage>, AgentError> {
    let tools_map: HashMap<String, &dyn Tool> = context
        .tools
        .iter()
        .map(|t| (t.definition().name.clone(), t.as_ref()))
        .collect();
    let tool_defs: Vec<_> = context.tools.iter().map(|t| t.definition()).collect();

    let mut messages = context.messages;

    events(AgentEvent::AgentStart);

    let mut has_tools_pending;
    for turn_idx in 0..config.max_turns {
        if cancel.is_cancelled() {
            events(AgentEvent::AgentEnd {
                messages: messages.clone(),
            });
            return Err(AgentError::Cancelled);
        }

        events(AgentEvent::TurnStart);

        // H5: transform context before provider call
        let transformed = hooks
            .transform_context(messages.clone(), cancel.clone())
            .await?;

        // Convert messages for the provider
        let llm_messages = hooks.convert_to_llm(&transformed)?;

        // Build the provider request
        let request = Request {
            model: context.model.clone(),
            system: context.system.clone(),
            messages: llm_messages,
            tools: tool_defs.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            thinking: Default::default(),
            stop_sequences: vec![],
            metadata: None,
            cancel: cancel.clone(),
        };

        // Stream the response
        let mut stream = context.provider.stream(request);
        let mut assistant_content: Vec<AssistantContent> = Vec::new();
        has_tools_pending = false;

        while let Some(item) = {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    events(AgentEvent::AgentEnd {
                        messages: messages.clone(),
                    });
                    return Err(AgentError::Cancelled);
                }
                item = stream.next() => item,
            }
        } {
            match item {
                Ok(event) => {
                    if let Some(msg) = process_stream_event(&event, &mut assistant_content, &events)
                    {
                        // Build the assistant message from accumulated content
                        let mut assistant_msg = msg;
                        assistant_msg.content = assistant_content.clone();
                        let agent_msg = AgentMessage::Llm(Message::Assistant(assistant_msg));

                        events(AgentEvent::MessageEnd {
                            message: agent_msg.clone(),
                        });

                        messages.push(agent_msg.clone());

                        // Check for tool calls
                        let tool_calls: Vec<_> = assistant_content
                            .iter()
                            .filter_map(|c| match c {
                                AssistantContent::ToolCall { tool_call } => Some(tool_call.clone()),
                                _ => None,
                            })
                            .collect();

                        if !tool_calls.is_empty() {
                            has_tools_pending = true;
                            let mut tool_results = Vec::new();
                            let mut terminate_flags = Vec::new();

                            // Determine batch execution mode (H3):
                            // parallel by default; any sequential tool forces serial
                            let batch_is_sequential = tool_calls.iter().any(|tc| {
                                tools_map
                                    .get(tc.name.as_str())
                                    .map(|t| t.execution_mode() == ExecutionMode::Sequential)
                                    .unwrap_or(true)
                            });

                            if batch_is_sequential {
                                for tc in &tool_calls {
                                    let args: serde_json::Value =
                                        serde_json::from_str(&tc.arguments).unwrap_or(json!({}));

                                    events(AgentEvent::ToolExecutionStart {
                                        tool_call_id: tc.id.clone(),
                                        tool_name: tc.name.clone(),
                                        args: args.clone(),
                                    });

                                    let result = execute_tool(
                                        &tc.id,
                                        &tc.name,
                                        &args,
                                        &tools_map,
                                        hooks,
                                        &messages,
                                        cancel.clone(),
                                    )
                                    .await;

                                    let is_error = result.is_error;
                                    terminate_flags.push(result.terminate);
                                    events(AgentEvent::ToolExecutionEnd {
                                        tool_call_id: tc.id.clone(),
                                        tool_name: tc.name.clone(),
                                        result: serde_json::json!(&result.content),
                                        is_error,
                                    });

                                    let trm = ToolResultMessage {
                                        tool_call_id: tc.id.clone(),
                                        tool_name: tc.name.clone(),
                                        content: result.content,
                                        details: result.details,
                                        is_error,
                                        timestamp_ms: 0,
                                    };
                                    tool_results.push(trm.clone());
                                    messages.push(AgentMessage::Llm(Message::ToolResult(trm)));
                                }
                            } else {
                                // Parallel execution — emit Start events before spawning
                                let tc_args: Vec<_> = tool_calls
                                    .iter()
                                    .map(|tc| {
                                        let args: serde_json::Value =
                                            serde_json::from_str(&tc.arguments)
                                                .unwrap_or(json!({}));
                                        events(AgentEvent::ToolExecutionStart {
                                            tool_call_id: tc.id.clone(),
                                            tool_name: tc.name.clone(),
                                            args: args.clone(),
                                        });
                                        (tc.clone(), args)
                                    })
                                    .collect();

                                let futures: Vec<_> = tc_args
                                    .iter()
                                    .map(|(tc, args)| {
                                        let tools_map = &tools_map;
                                        let messages = &messages;
                                        let cancel = cancel.clone();
                                        let tc_id = tc.id.clone();
                                        let tc_name = tc.name.clone();
                                        let args = args.clone();
                                        async move {
                                            let result = execute_tool(
                                                &tc_id, &tc_name, &args, tools_map, hooks,
                                                messages, cancel,
                                            )
                                            .await;
                                            (tc_id, tc_name, result)
                                        }
                                    })
                                    .collect();
                                let results = futures_util::future::join_all(futures).await;
                                for (tc_id, tc_name, result) in results {
                                    let is_error = result.is_error;
                                    terminate_flags.push(result.terminate);
                                    events(AgentEvent::ToolExecutionEnd {
                                        tool_call_id: tc_id.clone(),
                                        tool_name: tc_name.clone(),
                                        result: serde_json::json!(&result.content),
                                        is_error,
                                    });
                                    let trm = ToolResultMessage {
                                        tool_call_id: tc_id,
                                        tool_name: tc_name,
                                        content: result.content,
                                        details: result.details,
                                        is_error,
                                        timestamp_ms: 0,
                                    };
                                    tool_results.push(trm.clone());
                                    messages.push(AgentMessage::Llm(Message::ToolResult(trm)));
                                }
                            }

                            // H4: early stop if ALL results have terminate=true
                            let all_terminate =
                                !terminate_flags.is_empty() && terminate_flags.iter().all(|t| *t);

                            events(AgentEvent::TurnEnd {
                                message: agent_msg,
                                tool_results: tool_results.clone(),
                            });

                            if all_terminate {
                                events(AgentEvent::AgentEnd {
                                    messages: messages.clone(),
                                });
                                return Ok(messages);
                            }

                            // M1: pass only current turn's tool_results
                            let stop_ctx = ShouldStopAfterTurnContext {
                                messages: messages.clone(),
                                tool_results,
                            };
                            if hooks.should_stop_after_turn(stop_ctx).await {
                                events(AgentEvent::AgentEnd {
                                    messages: messages.clone(),
                                });
                                return Ok(messages);
                            }

                            // Break inner loop; outer for loop continues
                            break;
                        }

                        // No tool calls — this turn is done
                        events(AgentEvent::TurnEnd {
                            message: agent_msg.clone(),
                            tool_results: vec![],
                        });

                        // M1: no tool results for a text-only turn
                        let stop_ctx = ShouldStopAfterTurnContext {
                            messages: messages.clone(),
                            tool_results: vec![],
                        };
                        if hooks.should_stop_after_turn(stop_ctx).await {
                            events(AgentEvent::AgentEnd {
                                messages: messages.clone(),
                            });
                            return Ok(messages);
                        }
                    }
                }
                Err(e) => {
                    events(AgentEvent::AgentEnd {
                        messages: messages.clone(),
                    });
                    return Err(match &e {
                        opi_ai::provider::ProviderError::AuthFailed(msg) => {
                            AgentError::AuthFailed(msg.clone())
                        }
                        _ => AgentError::Provider(e.to_string()),
                    });
                }
            }
        }

        // -- Queue polling after turn completes --------------------------------

        // H5: prepare_next_turn hook
        let next_turn_ctx = hooks::PrepareNextTurnContext {
            messages: messages.clone(),
            turn: turn_idx + 1,
        };
        let mut hook_injected = false;
        if let Some(update) = hooks.prepare_next_turn(next_turn_ctx).await
            && !update.extra_messages.is_empty()
        {
            hook_injected = true;
            messages.extend(update.extra_messages);
        }

        // Poll steering queue (drain all)
        let steering = drain_queue(&context.steering_queue);
        if !steering.is_empty() {
            events(AgentEvent::QueueUpdate {
                steering: steering.clone(),
                follow_up: vec![],
            });
            for msg in steering {
                messages.push(user_text_message(msg));
            }
            continue; // next turn
        }

        // If hook injected messages, continue so they reach the provider
        if hook_injected {
            continue;
        }

        // If no tools pending (agent would stop), poll follow-up (one at a time)
        if !has_tools_pending {
            let follow_up = pop_follow_up(&context.follow_up_queue);
            if !follow_up.is_empty() {
                events(AgentEvent::QueueUpdate {
                    steering: vec![],
                    follow_up: follow_up.clone(),
                });
                for msg in follow_up {
                    messages.push(user_text_message(msg));
                }
                continue; // next turn
            }
            break; // no tools, no queues → agent stops
        }

        // Tools were executed and queues are empty → continue to next turn
        let _ = turn_idx;
    }

    events(AgentEvent::AgentEnd {
        messages: messages.clone(),
    });
    Ok(messages)
}

/// Process a single stream event, updating content and emitting message events.
/// Returns Some(AssistantMessage) when a terminal event is received.
fn process_stream_event(
    event: &opi_ai::stream::AssistantStreamEvent,
    content: &mut Vec<AssistantContent>,
    events: &AgentEventSink,
) -> Option<opi_ai::message::AssistantMessage> {
    use opi_ai::stream::AssistantStreamEvent::*;

    match event {
        Start { partial } => {
            let msg = AgentMessage::Llm(Message::Assistant(partial.clone()));
            events(AgentEvent::MessageStart { message: msg });
            None
        }
        TextDelta { delta, partial, .. } => {
            // Accumulate text into content vector
            match content.last_mut() {
                Some(AssistantContent::Text { text }) => {
                    text.push_str(delta);
                }
                _ => {
                    content.push(AssistantContent::Text {
                        text: delta.clone(),
                    });
                }
            }
            let msg = AgentMessage::Llm(Message::Assistant(partial.clone()));
            events(AgentEvent::MessageUpdate {
                message: msg,
                assistant_event: Box::new(event.clone()),
            });
            None
        }
        ToolCallEnd { tool_call, .. } => {
            content.push(AssistantContent::ToolCall {
                tool_call: tool_call.clone(),
            });
            None
        }
        Done { message, .. } => Some(message.clone()),
        Error { message, .. } => Some(message.clone()),
        _ => None,
    }
}

/// Execute a single tool, with validation and hook integration.
async fn execute_tool(
    call_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
    tools_map: &HashMap<String, &dyn Tool>,
    hooks: &dyn AgentHooks,
    messages: &[AgentMessage],
    cancel: CancellationToken,
) -> ToolResult {
    let tool = match tools_map.get(tool_name) {
        Some(t) => *t,
        None => {
            return ToolResult {
                content: vec![opi_ai::message::OutputContent::Text {
                    text: format!("unknown tool: {tool_name}"),
                }],
                details: None,
                is_error: true,
                terminate: false,
            };
        }
    };

    // Validate arguments against schema
    let schema = &tool.definition().input_schema;
    if let Err(err) = validation::validate(schema, args) {
        return ToolResult::from_validation_error(err);
    }

    // Run before_tool_call hook
    let ctx = BeforeToolCallContext {
        tool_call_id: call_id.to_owned(),
        tool_name: tool_name.to_owned(),
        args: args.clone(),
        messages: messages.to_vec(),
    };
    match hooks.before_tool_call(ctx).await {
        BeforeToolCallResult::Allow => {}
        BeforeToolCallResult::Deny { reason } => {
            return ToolResult {
                content: vec![opi_ai::message::OutputContent::Text { text: reason }],
                details: None,
                is_error: true,
                terminate: false,
            };
        }
    }

    // Execute the tool
    match tool.execute(call_id, args.clone(), cancel, None).await {
        Ok(result) => {
            let ctx = AfterToolCallContext {
                tool_call_id: call_id.to_owned(),
                tool_name: tool_name.to_owned(),
                result: result.clone(),
            };
            match hooks.after_tool_call(ctx).await {
                AfterToolCallResult::Keep => result,
                AfterToolCallResult::Replace(replacement) => replacement,
            }
        }
        Err(e) => ToolResult {
            content: vec![opi_ai::message::OutputContent::Text {
                text: e.to_string(),
            }],
            details: None,
            is_error: true,
            terminate: false,
        },
    }
}

/// Drain all messages from a queue (steering mode: All).
fn drain_queue(queue: &Option<Arc<Mutex<VecDeque<String>>>>) -> Vec<String> {
    match queue {
        Some(q) => {
            let mut q = q.lock().unwrap();
            q.drain(..).collect()
        }
        None => vec![],
    }
}

/// Pop one message from a queue (follow-up mode: OneAtATime).
fn pop_follow_up(queue: &Option<Arc<Mutex<VecDeque<String>>>>) -> Vec<String> {
    match queue {
        Some(q) => {
            let mut q = q.lock().unwrap();
            match q.pop_front() {
                Some(msg) => vec![msg],
                None => vec![],
            }
        }
        None => vec![],
    }
}

/// Create a user text AgentMessage.
fn user_text_message(text: String) -> AgentMessage {
    AgentMessage::Llm(Message::User(UserMessage {
        content: vec![InputContent::Text { text }],
        timestamp_ms: 0,
    }))
}
