<!-- cSpell: ignore slotmap -->
# Embedded MCP Server

The testing backend includes an embedded [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server that allows MCP-compatible clients (e.g. Claude Code) to inspect and interact with a running Slint application over HTTP. This document covers the architecture and internals for developers working on `internal/backends/testing/`.

## Overview

The MCP server shares a common introspection layer with the system-testing (protobuf/TCP) transport. Both transports use the same `IntrospectionState` for window and element tracking, the same protobuf-derived types for data structures, and the same `ElementHandle` API for interacting with the UI. The MCP transport adds a thin JSON-RPC/HTTP wrapper on top.

```
┌─────────────────────────────────────────────┐
│         Slint Application (event loop)      │
├──────────────────┬──────────────────────────┤
│                  │  introspection/           │
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

1. **Cargo feature `mcp`** — Compiles the MCP server code. Defined in `internal/backends/testing/Cargo.toml`, forwarded through `internal/backends/selector/Cargo.toml`, and exposed as the public `slint/mcp` feature.

2. **Environment variables** — `SLINT_MCP_PORT` controls whether the server actually starts at runtime. If not set, `mcp_server::init()` returns immediately with no overhead. `SLINT_MCP_HOST` sets the bind address and defaults to `127.0.0.1`.

### Enabling for a Slint Application

See the [README](../../internal/backends/testing/README.md#enabling-the-mcp-server) for setup instructions.

## Initialization Flow

Initialization is triggered from `init_testing_backends()` in the backend selector (`internal/backends/selector/lib.rs`) after the platform is successfully created:

1. `mcp_server::init()` checks `SLINT_MCP_PORT`. If absent, returns early.
2. Calls `introspection::ensure_window_tracking()` to install a window-shown hook that registers windows with the shared `IntrospectionState`.
3. Installs a second window-shown hook that lazily starts the TCP listener on the first window show. The server task is spawned onto the Slint event loop via `context.spawn_local()`.

The lazy start via `OnceCell` ensures the server only binds the port once the application has an event loop running and a window to inspect.

## Shared Introspection Layer (`introspection/`)

### IntrospectionState

The central data structure, stored as a thread-local `Rc<IntrospectionState>`. `windows` and
`element_handles` are keyed by a `slotmap`-based `ArenaIndex` (a generational key type
defined via `slotmap::new_key_type!`), not the `generational_arena` crate:

- **`windows`** — `RefCell<SlotMap<ArenaIndex, TrackedWindow>>`: tracks live windows via weak references to their `WindowAdapter`.
- **`element_handles`** — `RefCell<SlotMap<ArenaIndex, TrackedElement>>`: maps arena indices to `ElementHandle` instances, each with the tick of its last use.
- **`element_handle_clock`** — `Cell<u64>`: the tick, counted up on each use of a handle.

### Handle System

Both transports use `ArenaIndex` (a `slotmap` key) internally. The proto `Handle` type (`{index, generation}`) is the wire format — `index_to_handle()` and `handle_to_index()` convert between them.

Handles are generational: if an element is evicted and its arena slot reused, stale handles are detected because the generation won't match.

Window handles and element handles share the same `{index, generation}` shape, so clients (especially LLM agents) easily confuse them. To disambiguate, `tool_definitions()` gives the `windowHandle` and `elementHandle` parameters distinct `description`s, and the `initialize` instructions spell out that the two kinds are not interchangeable and where each one comes from.

### Eviction

The element arena is capped at 10,000 entries (`ELEMENT_HANDLE_CAP`).
A handle is used when a reply returns it, and when a request looks it up.
When a new handle takes the arena past the cap, the least recently used handles are evicted,
until the arena is an eighth under the cap (`ELEMENT_HANDLE_EVICTION_SLACK`).
Root element handles for tracked windows are never evicted.

### Validity Checking

When a handle is resolved via `IntrospectionState::element()`, the returned `ElementHandle` is checked with `is_valid()`. If the underlying UI element has been destroyed (e.g. the component was removed), the stale handle is cleaned up and an error is returned.

### Testable Properties

`element_properties()` fills `testableProperties` from `ElementHandle::testable_properties()`
and `testable_property_value()`.
They read the compiler-emitted per-element property table
through `ItemTreeVTable::element_testable_properties` and `element_testable_property_value`
(see `SubComponent::testable_properties` in the compiler's LLR).
Values cross the vtable as strings in the `i_slint_core::debug_info` encoding;
`mcp_server.rs::convert_testable_property_values` retypes them into JSON booleans and
numbers for both `get_element_properties` and `get_element_tree`.
`query_element_descendants` compares against the same strings,
so `stringify_testable_property_matches` turns typed `matchElementTestableProperty` values back into them.
The table exists only when the application was compiled with debug info.
An item tree whose generator doesn't implement the entries reports "unsupported",
which surfaces as a `note` instead of an empty list.
Only properties declared with the experimental
[`@testable` attribute](../astro/src/content/docs/guide/experimental/testable.mdx) are in the table,
whatever their visibility.
The compiler keeps them and their bindings alive, so the listing never depends on the optimizer.

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

- **Bind address**: the server binds to `127.0.0.1` by default. Set `SLINT_MCP_HOST` to bind another address, such as `0.0.0.0`, when clients connect from outside the machine.
- **Origin validation**: cross-origin requests from non-localhost origins are rejected with 403. This restricts browser clients to the same machine; non-browser clients send no `Origin` header and are unaffected.
- **No authentication**: the server is intended for development and testing, so there is no auth mechanism.

### Tool Dispatch

Tool calls arrive as `tools/call` JSON-RPC methods. The `handle_tool_call()` function dispatches by tool name. All tools deserialize parameters into proto request types (leveraging `pbjson`-generated `Deserialize` impls), call methods on `IntrospectionState`, and serialize the response back to JSON.

### MCP Instructions

The `initialize` response includes a detailed `instructions` field that guides MCP clients through the workflow, handle format, and enum values. This is the primary documentation that AI clients see when connecting.
The query syntax is in `query_element_descendants`' input schema, so a client that lists the tools reads it once.
`QUERY_STACK_FIELD_DESCRIPTIONS` adds only what the schema's field names and enums don't say, such as the id format.

### Scrolling Into View

`query_element_descendants` with `scrollIntoView` also searches content clipped by Flickables.
For virtualized lists, it pages through each Flickable and searches nested lists before advancing the outer page.
This keeps off-screen inner lists instantiated while their rows are searched.
A search without a match restores scroll positions in surviving Flickables.
Paging can recreate virtualized rows, invalidating their handles and resetting state stored in those rows.

### Snapshot

`get_element_snapshot` lists every window, or the subtree of an element, as text, one line per element, so an agent can explore without the token cost of `get_element_tree`.
It lists the elements on screen that have an accessible role or `@testable` properties.
The std widgets set `accessible-role: none` on their inner elements, so a `Button` is one line, not the elements it's built from.
An input handler, such as a `TouchArea` or `Flickable`, is listed only when no element with a role contains it.
That's what a hand-made control looks like.
Its caption, a `Text` that repeats the label of the element it's listed under, is left out.
The compiler binds `accessible-enabled` to `enabled` only for `TextInput`,
so the snapshot reads `enabled` of a `TouchArea` or gesture handler itself to mark it `disabled`.

Lines are indented under their nearest listed ancestor, found the same way as `get_element_tree`'s `parentHandle`.
The walk visits a popup's contents before the element it's attached to, so the listing is reordered to follow these links.
Only listed elements get a handle, so a snapshot evicts far fewer handles than a large `get_element_tree`.

The action tools take an MCP-only `snapshot` argument, marked by `ToolDef::offers_snapshot`,
and `get_element_snapshot` takes an MCP-only `rectangles` argument.
Both are removed before decoding the proto request.
After an action, `snapshot` appends a snapshot of every window as a second text block.
`rectangles` ends each line in the element's rectangle.
It's opt-in, since agents act by handle and rectangles make up a large part of a snapshot.

## Proto Build Pipeline (`build.rs`)

Both `system-testing` and `mcp` features trigger the same build pipeline:

1. `protox` compiles `slint_systest.proto` (pure-Rust, no external `protoc` needed)
2. `prost-build` generates Rust structs from the proto descriptors → `proto.rs`
3. `pbjson-build` generates `Serialize`/`Deserialize` impls → `proto.serde.rs`

The MCP transport uses the `serde_json`-based serialization, while the system-testing transport uses prost's binary encoding. Both share the same proto types.

## Verifying `take_screenshot` Results

`SLINT_BACKEND` selects a renderer by name (e.g. `winit-skia`), but an unavailable renderer falls back silently instead of erroring — a binary built without `renderer-skia` compiled in still accepts `SLINT_BACKEND=winit-skia` and renders with whatever renderer it does have. Before trusting a renderer-specific screenshot result, confirm the renderer actually linked into the binary:

```sh
nm -gU <binary> | grep -c i_slint_renderer_skia   # 0 means it isn't compiled in
```

Also confirm the state change you're checking for actually reached the application (e.g. read back an accessible property) before treating "no pixel diff" as a rendering bug rather than a stale or unaffected snapshot.

## Adding a New Tool

1. Add request and response message types to `slint_systest.proto`. The build pipeline will auto-generate the JSON schema for the MCP tool's `inputSchema`.
2. Add a `ToolDef` entry to the `TOOLS` table in `mcp_server.rs` with name, description, proto request type, and optional fields.
   Set `offers_snapshot` if the tool acts on the UI.
3. Add a match arm in `handle_tool_call()`.
4. If the tool needs new introspection capabilities, add methods to `IntrospectionState` in `introspection/mod.rs` so both transports can use them.
5. Update the `instructions` string in the `initialize` response if the new tool changes the recommended workflow.

## Key Files

| File | Purpose |
|------|---------|
| `internal/backends/testing/introspection/mod.rs` | Shared `IntrospectionState`, arena management, window/element operations |
| `internal/backends/testing/introspection/proto.rs` | Conversions between internal state and protobuf-derived types |
| `internal/backends/testing/mcp_server.rs` | HTTP server, JSON-RPC dispatch, MCP tool definitions |
| `internal/backends/testing/systest.rs` | System-testing TCP/protobuf transport (shares introspection layer) |
| `internal/backends/testing/slint_systest.proto` | Protobuf definitions (source of truth for data types) |
| `internal/backends/testing/build.rs` | Proto compilation pipeline |
| `internal/backends/selector/api.rs` | Backend initialization, MCP server startup hook |
