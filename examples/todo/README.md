# todo

An example package demonstrating a todo extension.

## What This Is

This is an **example package** that demonstrates task-tracking through extension
state and custom commands without adding core runtime state management.

## How It Works

The todo extension implements `on_command` to manage task items:

- **`todo/add`**: Creates a new todo item with a title and optional description.
  Returns the generated id and pending status.
- **`todo/update`**: Updates title, description, or status of an existing item.
  Validates id existence and status values.
- **`todo/list`**: Returns all items with their current state.
- **`todo/complete`**: Marks an item as completed.

## Key Properties

- **Task state management**: Items tracked through extension state.
- **Sequential IDs**: Items receive `todo-1`, `todo-2`, etc.
- **State persistence**: Full state round-trips through serialize/restore.
- **Event observation**: Parent agent events visible via `on_event`.
- **Event logging**: Operations are tracked in an events log.
- **Error handling**: Unknown ids and invalid statuses produce errors.
- **No core changes**: No core runtime modifications required.

## Package Structure

```text
todo/
  package.toml                            # Package manifest
  extensions/
    todo/
      extension.toml                      # Extension manifest
```

The actual Rust implementation lives in the test file
`crates/opi-coding-agent/tests/todo_example.rs`, which serves as both
the example code and the integration test suite.

## Process Adapter

This package also declares a process adapter in `package.toml`. The adapter
binary (`package_adapter_example`) runs as a child process communicating
over the opi extension JSONL protocol. It advertises the same todo commands
and state management capabilities through the adapter interface.

Adapter tests live in
`crates/opi-coding-agent/tests/example_adapters.rs`.
