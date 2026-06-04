# mcp-adapter

An example package demonstrating an MCP adapter extension.

## What This Is

This is an **example package** that maps MCP-style tools and resources through the
extension API without making MCP a core feature. All tool/resource data comes from
fixtures -- no live network calls.

## How It Works

The MCP adapter extension implements `on_command` to handle MCP-style operations:

- **`mcp/list_tools`**: Returns all registered MCP tools with schemas.
- **`mcp/call_tool`**: Calls a tool by name with validated arguments.
- **`mcp/list_resources`**: Returns all registered MCP resources.
- **`mcp/get_resource`**: Gets a resource by URI.

## Key Properties

- **Tool discovery**: Tools listed with names, descriptions, and JSON schemas.
- **Argument validation**: Required fields and type checking enforced.
- **Fixture execution**: No live network -- tools return fixture data.
- **Resource metadata**: Resources with URIs, MIME types, and content.
- **Cancellation**: Long-running tools respect cancellation tokens.
- **State persistence**: Full state round-trips through serialize/restore.
- **No core changes**: No MCP protocol or transport in the core runtime.

## Package Structure

```text
mcp-adapter/
  package.toml                            # Package manifest
  extensions/
    mcp-adapter/
      extension.toml                      # Extension manifest
```

The actual Rust implementation lives in the test file
`crates/opi-coding-agent/tests/mcp_adapter_example.rs`, which serves as both
the example code and the integration test suite.
