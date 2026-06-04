# Todo Extension Example

This is an **example extension** that demonstrates task-tracking through
extension state and custom commands. It is **not core runtime state management**
and does not add task tracking to the agent.

## Overview

The todo extension provides commands to create, update, list, and complete task
items. Each item has an id, title, description, and status. State persists
through serialization and an events log tracks all operations.

## Commands

| Command | Behavior |
|---------|----------|
| `todo/add` | Creates a new item. Requires `title`, optional `description`. |
| `todo/update` | Updates title, description, or status. Requires `id`. |
| `todo/list` | Returns all items with current state. |
| `todo/complete` | Marks an item as completed. Requires `id`. |

## How It Works

1. The extension registers `on_command` to handle `todo/add`,
   `todo/update`, `todo/list`, and `todo/complete`.
2. Items are stored in a `TodoState` with sequential IDs (`todo-1`, `todo-2`).
3. Each operation is logged to an `events_log` for audit purposes.
4. `on_event` observes parent agent events for diagnostic purposes.
5. State persists through `serialize_state`/`restore_state`.

## Status Values

| Status | Meaning |
|--------|---------|
| `pending` | Newly created, not yet started. |
| `in_progress` | Currently being worked on. |
| `completed` | Finished. |

## Error Handling

| Error | Condition |
|-------|-----------|
| `title is required` | `todo/add` called without a title. |
| `todo '<id>' not found` | Update or complete with unknown id. |
| `invalid status: <value>` | Update with unrecognized status string. |

## Test Coverage

Tests in `crates/opi-coding-agent/tests/todo_example.rs` cover:

- **Add**: Creates item with title and optional description.
- **Add validation**: Missing title produces an error.
- **Update**: Changes title, description, and status fields.
- **Update validation**: Unknown id and invalid status produce errors.
- **List**: Returns all items with correct count.
- **Complete**: Marks item as completed.
- **Complete validation**: Unknown id produces an error.
- **State persistence**: Full state round-trips through serialization.
- **Event observation**: Parent agent events received by extension.
- **Session integration**: Extension state survives an agent run.
- **Failure recovery**: State can be restored from a serialized checkpoint.
- **Event logging**: Operations are tracked in order.
- **Sequential IDs**: Items receive incremental ids.
- **Unknown commands**: Unrecognized commands return `None` (passthrough).

## Package Structure

```text
examples/todo/
  package.toml                            # Package manifest
  extensions/
    todo/
      extension.toml                      # Extension manifest
```

## Why This Is an Example

The todo extension does not introduce any new concepts into the core
runtime. It uses only the standard extension API (`Extension` trait,
`ExtensionRegistry`, `ExtensionCommand`) and demonstrates how to build
stateful workflows without modifying opi itself.
