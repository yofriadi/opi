//! Example adapter binary for the todo, permission-gate, and protected-paths packages.
//!
//! This is a `harness = false` test binary that acts as a child process
//! adapter for the three example packages. The mode is selected via the
//! first positional CLI argument.
//!
//! Modes:
//! - `todo` — advertises todo commands (todo/add, todo/list, todo/update,
//!   todo/complete), state serialize/restore, and event hook.
//! - `permission-gate` — advertises before_tool_call hook that blocks
//!   mutating tools (bash, write, edit), state serialize/restore.
//! - `protected-paths` — advertises before_tool_call hook that blocks
//!   file operations on protected paths (/etc, /proc), state serialize/restore.

use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("todo");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    match mode {
        "todo" => run_todo(&mut reader, &mut writer),
        "permission-gate" => run_permission_gate(&mut reader, &mut writer),
        "protected-paths" => run_protected_paths(&mut reader, &mut writer),
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

// ---------------------------------------------------------------------------
// todo mode
// ---------------------------------------------------------------------------

fn todo_capabilities() -> serde_json::Value {
    serde_json::json!({
        "tools": [],
        "commands": [
            {"name": "todo/add", "description": "Add a new todo item"},
            {"name": "todo/list", "description": "List all todo items"},
            {"name": "todo/update", "description": "Update a todo item"},
            {"name": "todo/complete", "description": "Complete a todo item"}
        ],
        "hooks": ["event"],
        "model_overrides": []
    })
}

fn run_todo(reader: &mut impl BufRead, writer: &mut impl Write) {
    let mut items: Vec<serde_json::Value> = Vec::new();
    let mut next_id: u64 = 1;

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
        let mut caps = todo_capabilities();
        caps["type"] = "capabilities".into();
        caps["id"] = id.into();
        write_msg(writer, &caps);
    }

    while let Some(line) = read_line(reader) {
        let msg: serde_json::Value = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let msg_type = msg["type"].as_str().unwrap_or("");
        let id = msg["id"].as_str().unwrap_or("").to_string();

        match msg_type {
            "command" => {
                let name = msg["name"].as_str().unwrap_or("");
                match name {
                    "todo/add" => {
                        let title = msg["args"]["title"].as_str().unwrap_or("untitled");
                        let description = msg["args"]["description"].as_str().unwrap_or("");
                        let item_id = format!("todo-{next_id}");
                        next_id += 1;
                        let item = serde_json::json!({
                            "id": item_id,
                            "title": title,
                            "description": description,
                            "status": "pending"
                        });
                        items.push(item.clone());
                        write_msg(
                            writer,
                            &serde_json::json!({
                                "type": "command_result",
                                "id": id,
                                "data": item
                            }),
                        );
                    }
                    "todo/list" => {
                        write_msg(
                            writer,
                            &serde_json::json!({
                                "type": "command_result",
                                "id": id,
                                "data": {"items": items}
                            }),
                        );
                    }
                    "todo/update" => {
                        let target_id = msg["args"]["id"].as_str().unwrap_or("");
                        let found = items
                            .iter_mut()
                            .find(|i| i["id"].as_str() == Some(target_id));
                        if let Some(item) = found {
                            if let Some(title) = msg["args"]["title"].as_str() {
                                item["title"] = title.into();
                            }
                            if let Some(description) = msg["args"]["description"].as_str() {
                                item["description"] = description.into();
                            }
                            if let Some(status) = msg["args"]["status"].as_str() {
                                item["status"] = status.into();
                            }
                            write_msg(
                                writer,
                                &serde_json::json!({
                                    "type": "command_result",
                                    "id": id,
                                    "data": item
                                }),
                            );
                        } else {
                            write_msg(
                                writer,
                                &serde_json::json!({
                                    "type": "error",
                                    "id": id,
                                    "message": format!("todo item not found: {target_id}")
                                }),
                            );
                        }
                    }
                    "todo/complete" => {
                        let target_id = msg["args"]["id"].as_str().unwrap_or("");
                        let found = items
                            .iter_mut()
                            .find(|i| i["id"].as_str() == Some(target_id));
                        if let Some(item) = found {
                            item["status"] = "completed".into();
                            write_msg(
                                writer,
                                &serde_json::json!({
                                    "type": "command_result",
                                    "id": id,
                                    "data": item
                                }),
                            );
                        } else {
                            write_msg(
                                writer,
                                &serde_json::json!({
                                    "type": "error",
                                    "id": id,
                                    "message": format!("todo item not found: {target_id}")
                                }),
                            );
                        }
                    }
                    _ => {
                        write_msg(
                            writer,
                            &serde_json::json!({
                                "type": "error",
                                "id": id,
                                "message": format!("unknown command: {name}")
                            }),
                        );
                    }
                }
            }
            "state_serialize" => {
                write_msg(
                    writer,
                    &serde_json::json!({
                        "type": "state_result",
                        "id": id,
                        "state": {"items": items, "next_id": next_id}
                    }),
                );
            }
            "state_restore" => {
                let state = &msg["state"];
                if let Some(arr) = state["items"].as_array() {
                    items = arr.clone();
                }
                if let Some(n) = state["next_id"].as_u64() {
                    next_id = n;
                }
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
                // Fire-and-forget
            }
            "shutdown" => {
                return;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// permission-gate mode
// ---------------------------------------------------------------------------

fn permission_gate_capabilities() -> serde_json::Value {
    serde_json::json!({
        "tools": [],
        "commands": [],
        "hooks": ["before_tool_call", "event"],
        "model_overrides": []
    })
}

/// Mutating tools that the permission gate blocks by default.
const MUTATING_TOOLS: &[&str] = &["bash", "write", "edit"];

fn run_permission_gate(reader: &mut impl BufRead, writer: &mut impl Write) {
    let mut audit_log: Vec<serde_json::Value> = Vec::new();

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
        let mut caps = permission_gate_capabilities();
        caps["type"] = "capabilities".into();
        caps["id"] = id.into();
        write_msg(writer, &caps);
    }

    while let Some(line) = read_line(reader) {
        let msg: serde_json::Value = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let msg_type = msg["type"].as_str().unwrap_or("");
        let id = msg["id"].as_str().unwrap_or("").to_string();

        match msg_type {
            "hook" => {
                let hook = msg["hook"].as_str().unwrap_or("");
                match hook {
                    "before_tool_call" => {
                        let payload = &msg["payload"];
                        let tool = payload["tool"].as_str().unwrap_or("");

                        if MUTATING_TOOLS.contains(&tool) {
                            let reason =
                                format!("{tool} blocked by example permission-gate adapter");
                            audit_log.push(serde_json::json!({
                                "tool": tool,
                                "action": "block",
                                "reason": reason
                            }));
                            write_msg(
                                writer,
                                &serde_json::json!({
                                    "type": "hook_result",
                                    "id": id,
                                    "action": "block",
                                    "data": {"reason": reason}
                                }),
                            );
                        } else {
                            audit_log.push(serde_json::json!({
                                "tool": tool,
                                "action": "allow"
                            }));
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
                    }
                    _ => {
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
                }
            }
            "state_serialize" => {
                write_msg(
                    writer,
                    &serde_json::json!({
                        "type": "state_result",
                        "id": id,
                        "state": {"audit_log": audit_log}
                    }),
                );
            }
            "state_restore" => {
                let state = &msg["state"];
                if let Some(arr) = state["audit_log"].as_array() {
                    audit_log = arr.clone();
                }
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
                // Fire-and-forget
            }
            "shutdown" => {
                return;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// protected-paths mode
// ---------------------------------------------------------------------------

fn protected_paths_capabilities() -> serde_json::Value {
    serde_json::json!({
        "tools": [],
        "commands": [],
        "hooks": ["before_tool_call", "event"],
        "model_overrides": []
    })
}

/// Path prefixes that the protected-paths adapter blocks.
const PROTECTED_PREFIXES: &[&str] = &["/etc/", "/proc/", "/sys/"];

/// File tools whose `path` argument is checked.
const FILE_TOOLS: &[&str] = &["read", "write", "edit"];

fn is_protected_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    PROTECTED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

fn run_protected_paths(reader: &mut impl BufRead, writer: &mut impl Write) {
    let mut audit_log: Vec<serde_json::Value> = Vec::new();

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
        let mut caps = protected_paths_capabilities();
        caps["type"] = "capabilities".into();
        caps["id"] = id.into();
        write_msg(writer, &caps);
    }

    while let Some(line) = read_line(reader) {
        let msg: serde_json::Value = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let msg_type = msg["type"].as_str().unwrap_or("");
        let id = msg["id"].as_str().unwrap_or("").to_string();

        match msg_type {
            "hook" => {
                let hook = msg["hook"].as_str().unwrap_or("");
                match hook {
                    "before_tool_call" => {
                        let payload = &msg["payload"];
                        let tool = payload["tool"].as_str().unwrap_or("");
                        let args = &payload["args"];

                        if FILE_TOOLS.contains(&tool) {
                            let path = args["path"].as_str().unwrap_or("");
                            if is_protected_path(path) {
                                let reason = format!("{tool} blocked: path '{path}' is protected");
                                audit_log.push(serde_json::json!({
                                    "tool": tool,
                                    "path": path,
                                    "action": "block",
                                    "reason": reason
                                }));
                                write_msg(
                                    writer,
                                    &serde_json::json!({
                                        "type": "hook_result",
                                        "id": id,
                                        "action": "block",
                                        "data": {"reason": reason}
                                    }),
                                );
                                continue;
                            }
                        }

                        audit_log.push(serde_json::json!({
                            "tool": tool,
                            "action": "allow"
                        }));
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
                    _ => {
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
                }
            }
            "state_serialize" => {
                write_msg(
                    writer,
                    &serde_json::json!({
                        "type": "state_result",
                        "id": id,
                        "state": {"audit_log": audit_log}
                    }),
                );
            }
            "state_restore" => {
                let state = &msg["state"];
                if let Some(arr) = state["audit_log"].as_array() {
                    audit_log = arr.clone();
                }
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
                // Fire-and-forget
            }
            "shutdown" => {
                return;
            }
            _ => {}
        }
    }
}
