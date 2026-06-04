# MCP Adapter Extension Example

This is an **example extension** that maps MCP-style tools/resources through the
extension API without making MCP a core feature. All tool/resource data comes from
fixtures -- there are no live network calls.

## Overview

The MCP adapter provides commands to discover, validate, and execute MCP-style tools,
as well as list and retrieve resources. State persists through serialization and an
execution log tracks all tool calls.

## Commands

| Command | Behavior |
|---------|----------|
| `mcp/list_tools` | Returns all registered MCP tools with schemas. |
| `mcp/call_tool` | Calls a tool by name with validated arguments. Requires `name`, optional `arguments`. |
| `mcp/list_resources` | Returns all registered MCP resources with metadata. |
| `mcp/get_resource` | Gets a resource by URI. Requires `uri`. |

## Fixture Tools

| Tool | Behavior |
|------|----------|
| `weather/get` | Returns fixed weather data. Requires `location` (string). |
| `calculator/add` | Returns sum of two numbers. Requires `a` and `b` (numbers). |
| `slow_query` | Blocks until cancelled. For testing cancellation. |
| `failing_tool` | Always returns a structured error. |

## Fixture Resources

| URI | MIME Type | Description |
|-----|-----------|-------------|
| `file:///config.json` | `application/json` | Application configuration. |
| `file:///readme.md` | `text/markdown` | Project readme. |

## Argument Validation

- Required fields are checked before execution.
- Type checking: `"number"` fields must be numeric, `"string"` fields must be strings.
- Missing required arguments produce `missing required argument: <field>` errors.
- Type mismatches produce `argument '<field>' must be a <type>` errors.

## Error Handling

| Error | Condition |
|-------|-----------|
| `tool name is required` | `mcp/call_tool` called without a name. |
| `tool '<name>' not found` | Tool name not in the registry. |
| `missing required argument: <field>` | Required argument missing from tool call. |
| `argument '<field>' must be a number` | Number field received non-numeric value. |
| `argument '<field>' must be a string` | String field received non-string value. |
| `uri is required` | `mcp/get_resource` called without a URI. |
| `resource '<uri>' not found` | Resource URI not in the registry. |

## Test Coverage

Tests in `crates/opi-coding-agent/tests/mcp_adapter_example.rs` cover:

- **Tool discovery**: List tools returns all fixture tools with schemas.
- **Schema exposure**: Tools include descriptions and input schemas.
- **Argument validation**: Missing required args, type mismatches, missing tool name.
- **Tool execution success**: Fixed results, calculator, execution logging.
- **Tool execution error**: Unknown tool, structured error from failing tool.
- **Resource metadata**: List resources, get content, unknown URI, missing URI.
- **Cancellation**: Slow tool respects cancellation token.
- **State persistence**: Full state round-trips through serialization.
- **Event observation**: Parent agent events received by extension.
- **Session integration**: Extension state survives an agent run.
- **No live network**: All operations use fixture data.
- **Unknown commands**: Unrecognized commands return `None` (passthrough).

## Package Structure

```text
examples/mcp-adapter/
  package.toml                            # Package manifest
  extensions/
    mcp-adapter/
      extension.toml                      # Extension manifest
```

## Why This Is an Example

The MCP adapter does not introduce any MCP protocol or transport into the core
runtime. It uses only the standard extension API (`Extension` trait,
`ExtensionRegistry`, `ExtensionCommand`) and demonstrates how to map external
tool/resource protocols through the extension surface without modifying opi itself.
