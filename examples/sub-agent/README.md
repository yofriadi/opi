# sub-agent

An example package demonstrating a sub-agent extension.

## What This Is

This is an **example package** that demonstrates how to run nested agent
workflows through the extension API and SDK/RPC command model. It is **not
core sub-agent functionality** and does not add feature flags to the agent
runtime.

## How It Works

The sub-agent extension implements `on_command` to dispatch child agent runs:

- **`sub-agent/run`**: Creates a fresh child agent with isolated provider,
  tools, and cancellation token, runs it to completion, and returns the result.
- **`sub-agent/list`**: Returns the run history with status and results.

## Key Properties

- **Isolated state**: Each child run creates a fresh agent. Child state does
  not leak to the parent.
- **Bounded cancellation**: Child agents have their own cancellation tokens
  that can be triggered externally.
- **Event routing**: Child agent events are observable by the parent extension.
- **SDK integration**: Uses `SDK_SCHEMA_VERSION` and SDK types for command
  responses.
- **No feature flags**: No core runtime changes required.

## Package Structure

```text
sub-agent/
  package.toml                            # Package manifest
  extensions/
    sub-agent/
      extension.toml                      # Extension manifest
```

The actual Rust implementation lives in the test file
`crates/opi-coding-agent/tests/sub_agent_example.rs`, which serves as both
the example code and the integration test suite.
