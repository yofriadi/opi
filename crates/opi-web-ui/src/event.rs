//! RPC/SDK event parsing for web UI consumption.
//!
//! Converts raw JSON values from the RPC JSONL protocol into typed
//! [`WebUiEvent`] variants suitable for driving a conversation state machine
//! and rendering layer.
//!
//! **Unstable 0.x API** — these types may change between minor versions.

/// A parsed event from the RPC/SDK stream, suitable for UI consumption.
///
/// Events are parsed from raw JSON values emitted by the RPC JSONL protocol.
/// Unknown event types are captured as [`WebUiEvent::Unknown`] to allow
/// forward-compatible handling.
#[derive(Debug, Clone)]
pub enum WebUiEvent {
    // -- RPC protocol events --
    /// RPC subprocess is ready (header event).
    RpcReady {
        schema_version: u32,
        version: String,
    },
    /// RPC command response.
    RpcResponse {
        command: String,
        success: bool,
        id: Option<String>,
        error: Option<String>,
        data: Option<serde_json::Value>,
    },

    // -- Agent lifecycle --
    /// Agent loop started.
    AgentStart,
    /// Agent loop ended.
    AgentEnd { message_count: usize },

    // -- Turn lifecycle --
    /// A provider request/response turn started.
    TurnStart,
    /// A provider request/response turn ended.
    TurnEnd,

    // -- Message streaming --
    /// An assistant message started streaming.
    MessageStart { model: String, provider: String },
    /// Text content delta received.
    TextDelta { index: usize, delta: String },
    /// Thinking/reasoning block started.
    ThinkingStart { index: usize },
    /// Thinking/reasoning content delta received.
    ThinkingDelta { index: usize, delta: String },
    /// Thinking/reasoning block ended.
    ThinkingEnd { index: usize, content: String },
    /// An assistant message finished streaming.
    MessageEnd,

    // -- Tool execution --
    /// Tool execution started.
    ToolStart {
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
    },
    /// Tool execution completed.
    ToolEnd {
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
        is_error: bool,
    },

    // -- Queue and retry --
    /// Steering/follow-up queue updated.
    QueueUpdate {
        steering: Vec<String>,
        follow_up: Vec<String>,
    },
    /// Auto-retry attempt started.
    AutoRetryStart {
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        error_message: String,
    },

    // -- Compaction --
    /// Context compaction started.
    CompactionStart { reason: String },
    /// Context compaction ended.
    CompactionEnd { reason: String, aborted: bool },

    // -- Session --
    /// Session metadata received.
    SessionInfo {
        session_id: String,
        turn_count: u64,
        message_count: u64,
    },
    /// Model changed.
    ModelChanged { model: String },
    /// Session persistence error.
    SessionPersistError { message: String },

    /// An event type not recognized by this version of the parser.
    Unknown {
        event_type: String,
        raw: serde_json::Value,
    },
}

impl WebUiEvent {
    /// Parse a raw JSON value into a typed event.
    ///
    /// Returns `Ok(WebUiEvent)` for all inputs — unknown types become
    /// [`WebUiEvent::Unknown`]. Returns `Err` only if the input is not a
    /// JSON object at all.
    pub fn parse(raw: &serde_json::Value) -> Result<Self, ParseError> {
        let obj = raw
            .as_object()
            .ok_or_else(|| ParseError("event is not a JSON object".into()))?;

        let event_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let event = match event_type {
            "rpc_ready" => Self::parse_rpc_ready(obj),
            "response" => Self::parse_rpc_response(obj),
            "AgentStart" => Self::AgentStart,
            "AgentEnd" => Self::parse_agent_end(obj),
            "TurnStart" => Self::TurnStart,
            "TurnEnd" => Self::TurnEnd,
            "MessageStart" => Self::parse_message_start(obj),
            "MessageUpdate" => Self::parse_message_update(obj),
            "MessageEnd" => Self::MessageEnd,
            "ToolExecutionStart" => Self::parse_tool_start(obj),
            "ToolExecutionEnd" => Self::parse_tool_end(obj),
            "QueueUpdate" => Self::parse_queue_update(obj),
            "AutoRetryStart" => Self::parse_auto_retry_start(obj),
            "CompactionStart" => Self::parse_compaction_start(obj),
            "CompactionEnd" => Self::parse_compaction_end(obj),
            "SessionPersistError" => Self::parse_session_persist_error(obj),
            _ => Self::Unknown {
                event_type: event_type.to_owned(),
                raw: raw.clone(),
            },
        };

        Ok(event)
    }

    fn parse_rpc_ready(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::RpcReady {
            schema_version: obj
                .get("schema_version")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            version: obj
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
        }
    }

    fn parse_rpc_response(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::RpcResponse {
            command: obj
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            success: obj
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            id: obj.get("id").and_then(|v| v.as_str()).map(|s| s.to_owned()),
            error: obj
                .get("error")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned()),
            data: obj.get("data").cloned(),
        }
    }

    fn parse_agent_end(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        let message_count = obj
            .get("messages")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        Self::AgentEnd { message_count }
    }

    fn parse_message_start(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        let (model, provider) = extract_model_provider(obj);
        Self::MessageStart { model, provider }
    }

    fn parse_message_update(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        let inner = obj
            .get("assistant_event")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let inner_type = inner.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match inner_type {
            "text_delta" => {
                let index = inner
                    .get("content_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let delta = inner
                    .get("delta")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                Self::TextDelta { index, delta }
            }
            "text_start" => {
                let index = inner
                    .get("content_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                Self::TextDelta {
                    index,
                    delta: String::new(),
                }
            }
            "text_end" => {
                let index = inner
                    .get("content_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                Self::TextDelta {
                    index,
                    delta: String::new(),
                }
            }
            "thinking_delta" => {
                let index = inner
                    .get("content_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let delta = inner
                    .get("delta")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                Self::ThinkingDelta { index, delta }
            }
            "thinking_start" => {
                let index = inner
                    .get("content_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                Self::ThinkingStart { index }
            }
            "thinking_end" => {
                let index = inner
                    .get("content_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let content = inner
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                Self::ThinkingEnd { index, content }
            }
            "tool_call_start" | "tool_call_delta" | "tool_call_end" => Self::Unknown {
                event_type: format!("MessageUpdate/{inner_type}"),
                raw: serde_json::Value::Object(inner),
            },
            _ => Self::Unknown {
                event_type: format!("MessageUpdate/{inner_type}"),
                raw: serde_json::Value::Object(inner),
            },
        }
    }

    fn parse_tool_start(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::ToolStart {
            tool_call_id: obj
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            tool_name: obj
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            args: obj.get("args").cloned().unwrap_or(serde_json::Value::Null),
        }
    }

    fn parse_tool_end(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::ToolEnd {
            tool_call_id: obj
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            tool_name: obj
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            result: obj
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            is_error: obj
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }

    fn parse_queue_update(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::QueueUpdate {
            steering: obj
                .get("steering")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            follow_up: obj
                .get("follow_up")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn parse_auto_retry_start(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::AutoRetryStart {
            attempt: obj.get("attempt").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            max_attempts: obj
                .get("max_attempts")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            delay_ms: obj.get("delay_ms").and_then(|v| v.as_u64()).unwrap_or(0),
            error_message: obj
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
        }
    }

    fn parse_compaction_start(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::CompactionStart {
            reason: obj
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
        }
    }

    fn parse_compaction_end(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::CompactionEnd {
            reason: obj
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            aborted: obj
                .get("aborted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }

    fn parse_session_persist_error(obj: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self::SessionPersistError {
            message: obj
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
        }
    }
}

/// Extract model and provider from a nested message object in an event.
fn extract_model_provider(obj: &serde_json::Map<String, serde_json::Value>) -> (String, String) {
    let msg = obj.get("message").and_then(|v| v.as_object());
    let model = msg
        .and_then(|m| m.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let provider = msg
        .and_then(|m| m.get("provider"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    (model, provider)
}

/// Error type for event parsing failures.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ParseError(pub String);
