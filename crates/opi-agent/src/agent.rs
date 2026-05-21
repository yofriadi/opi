//! Stateful Agent wrapper around the agent loop (S8.2).
//!
//! Provides `prompt`, `continue_`, `abort`, `subscribe`, `steer`, and
//! `follow_up` methods, managing conversation state, cancellation, event
//! subscribers, and message queues.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use opi_ai::message::{InputContent, Message, UserMessage};
use opi_ai::provider::Provider;
use tokio_util::sync::CancellationToken;

use crate::event::{AgentEvent, AgentEventSink};
use crate::hooks::AgentHooks;
use crate::loop_types::{AgentError, AgentLoopConfig, AgentLoopContext};
use crate::message::AgentMessage;
use crate::tool::{ExecutionMode, Tool, ToolError, ToolResult};

// -- Arc wrappers for Provider and Tool reuse across calls ------------------

struct SharedProvider(Arc<dyn Provider>);

impl Provider for SharedProvider {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn models(&self) -> &[opi_ai::provider::ModelInfo] {
        self.0.models()
    }
    fn stream(&self, request: opi_ai::provider::Request) -> opi_ai::provider::EventStream {
        self.0.stream(request)
    }
}

struct SharedTool(Arc<dyn Tool>);

impl Tool for SharedTool {
    fn definition(&self) -> opi_ai::message::ToolDef {
        self.0.definition()
    }

    fn execute(
        &self,
        call_id: &str,
        arguments: serde_json::Value,
        signal: CancellationToken,
        on_update: Option<crate::tool::UpdateCallback>,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send>> {
        self.0.execute(call_id, arguments, signal, on_update)
    }

    fn execution_mode(&self) -> ExecutionMode {
        self.0.execution_mode()
    }
}

// -- Agent -------------------------------------------------------------------

type EventSubscriber = Box<dyn Fn(&AgentEvent) + Send + Sync>;

/// Stateful wrapper around `agent_loop` with conversation state, cancellation,
/// event subscription, and message queue management.
pub struct Agent {
    provider: Arc<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    model: String,
    system: Option<String>,
    config: AgentLoopConfig,
    hooks: Box<dyn AgentHooks>,
    cancel: CancellationToken,
    subscribers: Arc<Mutex<Vec<EventSubscriber>>>,
    messages: Vec<AgentMessage>,
    steering_queue: Arc<Mutex<VecDeque<String>>>,
    follow_up_queue: Arc<Mutex<VecDeque<String>>>,
}

impl Agent {
    /// Create a new Agent with the given provider, tools, model, and hooks.
    pub fn new(
        provider: Box<dyn Provider>,
        tools: Vec<Box<dyn Tool>>,
        model: String,
        system: Option<String>,
        config: AgentLoopConfig,
        hooks: Box<dyn AgentHooks>,
    ) -> Self {
        Self {
            provider: Arc::from(provider),
            tools: tools.into_iter().map(Arc::from).collect(),
            model,
            system,
            config,
            hooks,
            cancel: CancellationToken::new(),
            subscribers: Arc::new(Mutex::new(Vec::new())),
            messages: Vec::new(),
            steering_queue: Arc::new(Mutex::new(VecDeque::new())),
            follow_up_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Send a user message and run the agent loop.
    ///
    /// Resets the cancellation state if the agent was previously aborted,
    /// allowing a fresh conversation turn.
    pub async fn prompt(
        &mut self,
        text: impl Into<String>,
    ) -> Result<Vec<AgentMessage>, AgentError> {
        self.maybe_reset_cancel();
        let token = self.cancel.child_token();
        self.messages
            .push(AgentMessage::Llm(Message::User(UserMessage {
                content: vec![InputContent::Text { text: text.into() }],
                timestamp_ms: 0,
            })));
        self.run_with_token(token).await
    }

    /// Continue the conversation with an additional user message.
    ///
    /// Requires the last context message to be a user message or tool result.
    pub async fn continue_(
        &mut self,
        text: impl Into<String>,
    ) -> Result<Vec<AgentMessage>, AgentError> {
        self.maybe_reset_cancel();

        if self.messages.is_empty() {
            return Err(AgentError::Hook("cannot continue: no messages".into()));
        }

        let token = self.cancel.child_token();
        self.messages
            .push(AgentMessage::Llm(Message::User(UserMessage {
                content: vec![InputContent::Text { text: text.into() }],
                timestamp_ms: 0,
            })));
        self.run_with_token(token).await
    }

    /// Cancel the current operation.
    ///
    /// Equivalent to the first Ctrl+C. The running `prompt` or `continue_`
    /// call will return `AgentError::Cancelled`.
    pub fn abort(&self) {
        self.cancel.cancel();
    }

    /// Register an event subscriber that receives all `AgentEvent`s.
    pub fn subscribe(&mut self, callback: EventSubscriber) {
        self.subscribers.lock().unwrap().push(callback);
    }

    /// Return a clonable cancellation token for external cancellation.
    ///
    /// Cancelling this token cancels the currently running loop operation.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Add a steering message to be delivered before the next provider request.
    ///
    /// Steering messages are high-priority and delivered after the current
    /// turn's tool calls complete but before the next provider request.
    pub fn steer(&self, message: String) {
        self.steering_queue.lock().unwrap().push_back(message);
    }

    /// Add a follow-up message to be delivered when the agent would otherwise stop.
    ///
    /// Follow-up messages are only delivered when the agent has no tool calls
    /// pending and no steering messages queued.
    pub fn follow_up(&self, message: String) {
        self.follow_up_queue.lock().unwrap().push_back(message);
    }

    // -- Internal helpers ---------------------------------------------------

    fn maybe_reset_cancel(&mut self) {
        if self.cancel.is_cancelled() {
            self.cancel = CancellationToken::new();
        }
    }

    fn build_event_sink(&self) -> AgentEventSink {
        let subscribers = self.subscribers.clone();
        Box::new(move |event: AgentEvent| {
            let subs = subscribers.lock().unwrap();
            for sub in subs.iter() {
                sub(&event);
            }
        })
    }

    async fn run_with_token(
        &mut self,
        cancel: CancellationToken,
    ) -> Result<Vec<AgentMessage>, AgentError> {
        let context = AgentLoopContext {
            provider: Box::new(SharedProvider(self.provider.clone())),
            tools: self
                .tools
                .iter()
                .map(|t| Box::new(SharedTool(t.clone())) as Box<dyn Tool>)
                .collect(),
            messages: self.messages.clone(),
            model: self.model.clone(),
            system: self.system.clone(),
            steering_queue: Some(self.steering_queue.clone()),
            follow_up_queue: Some(self.follow_up_queue.clone()),
        };

        let sink = self.build_event_sink();
        let result =
            crate::agent_loop(context, self.config.clone(), &*self.hooks, sink, cancel).await?;

        self.messages = result.clone();
        Ok(result)
    }
}
