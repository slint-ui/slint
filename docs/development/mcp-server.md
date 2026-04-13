# Embedded MCP Server

The testing backend includes an embedded [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server that allows MCP-compatible clients (e.g. Claude Code) to inspect and interact with a running Slint application over HTTP. This document covers the architecture and internals for developers working on `internal/backends/testing/`.

## Overview

The MCP server shares a common introspection layer with the system-testing (protobuf/TCP) transport. Both transports use the same `IntrospectionState` for window and element tracking, the same protobuf-derived types for data structures, and the same `ElementHandle` API for interacting with the UI. The MCP transport adds a thin JSON-RPC/HTTP wrapper on top.

```
┌─────────────────────────────────────────────┐
│         Slint Application (event loop)      │
├──────────────────┬──────────────────────────┤
│                  │  introspection.rs         │
│                  │  IntrospectionState       │
│                  │  (window/element arenas)  │
│       ┌──────────┴──────────┐               │
│       │                     │               │
│  systest.rs            mcp_server.rs        │
│  (TCP/protobuf)        (HTTP/JSON-RPC)      │
│  system-testing        mcp feature          │
│  feature                                    │
└───────┴─────────────────────┴───────────────┘
```

## Feature Gating

The MCP server is controlled by two layers:

1. **Cargo feature `mcp`** — Compiles the MCP server code. Defined in `internal/backends/testing/Cargo.toml` and forwarded through `internal/backends/selector/Cargo.toml`. Not currently exposed through the public `slint` crate.

2. **Environment variable `SLINT_MCP_PORT`** — Controls whether the server actually starts at runtime. If not set, `mcp_server::init()` returns immediately with no overhead.

### Enabling for a Slint Application

Since the `mcp` feature is not plumbed through the public `slint` crate, users enable it directly on the backend selector:

```toml
[dependencies]
slint = "x.y.z"
i-slint-backend-selector = { version = "=x.y.z", features = ["mcp"] }
```

Then run with:

```sh
SLINT_MCP_PORT=8080 cargo run -p my-app
```

## Initialization Flow

Initialization is triggered from the backend selector (`internal/backends/selector/lib.rs`) after the platform is successfully created:

1. `mcp_server::init()` checks `SLINT_MCP_PORT`. If absent, returns early.
2. Calls `introspection::ensure_window_tracking()` to install a window-shown hook that registers windows with the shared `IntrospectionState`.
3. Installs a second window-shown hook that lazily starts the TCP listener on the first window show. The server task is spawned onto the Slint event loop via `context.spawn_local()`.

The lazy start via `OnceCell` ensures the server only binds the port once the application has an event loop running and a window to inspect.

## Shared Introspection Layer (`introspection.rs`)

### IntrospectionState

The central data structure, stored as a thread-local `Rc<IntrospectionState>`:

- **`windows`** — `Arena<TrackedWindow>`: tracks live windows via weak references to their `WindowAdapter`.
- **`element_handles`** — `Arena<ElementHandle>`: maps arena indices to `ElementHandle` instances.
- **`element_handle_order`** — `VecDeque<Index>`: tracks insertion order for FIFO eviction.

### Handle System

Both transports use `generational_arena::Index` internally. The proto `Handle` type (`{index, generation}`) is the wire format — `index_to_handle()` and `handle_to_index()` convert between them.

Handles are generational: if an element is evicted and its arena slot reused, stale handles are detected because the generation won't match.

### FIFO Eviction

The element arena is capped at 10,000 entries (`ELEMENT_HANDLE_CAP`). When the cap is exceeded, the oldest handles are evicted (FIFO order), with one exception: root element handles for tracked windows are never evicted — they are pushed to the back of the queue instead.

### Validity Checking

When a handle is resolved via `IntrospectionState::element()`, the returned `ElementHandle` is checked with `is_valid()`. If the underlying UI element has been destroyed (e.g. the component was removed), the stale handle is cleaned up and an error is returned.

## MCP Transport (`mcp_server.rs`)

### Protocol

The server implements MCP's [Streamable HTTP transport](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#streamable-http):

- Endpoint: `POST /mcp` (or `POST /`)
- Content-Type: `application/json`
- JSON-RPC 2.0 messages

The server is stateless (no session management). Each request is a single JSON-RPC call — batch requests are rejected.

### HTTP Server

The HTTP server is built directly on `async-net` (async TCP) and `httparse` (HTTP/1.1 parsing), with no framework dependency. It supports:

- HTTP/1.1 keep-alive (persistent connections)
- CORS preflight (`OPTIONS`) for browser-based clients
- Origin validation: only `localhost`, `127.0.0.1`, and `::1` origins are accepted
- 4 MB maximum body size

### Security

- **Localhost only**: the server binds to `127.0.0.1`, not `0.0.0.0`.
- **Origin validation**: cross-origin requests from non-localhost origins are rejected with 403.
- **No authentication**: since the server is localhost-only and intended for development/testing, there is no auth mechanism.

### Tool Dispatch

Tool calls arrive as `tools/call` JSON-RPC methods. The `handle_tool_call()` function dispatches by tool name. Most tools deserialize parameters into proto types (leveraging `pbjson`-generated `Deserialize` impls), call methods on `IntrospectionState`, and serialize the response back to JSON.

Two tools (`get_element_tree` and `dispatch_key_event`) use custom parameter handling rather than proto types, since they don't have direct protobuf equivalents.

### MCP Instructions

The `initialize` response includes a detailed `instructions` field that guides MCP clients through the workflow, handle format, enum values, and query syntax. This is the primary documentation that AI clients see when connecting.

## Proto Build Pipeline (`build.rs`)

Both `system-testing` and `mcp` features trigger the same build pipeline:

1. `protox` compiles `slint_systest.proto` (pure-Rust, no external `protoc` needed)
2. `prost-build` generates Rust structs from the proto descriptors → `proto.rs`
3. `pbjson-build` generates `Serialize`/`Deserialize` impls → `proto.serde.rs`

The MCP transport uses the `serde_json`-based serialization, while the system-testing transport uses prost's binary encoding. Both share the same proto types.

## Adding a New Tool

1. If the tool maps to a proto request/response, add the message types to `slint_systest.proto`. Otherwise, handle parameters manually in `handle_tool_call()`.
2. Add a tool definition entry in `tool_definitions()` with name, description, and input schema.
3. Add a match arm in `handle_tool_call()`.
4. If the tool needs new introspection capabilities, add methods to `IntrospectionState` in `introspection.rs` so both transports can use them.
5. Update the `instructions` string in the `initialize` response if the new tool changes the recommended workflow.

## Key Files

| File | Purpose |
|------|---------|
| `internal/backends/testing/introspection.rs` | Shared `IntrospectionState`, arena management, window/element operations |
| `internal/backends/testing/mcp_server.rs` | HTTP server, JSON-RPC dispatch, MCP tool definitions |
| `internal/backends/testing/systest.rs` | System-testing TCP/protobuf transport (shares introspection layer) |
| `internal/backends/testing/slint_systest.proto` | Protobuf definitions (source of truth for data types) |
| `internal/backends/testing/build.rs` | Proto compilation pipeline |
| `internal/backends/selector/lib.rs` | Backend initialization, MCP server startup hook |
