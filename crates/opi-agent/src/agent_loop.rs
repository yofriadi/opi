use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use opi_ai::message::{AssistantContent, InputContent, Message, ToolResultMessage, UserMessage};
use opi_ai::provider::{Request, validate_request_capabilities};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::event::{AgentEvent, AgentEventSink};
use crate::hooks::{
    AfterToolCallContext, AfterToolCallResult, AgentHooks, BeforeToolCallContext,
    BeforeToolCallResult, PrepareNextTurnContext, ShouldStopAfterTurnContext,
};
use crate::loop_types::{AgentError, AgentLoopConfig, AgentLoopContext};
use crate::message::AgentMessage;
use crate::tool::{ExecutionMode, Tool, ToolResult};
use crate::validation;

/// Run the agent loop until completion or cancellation.
///
/// The loop iterates: provider request, stream response, detect tool calls,
/// validate and execute tools, send tool results back, and repeat until no
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

        let transformed = hooks
            .transform_context(messages.clone(), cancel.clone())
            .await?;

        let llm_messages = hooks.convert_to_llm(&transformed)?;

        let mut assistant_content: Vec<AssistantContent> = Vec::new();
        has_tools_pending = false;
        let mut retry_attempt: u32 = 0;
        let max_attempts = config.retry.as_ref().map(|r| r.max_attempts).unwrap_or(0);

        'stream: loop {
            let request = Request {
                model: context.model.clone(),
                system: context.system.clone(),
                messages: llm_messages.clone(),
                tools: tool_defs.clone(),
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                thinking: config.thinking.clone().unwrap_or_default(),
                stop_sequences: vec![],
                metadata: None,
                cancel: cancel.clone(),
            };
            if let Err(e) = validate_request_capabilities(context.provider.as_ref(), &request) {
                events(AgentEvent::AgentEnd {
                    messages: messages.clone(),
                });
                return Err(AgentError::Provider(e.to_string()));
            }
            let mut stream = context.provider.stream(request);
            assistant_content.clear();

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
                        if let Some(msg) =
                            process_stream_event(&event, &mut assistant_content, &events)
                        {
                            // Use the provider's complete content, which can include thinking,
                            // instead of the local accumulator that tracks text and tool calls.
                            let agent_msg = AgentMessage::Llm(Message::Assistant(msg));

                            events(AgentEvent::MessageEnd {
                                message: agent_msg.clone(),
                            });

                            messages.push(agent_msg.clone());

                            let content = match &agent_msg {
                                AgentMessage::Llm(Message::Assistant(a)) => &a.content,
                                _ => &Vec::new(),
                            };
                            let tool_calls: Vec<_> = content
                                .iter()
                                .filter_map(|c| match c {
                                    AssistantContent::ToolCall { tool_call } => {
                                        Some(tool_call.clone())
                                    }
                                    _ => None,
                                })
                                .collect();

                            if !tool_calls.is_empty() {
                                has_tools_pending = true;
                                let mut tool_results = Vec::new();
                                let mut terminate_flags = Vec::new();

                                let batch_is_sequential = tool_calls.iter().any(|tc| {
                                    tools_map
                                        .get(tc.name.as_str())
                                        .map(|t| t.execution_mode() == ExecutionMode::Sequential)
                                        .unwrap_or(true)
                                });

                                if batch_is_sequential {
                                    for tc in &tool_calls {
                                        let args: serde_json::Value =
                                            serde_json::from_str(&tc.arguments)
                                                .unwrap_or(json!({}));

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
                                        let details = result.details.clone();
                                        terminate_flags.push(result.terminate);
                                        events(AgentEvent::ToolExecutionEnd {
                                            tool_call_id: tc.id.clone(),
                                            tool_name: tc.name.clone(),
                                            result: serde_json::json!(&result.content),
                                            details,
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
                                        let details = result.details.clone();
                                        terminate_flags.push(result.terminate);
                                        events(AgentEvent::ToolExecutionEnd {
                                            tool_call_id: tc_id.clone(),
                                            tool_name: tc_name.clone(),
                                            result: serde_json::json!(&result.content),
                                            details,
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

                                let all_terminate = !terminate_flags.is_empty()
                                    && terminate_flags.iter().all(|t| *t);

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

                                break 'stream;
                            }

                            events(AgentEvent::TurnEnd {
                                message: agent_msg.clone(),
                                tool_results: vec![],
                            });

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
                        if e.is_retryable()
                            && retry_attempt < max_attempts
                            && let Some(ref rc) = config.retry
                        {
                            let retry_after_ms = match &e {
                                opi_ai::provider::ProviderError::RateLimited { retry_after_ms } => {
                                    *retry_after_ms
                                }
                                _ => None,
                            };
                            let delay_ms = rc.delay_for_attempt(retry_attempt, retry_after_ms);
                            retry_attempt += 1;

                            events(AgentEvent::AutoRetryStart {
                                attempt: retry_attempt,
                                max_attempts: rc.max_attempts,
                                delay_ms,
                                error_message: e.to_string(),
                            });

                            tokio::select! {
                                biased;
                                _ = cancel.cancelled() => {
                                    events(AgentEvent::AgentEnd {
                                        messages: messages.clone(),
                                    });
                                    return Err(AgentError::Cancelled);
                                }
                                _ = tokio::time::sleep(
                                    std::time::Duration::from_millis(delay_ms)
                                ) => {}
                            }
                            continue 'stream;
                        }

                        if retry_attempt > 0 {
                            events(AgentEvent::AutoRetryEnd {
                                success: false,
                                attempt: retry_attempt,
                                final_error: Some(e.to_string()),
                            });
                        }

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

            if retry_attempt > 0 {
                events(AgentEvent::AutoRetryEnd {
                    success: true,
                    attempt: retry_attempt,
                    final_error: None,
                });
            }
            break 'stream;
        }

        let next_turn_ctx = PrepareNextTurnContext {
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

        let steering = drain_queue(&context.steering_queue);
        if !steering.is_empty() {
            events(AgentEvent::QueueUpdate {
                steering: steering.clone(),
                follow_up: vec![],
            });
            for msg in steering {
                messages.push(user_text_message(msg));
            }
            continue;
        }

        if hook_injected {
            continue;
        }

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
                continue;
            }
            break;
        }

        let _ = turn_idx;
    }

    events(AgentEvent::AgentEnd {
        messages: messages.clone(),
    });
    Ok(messages)
}

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
        ThinkingStart { partial, .. }
        | ThinkingDelta { partial, .. }
        | ThinkingEnd { partial, .. } => {
            let msg = AgentMessage::Llm(Message::Assistant(partial.clone()));
            events(AgentEvent::MessageUpdate {
                message: msg,
                assistant_event: Box::new(event.clone()),
            });
            None
        }
        Done { message, .. } => Some(message.clone()),
        Error { message, .. } => Some(message.clone()),
        _ => None,
    }
}

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

    let schema = &tool.definition().input_schema;
    if let Err(err) = validation::validate(schema, args) {
        return ToolResult::from_validation_error(err);
    }

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

fn drain_queue(queue: &Option<Arc<Mutex<VecDeque<String>>>>) -> Vec<String> {
    match queue {
        Some(q) => {
            let mut q = q.lock().unwrap();
            q.drain(..).collect()
        }
        None => vec![],
    }
}

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

fn user_text_message(text: String) -> AgentMessage {
    AgentMessage::Llm(Message::User(UserMessage {
        content: vec![InputContent::Text { text }],
        timestamp_ms: 0,
    }))
}
