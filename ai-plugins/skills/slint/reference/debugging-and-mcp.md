# Debugging, Headless Rendering & the MCP Server

## Common Issues

- **Binding loops**: a property depends on itself through a chain; break the
  cycle with an intermediate property.
- **Element not visible**: check `width`/`height` (may be `0` outside a
  layout), `visible`, `opacity`, `clip`, and z-order (later siblings on top).
- **Performance**: use `ListView` for long lists — it virtualizes, while `for`
  inside a `ScrollView` instantiates every row. Keep `opacity`/`clip` layers
  flat; each nested level adds a render pass.
- **Blurry upscaled images**: `Image` smooths by default; for pixel art set
  `image-rendering: pixelated`.
- Compile errors and sizing problems: [gotchas.md](gotchas.md),
  [language-and-layout.md](language-and-layout.md).

## Debug Helpers

- `debug("msg", expr)` prints to stderr at runtime.
- `SLINT_DEBUG_PERFORMANCE=refresh_lazy,console` prints frame diagnostics.
- `SLINT_BACKEND=winit-skia` / `winit-femtovg` / `winit-software` selects the
  backend and renderer; `SLINT_BACKEND=headless` (needs `--features slint/mcp`)
  runs with no display at all — see the MCP section.
- `Window::take_snapshot()` (Rust) renders the window to a pixel buffer.

## Check a `.slint` file (`slint-viewer --check`)

`slint-viewer --check ui/main.slint` compiles the file and prints diagnostics
without opening a window `(1.17+)`: exit 1 on errors, 0 otherwise
(warnings still print). `-I`/`-L`/`--style` apply. Use this as the fast
per-file compile check; the host build is only needed for the interop side.

## Headless screenshots of a `.slint` file (`slint-viewer --screenshot`)

Renders a component to an image with no windowing system and no app code
`(1.17+)`:

```sh
slint-viewer --screenshot out.png ui/main.slint              # .png/.jpg, or - for stdout
slint-viewer --screenshot out.png --component MyCard ui/widgets.slint
slint-viewer --screenshot out.png --load-data props.json ui/main.slint
```

- With no `--backend`, the viewer uses its own headless software backend (no
  X/Wayland/Xvfb), renders at the component's preferred size, and exits.
  `--component` (default: last exported), `--style`, `-I`/`-L`, and
  `SLINT_SCALE_FACTOR` apply.
- `Error: take_snapshot() called on window with invalid size` means the
  component's preferred size is zero — give the root explicit
  `width`/`height`. (For compile checking alone, use `--check` instead.)
- `--load-data file.json` sets the root component's properties *and* `global`
  singletons — dot-qualify (`{"Theme.dark": true}`) or nest
  (`{"AppData": {"rows": [...]}}`). It runs no host-language logic, so for
  real application state use the MCP server or `Window::take_snapshot()`.

Rule of thumb: **viewer** for previewing components/layout/theme; **MCP
server** for the running app with real data and interactions.

When the assistant host can display local images inline, include the rendered
screenshot in the chat. In CLI-only hosts, print the absolute image path and
summarize what was visually checked.

## MCP Server for AI-Assisted Debugging

Slint ships an embedded MCP server `(1.17+)`: walk the UI tree, read
accessibility properties, screenshot, and simulate clicks, drags, and typing
in the running app — the best way to verify real interactions. (If
`--features slint/mcp` fails with "unknown feature", the Slint version is too
old.)

### Enabling

Always set `SLINT_EMIT_DEBUG_INFO=1` at *app* build time (preserves element
IDs/source locations) and `SLINT_MCP_PORT` to a free TCP port at run time (the
server binds `127.0.0.1:<port>` and logs `Slint MCP server listening on …`). The
`mcp` feature must also be compiled into the Slint library:

**Rust:** pass `--features slint/mcp` on the command line (do **not** put `mcp`
in `Cargo.toml`):

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_MCP_PORT=9315 cargo run --features slint/mcp
```

**C++:** released packages don't carry the `mcp` feature — build Slint from
source with `-DSLINT_FEATURE_MCP=ON`.

**Headless** (CI, container, agent sandbox — no display server or GPU): also
set `SLINT_BACKEND=headless`; the whole MCP toolset including
`take_screenshot` then works with no X/Wayland/Xvfb. Suffix a renderer
(`headless-software`, `headless-skia`) to force a specific rasterizer. If
`SLINT_BACKEND` is unset and the regular backend fails to initialize, the
headless backend kicks in as a fallback. It is an MCP-oriented entry point
(needs the `slint/mcp` feature) and the exact value may change between
releases:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_MCP_PORT=9315 SLINT_BACKEND=headless cargo run --features slint/mcp
```

**Node.js** (`slint-ui`): install the optional `slint-ui-dev` package as a dev
dependency at the same version as `slint-ui`. It ships a binary with the MCP
server (and system-testing) compiled in, which `slint-ui` loads automatically —
but only when `SLINT_MCP_PORT` (or `SLINT_TEST_SERVER`) is set, so ordinary runs
stay on the lean release binary. There is nothing to import from it; set the
variable before launching node:

```sh
npm install --save-dev slint-ui-dev
SLINT_EMIT_DEBUG_INFO=1 SLINT_MCP_PORT=9315 node app.js
```

**Python** (`slint`): install the optional `slint-dev` wheel at the same
version as `slint` (`uv add "slint[dev]"`). It carries the MCP-enabled binary,
which `slint` loads automatically — but only when `SLINT_MCP_PORT` (or
`SLINT_TEST_SERVER`) is set, so ordinary runs stay on the lean release binary.
Set the variable before importing slint (there is nothing to import from
`slint-dev`):

```sh
uv add "slint[dev]"
SLINT_EMIT_DEBUG_INFO=1 SLINT_MCP_PORT=9315 uv run app.py
```

Connect to the MCP endpoint (for example, `http://localhost:9315/mcp`) using
Streamable HTTP / JSON-RPC. When mentioning local MCP endpoints in chat or
status updates, always format them as inline code or inside a code block; never
write a bare URL, because some hosts auto-render `http://127.0.0.1...` as a web
preview even though the endpoint is not a browser UI. From a shell, `curl` is
the most reliable client — include the `Accept` header:

```sh
# List tools (confirms the server is up)
curl -s -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

# Screenshot — pipe to a file; the base64 payload breaks naive inline JSON parsing
curl -s -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"take_screenshot","arguments":{"windowHandle":{"index":"1","generation":"1"}}}}' > shot.json
```

### Tools (typical)

`list_windows`, `get_window_properties`, `get_element_tree`,
`get_element_properties`, `find_elements_by_id` (qualified id like
`MyComponent::my-button`), `query_element_descendants`, `take_screenshot`,
`click_element`, `drag_element`, `invoke_accessibility_action`,
`set_element_value`, `dispatch_key_event`, `start`/`stop_event_recording`.
Most take element/window handles returned by `list_windows`/the tree calls.

### Tips

- Give elements ids (`foo := Rectangle {}`), then target them via
  `find_elements_by_id` (`ComponentName::id`).
- Drive a flow (`click_element`, `dispatch_key_event`), then `take_screenshot`
  to verify the result.
- Claude Code reads a project-level `.mcp.json`; declaring the server there
  lets it attach automatically while the app runs:
  `{"mcpServers": {"my-app": {"type": "http", "url": "http://localhost:9315/mcp"}}}`.
