//! Interactive TUI mode using opi-tui for terminal rendering.
//!
//! The agent prompt runs in a spawned tokio task while the TUI render loop
//! continues to poll crossterm events and redraw at ~20 fps. Agent callbacks
//! update shared `TuiState`, which the render loop reads each frame.

use std::io;
use std::sync::{Arc, Mutex};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use opi_agent::event::AgentEvent;
use opi_agent::loop_types::AgentError;
use opi_agent::message::AgentMessage;
use opi_ai::message::{AssistantContent, Message};
use opi_ai::stream::AssistantStreamEvent;
use opi_tui::{
    AppState, Key, KeyCombo, Keybindings, Message as TuiMessage, Role as TuiRole, Shell, Theme,
    ToolCallStatus, resolve_theme,
};

use crate::harness::CodingHarness;

/// Shared state mutated by the agent callback and read by the TUI render loop.
struct TuiState {
    messages: Vec<TuiMessage>,
    input_text: String,
    app_state: AppState,
    model: String,
    active_tool: Option<(String, String, ToolCallStatus)>,
    /// True when a TextDelta has been received for the current streaming cycle.
    /// Prevents MessageEnd from pushing a duplicate text message.
    streaming_started: bool,
    theme: Theme,
    keybindings: Keybindings,
}

pub async fn run_interactive_tui(
    harness: CodingHarness,
    model: String,
    theme_name: &str,
    keybindings: Keybindings,
) -> Result<(), Box<dyn std::error::Error>> {
    let theme = resolve_theme(theme_name);
    if theme.name != theme_name {
        eprintln!("opi: warning: unknown theme {theme_name:?}, using default");
    }
    let state = Arc::new(Mutex::new(TuiState {
        messages: Vec::new(),
        input_text: String::new(),
        app_state: AppState::Idle,
        model: model.clone(),
        active_tool: None,
        streaming_started: false,
        theme,
        keybindings,
    }));

    // Wire agent events into shared state before wrapping harness
    let state_clone = state.clone();
    let mut harness = harness;
    harness.subscribe(Box::new(move |event| {
        let mut s = state_clone.lock().unwrap();
        match event {
            AgentEvent::MessageStart { .. } => {
                s.app_state = AppState::Streaming;
                s.streaming_started = false;
            }
            AgentEvent::MessageUpdate {
                assistant_event, ..
            } => {
                if let AssistantStreamEvent::TextDelta { delta, .. } = assistant_event.as_ref() {
                    if !s.streaming_started {
                        s.messages
                            .push(TuiMessage::new(TuiRole::Assistant, delta.clone()));
                        s.streaming_started = true;
                    } else if let Some(msg) = s.messages.last_mut() {
                        msg.content.push_str(delta);
                    }
                }
            }
            AgentEvent::MessageEnd {
                message: AgentMessage::Llm(Message::Assistant(a)),
            } => {
                for content in &a.content {
                    match content {
                        AssistantContent::Text { text } if !s.streaming_started => {
                            s.messages
                                .push(TuiMessage::new(TuiRole::Assistant, text.clone()));
                        }
                        AssistantContent::ToolCall { tool_call } => {
                            s.active_tool = Some((
                                tool_call.name.clone(),
                                tool_call.arguments.clone(),
                                ToolCallStatus::Running,
                            ));
                        }
                        _ => {}
                    }
                }
                s.streaming_started = false;
            }
            AgentEvent::ToolExecutionStart {
                tool_name, args, ..
            } => {
                s.app_state = AppState::ToolExecuting;
                s.active_tool = Some((
                    tool_name.clone(),
                    format!("{args}"),
                    ToolCallStatus::Running,
                ));
            }
            AgentEvent::ToolExecutionEnd {
                tool_name,
                is_error,
                ..
            } => {
                if let Some((name, args, _)) = &s.active_tool
                    && name == tool_name
                {
                    let status = if *is_error {
                        ToolCallStatus::Error("failed".into())
                    } else {
                        ToolCallStatus::Success
                    };
                    s.active_tool = Some((name.clone(), args.clone(), status));
                }
                s.app_state = AppState::Streaming;
            }
            AgentEvent::AgentEnd { .. } => {
                s.app_state = AppState::Idle;
                s.active_tool = None;
            }
            AgentEvent::TurnStart => {
                s.app_state = AppState::Thinking;
            }
            _ => {}
        }
    }));

    let harness = Arc::new(tokio::sync::Mutex::new(harness));

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main TUI loop
    let result = tui_event_loop(&mut terminal, &harness, &state).await;

    // Restore terminal
    terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn tui_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    harness: &Arc<tokio::sync::Mutex<CodingHarness>>,
    state: &Arc<Mutex<TuiState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pending: Option<tokio::task::JoinHandle<Result<Vec<AgentMessage>, AgentError>>> = None;
    let mut cancel_token = harness.lock().await.cancel_token();

    loop {
        // Render current state
        {
            let s = state.lock().unwrap();
            let shell = build_shell(&s);
            terminal.draw(|frame| frame.render_widget(shell, frame.area()))?;
        }

        // Check if pending prompt finished (non-blocking)
        if let Some(handle) = &mut pending
            && handle.is_finished()
        {
            match handle.await {
                Ok(Ok(_messages)) => {
                    let mut s = state.lock().unwrap();
                    s.app_state = AppState::Idle;
                }
                Ok(Err(AgentError::Cancelled)) => {
                    let mut s = state.lock().unwrap();
                    s.app_state = AppState::Idle;
                }
                Ok(Err(e)) => {
                    let mut s = state.lock().unwrap();
                    s.messages
                        .push(TuiMessage::new(TuiRole::System, format!("error: {e}")));
                    s.app_state = AppState::Idle;
                }
                Err(e) => {
                    let mut s = state.lock().unwrap();
                    s.messages
                        .push(TuiMessage::new(TuiRole::System, format!("error: {e}")));
                    s.app_state = AppState::Idle;
                }
            }
            // Refresh cancel token — Agent::maybe_reset_cancel() creates a new one
            // after cancellation, so the old token would be stale.
            cancel_token = harness.lock().await.cancel_token();
            pending = None;
        }

        // Poll for terminal events (non-blocking with timeout)
        if event::poll(std::time::Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let kb = state.lock().unwrap().keybindings.clone();
            if matches_key_combo(key.code, key.modifiers, &kb.submit) {
                // Ignore submit while agent is running
                if pending.is_some() {
                    continue;
                }

                let input = {
                    let mut s = state.lock().unwrap();
                    let text = s.input_text.trim().to_string();
                    s.input_text.clear();
                    text
                };

                if input == "exit" || input == "quit" {
                    // Cancel any pending task on exit
                    if let Some(handle) = pending.take() {
                        cancel_token.cancel();
                        let _ = handle.await;
                    }
                    return Ok(());
                }
                if input.is_empty() {
                    continue;
                }

                // Add user message to display
                {
                    let mut s = state.lock().unwrap();
                    s.messages
                        .push(TuiMessage::new(TuiRole::User, input.clone()));
                    s.app_state = AppState::Thinking;
                }

                // Spawn agent prompt in background task
                let h = harness.clone();
                let handle = tokio::spawn(async move {
                    let mut h = h.lock().await;
                    h.prompt(&input).await
                });
                pending = Some(handle);
            } else if matches_key_combo(key.code, key.modifiers, &kb.abort) {
                if pending.is_some() {
                    cancel_token.cancel();
                } else {
                    return Ok(());
                }
            } else if matches_key_combo(key.code, key.modifiers, &kb.new_line) {
                if pending.is_none() {
                    state.lock().unwrap().input_text.push('\n');
                }
            } else {
                match key.code {
                    KeyCode::Char(c) if pending.is_none() => {
                        state.lock().unwrap().input_text.push(c);
                    }
                    KeyCode::Backspace if pending.is_none() => {
                        state.lock().unwrap().input_text.pop();
                    }
                    _ => {}
                }
            }
        }
    }
}

fn build_shell(s: &TuiState) -> Shell {
    let mut shell = Shell::new(s.model.clone())
        .input_text(s.input_text.clone())
        .state(s.app_state)
        .theme(s.theme.clone());

    if !s.messages.is_empty() {
        shell = shell.messages(s.messages.clone());
    }

    if let Some((name, args, status)) = &s.active_tool {
        shell = shell.active_tool(name.clone(), args.clone(), status.clone());
    }

    shell
}

fn matches_key_combo(code: KeyCode, modifiers: KeyModifiers, combo: &KeyCombo) -> bool {
    let key_matches = match (code, &combo.key) {
        (KeyCode::Enter, Key::Enter) => true,
        (KeyCode::Esc, Key::Escape) => true,
        (KeyCode::Tab, Key::Tab) => true,
        (KeyCode::Backspace, Key::Backspace) => true,
        (KeyCode::Char(c), Key::Char(expected)) => c == *expected,
        _ => false,
    };
    if !key_matches {
        return false;
    }
    combo.modifiers.alt == modifiers.contains(KeyModifiers::ALT)
        && combo.modifiers.ctrl == modifiers.contains(KeyModifiers::CONTROL)
        && combo.modifiers.shift == modifiers.contains(KeyModifiers::SHIFT)
}
