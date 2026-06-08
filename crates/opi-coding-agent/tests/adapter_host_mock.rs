//! Mock adapter binary for adapter_host tests.
//!
//! This is a `harness = false` test binary that acts as a child process
//! adapter. Controlled via the `OPI_ADAPTER_TEST_MODE` environment variable.
//!
//! Modes:
//! - `capabilities` — full adapter: responds to initialize, tool_call,
//!   command, hook, state_serialize, state_restore, shutdown
//! - `hang` — reads initialize but never responds (timeout test)
//! - `crash` — reads initialize then exits with code 1 (crash test)
//! - `hang_request` — responds to initialize, then never responds again
//!   (per-request timeout test)

use std::io::{BufRead, Write};

fn main() {
    let mode = std::env::var("OPI_ADAPTER_TEST_MODE").unwrap_or_else(|_| "capabilities".into());
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    match mode.as_str() {
        "capabilities" => run_capabilities(&mut reader, &mut writer),
        "hang" => run_hang(&mut reader),
        "crash" => run_crash(&mut reader),
        "hang_request" => run_hang_request(&mut reader, &mut writer),
        _ => std::process::exit(1),
    }
}

fn read_line(reader: &mut impl BufRead) -> Option<String> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => Some(line.trim().to_string()),
        Err(_) => None,
    }
}

fn write_msg(writer: &mut impl Write, value: &serde_json::Value) {
    let json = serde_json::to_string(value).unwrap();
    writer.write_all(json.as_bytes()).unwrap();
    writer.write_all(b"\n").unwrap();
    writer.flush().unwrap();
}

fn run_capabilities(reader: &mut impl BufRead, writer: &mut impl Write) {
    // Handle initialize handshake
    if let Some(line) = read_line(reader) {
        let msg: serde_json::Value = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => return,
        };
        if msg["type"].as_str() != Some("initialize") {
            return;
        }
        let id = msg["id"].as_str().unwrap_or("1");
        write_msg(
            writer,
            &serde_json::json!({
                "type": "capabilities",
                "id": id,
                "tools": [{
                    "name": "test_tool",
                    "description": "A test tool",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "input": {"type": "string"}
                        }
                    }
                }],
                "commands": [{
                    "name": "test/status",
                    "description": "Get status"
                }],
                "hooks": ["before_tool_call", "event"],
                "model_overrides": []
            }),
        );
    }

    // Handle subsequent messages
    while let Some(line) = read_line(reader) {
        let msg: serde_json::Value = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let msg_type = msg["type"].as_str().unwrap_or("");
        let id = msg["id"].as_str().unwrap_or("").to_string();

        match msg_type {
            "tool_call" => {
                write_msg(
                    writer,
                    &serde_json::json!({
                        "type": "tool_result",
                        "id": id,
                        "content": [{"type": "text", "text": "mock_result"}],
                        "is_error": false
                    }),
                );
            }
            "command" => {
                write_msg(
                    writer,
                    &serde_json::json!({
                        "type": "command_result",
                        "id": id,
                        "data": {"status": "ok"}
                    }),
                );
            }
            "hook" => {
                write_msg(
                    writer,
                    &serde_json::json!({
                        "type": "hook_result",
                        "id": id,
                        "action": "continue",
                        "data": null
                    }),
                );
            }
            "state_serialize" => {
                write_msg(
                    writer,
                    &serde_json::json!({
                        "type": "state_result",
                        "id": id,
                        "state": {"mock": true}
                    }),
                );
            }
            "state_restore" => {
                write_msg(
                    writer,
                    &serde_json::json!({
                        "type": "state_result",
                        "id": id,
                        "state": {}
                    }),
                );
            }
            "event" | "cancel" => {
                // Fire-and-forget, no response
            }
            "shutdown" => {
                return;
            }
            _ => {}
        }
    }
}

fn run_hang(reader: &mut impl BufRead) {
    // Read the initialize line but never respond
    let mut line = String::new();
    let _ = reader.read_line(&mut line);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

fn run_crash(reader: &mut impl BufRead) {
    // Read the initialize line then crash
    let mut line = String::new();
    let _ = reader.read_line(&mut line);
    std::process::exit(1);
}

fn run_hang_request(reader: &mut impl BufRead, writer: &mut impl Write) {
    // Respond to initialize normally, then hang on subsequent requests
    if let Some(line) = read_line(reader) {
        let msg: serde_json::Value = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => return,
        };
        let id = msg["id"].as_str().unwrap_or("1");
        write_msg(
            writer,
            &serde_json::json!({
                "type": "capabilities",
                "id": id,
                "tools": [],
                "commands": [],
                "hooks": [],
                "model_overrides": []
            }),
        );
    }

    // Never respond to subsequent requests
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
