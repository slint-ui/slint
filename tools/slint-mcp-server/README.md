# Slint MCP Server

An [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server that bridges the Slint system testing protocol, enabling LLMs and other MCP clients to inspect and interact with running Slint applications.

## How It Works

```
┌──────────┐     stdio (JSON-RPC)     ┌──────────────────┐     TCP (Protobuf)     ┌──────────────┐
│ MCP      │ ◄──────────────────────► │ slint-mcp-server │ ◄──────────────────── │ Slint App    │
│ Client   │                          │                  │                        │              │
│ (Claude) │                          │  Listens on port │                        │ SLINT_TEST_  │
│          │                          │  4242 by default │                        │ SERVER=:4242 │
└──────────┘                          └──────────────────┘                        └──────────────┘
```

1. The MCP server starts and listens on a TCP port (default: 4242)
2. You launch your Slint app with `SLINT_TEST_SERVER=localhost:4242`
3. The app connects to the MCP server
4. MCP clients can then use tools to inspect/interact with the app

## Building

```bash
cd tools/slint-mcp-server
cargo build --release
```

**Note:** Your Slint app must be built with the `system-testing` feature and `SLINT_EMIT_DEBUG_INFO=1` for full element introspection.

## Usage

```bash
# Start the MCP server
slint-mcp-server --port 4242

# In another terminal, launch your Slint app
SLINT_TEST_SERVER=localhost:4242 ./your-slint-app
```

### MCP Client Configuration

Add to your MCP client configuration (e.g., Claude Desktop):

```json
{
  "mcpServers": {
    "slint": {
      "command": "path/to/slint-mcp-server",
      "args": ["--port", "4242"]
    }
  }
}
```

## Available Tools

| Tool | Description |
|------|-------------|
| `list_windows` | List all windows in the connected Slint application |
| `get_window_properties` | Get window size, position, and state |
| `find_elements_by_id` | Find elements by qualified ID (e.g., `App::mybutton`) |
| `get_element_properties` | Get all properties of an element (type, accessible props, geometry) |
| `query_element_descendants` | Query descendants with filters (by ID, type, role) |
| `get_element_tree` | Get the full element tree as a hierarchical JSON structure (see note below) |
| `take_screenshot` | Take a screenshot (returned as base64 PNG) |
| `click_element` | Simulate single/double click on an element |
| `invoke_accessibility_action` | Invoke accessibility actions (default, increment, decrement, expand) |
| `set_element_value` | Set the accessible value of an element |
| `dispatch_key_event` | Send keyboard events to a window |

### Performance Note: `get_element_tree`

`get_element_tree` makes two sequential protobuf round-trips per element in the tree (one for properties, one for children). For large UIs with hundreds of elements, this can be slow. Use `max_depth` to limit traversal depth and prefer `query_element_descendants` with targeted filters for better performance.

## Example Session

```
> list_windows
{ "windows": [{ "index": 0, "generation": 0 }] }

> get_window_properties { "window_handle": { "index": 0, "generation": 0 } }
{ "size": { "width": 800, "height": 600 }, "root_element_handle": { "index": 0, "generation": 0 }, ... }

> get_element_tree { "element_handle": { "index": 0, "generation": 0 }, "max_depth": 3 }
{ "type_info": [...], "accessible_role": "unknown", "children": [...] }

> take_screenshot { "window_handle": { "index": 0, "generation": 0 } }
{ "content": [{ "type": "image", "data": "iVBORw0KGgo...", "mimeType": "image/png" }] }
```

## Recommended CLAUDE.md Snippet

If you're using Claude Code with a Slint project, add the following to your project's `CLAUDE.md` so Claude knows how to build and inspect your app:

````markdown
## Slint UI Introspection

This project uses the Slint UI framework. To inspect the running application:

### Build for introspection
```bash
SLINT_EMIT_DEBUG_INFO=1 cargo build --features system-testing
```

### Run with the MCP server
```bash
SLINT_TEST_SERVER=localhost:4242 cargo run --features system-testing
```

The `slint-mcp-server` MCP tool is available for inspecting the running app. Typical workflow:
1. `list_windows` — discover open windows
2. `get_window_properties` — get window info and the root element handle
3. `get_element_tree` (start with max_depth=2 or 3) — explore the UI hierarchy
4. `find_elements_by_id` or `query_element_descendants` — find specific elements
5. `get_element_properties` — read properties of a specific element
6. `take_screenshot` — capture the current visual state
````
