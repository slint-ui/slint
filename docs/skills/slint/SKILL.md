---
name: slint
description: Expert guidance for building, debugging, and working with Slint GUI applications. Covers the .slint markup language, project setup, debugging with the embedded MCP server, and language API bindings for Rust, C++, JavaScript, and Python.
---

# Slint Development Skill

Use this skill when building, debugging, or reviewing applications that use [Slint](https://slint.dev), a declarative GUI toolkit for native user interfaces across desktop, embedded, mobile, and web platforms.

## When to Use This Skill

Use this skill when the task involves:
- Writing or debugging `.slint` files
- Integrating Slint with Rust, C++, JavaScript, or Python
- Investigating layout, binding, rendering, or event-handling issues
- Enabling the Slint MCP server for runtime inspection and UI debugging
- Explaining or reviewing Slint-specific code patterns

## How to Help

When using this skill:
- Prefer idiomatic Slint patterns over manual UI workarounds
- Match guidance to the user's language binding and Slint version
- Watch for common pitfalls such as binding loops, missing layout constraints, and type mismatches
- Suggest the MCP server when runtime inspection or interaction would make debugging easier
- Prefer solutions that preserve Slint's declarative and reactive model

## The .slint Language

Slint UIs are written in `.slint` markup files. The language is declarative and reactive.

## Project Setup

### Rust

```toml
# Cargo.toml
[dependencies]
slint = "1.x"

[build-dependencies]
slint-build = "1.x"
```

```rust
// build.rs
fn main() {
    slint_build::compile("ui/main.slint").unwrap();
}
```

```rust
// main.rs
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = MainWindow::new()?;
    // Set up callbacks, models, etc.
    app.run()
}
```

### C++

Use CMake with `FetchContent` or `find_package`:
```cmake
find_package(Slint)
slint_target_sources(my_app ui/main.slint)
```

### Node.js

```js
const slint = require("slint-ui");
const app = new slint.MainWindow();
app.run();
```

### Python

```python
import slint
# Load .slint files dynamically
```

## Debugging Slint Applications

### Common Issues

1. **Binding loops**: A property depends on itself through a chain of bindings. The compiler warns about these. Break the cycle by introducing an intermediate property or restructuring.

2. **Elements not visible**: Check `width`, `height` (may be 0 if not in a layout), `visible`, `opacity`, and parent clipping.

3. **Layout sizing**: Elements outside layouts need explicit `width`/`height`. Inside layouts, they get sized automatically. Use `preferred-width`, `min-width`, `max-width` to constrain.

4. **Type mismatches**: `length` and `int`/`float` are different types. Use `1px * my_int` to convert, or `my_length / 1px` to get a number.

5. **Performance**: Use `ListView` (not `for` in `ScrollView`) for long lists because it virtualizes. Use `image-rendering: pixelated` only when needed. Avoid deeply nested opacity or clip layers.

### Debug Helpers

- `debug("message", expression)` prints to stderr at runtime
- `SLINT_DEBUG_PERFORMANCE=refresh_lazy,console` enables performance diagnostics
- Run with `SLINT_BACKEND=winit-skia` or other backend variants for testing

## MCP Server for AI-Assisted Debugging

Slint includes an embedded MCP (Model Context Protocol) server that lets you inspect and interact with a running Slint application in real time. The server provides tools for exploring the UI tree, taking screenshots, clicking elements, dragging, typing, and more.

Once enabled, an AI coding assistant can connect to the MCP endpoint to inspect and interact with the running UI.

### Enabling the MCP Server

**Step 1**: Build with `SLINT_EMIT_DEBUG_INFO=1` so that element IDs and source locations are preserved in the compiled output. Without this, elements will lack the debug metadata needed for meaningful introspection. Set `SLINT_MCP_PORT` to an available port when running, and pass `--features slint/mcp` to enable the server:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_MCP_PORT=9315 cargo run -p my-app --features slint/mcp
```

Do not add `mcp` to the `[features]` section of your `Cargo.toml` — use the `--features` flag on the command line instead.

**Step 2**: Connect to the running application's MCP server at `http://localhost:9315/mcp` using Streamable HTTP transport and use the available tools to inspect and interact with the UI.

When scripting or verifying the server from the command line, use `curl` — it is the most reliable approach for raw JSON-RPC. Prefer `curl` over built-in HTTP fetch tools, which agents sometimes reach for but which are less predictable for this use case:

```sh
# Initialize (confirms the server is up and prints available tools)
curl -s -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'

# List windows
curl -s -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_windows","arguments":{}}}'

# Take a screenshot (response contains a base64-encoded PNG in the "data" field)
curl -s -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"take_screenshot","arguments":{"windowHandle":{"index":"1","generation":"1"}}}}'
```

### Version Requirements

| Slint Version | MCP Support |
|---------------|-------------|
| < 1.17.0 | Not available |
| >= 1.17.0 | Enable via `--features slint/mcp` on the cargo command line |

### When to Suggest MCP

Suggest enabling the MCP server when the user is:
- Debugging layout or visual issues
- Trying to understand the runtime element hierarchy
- Testing interactions programmatically
- Verifying accessibility properties
- Diagnosing event handling problems

## Documentation Reference

Full documentation for the latest version is at https://slint.dev/docs. Key sections:
- Language guide: concepts, syntax, and coding patterns
- Reference: elements, properties, types, and standard widgets
- Language integrations: Rust, C++, Node.js, and Python API docs
- Tutorials: step-by-step guides for each language

For a specific Slint version, the documentation can be found at `https://releases.slint.dev/<version>/docs`, for example `https://releases.slint.dev/1.15.1/docs`.
