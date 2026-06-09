# Debugging, Headless Rendering & the MCP Server

## Common Issues

1. **Binding loops**: a property depends on itself through a chain. The compiler
   warns; break the cycle with an intermediate property.
2. **Elements not visible**: check `width`/`height` (may be `0` outside a layout),
   `visible`, `opacity`, `clip`, and z-order (later siblings render on top).
3. **Layout sizing**: elements outside layouts need explicit `width`/`height`;
   custom components/layouts may need `width/height: 100%` to fill (see
   `reference/language-and-layout.md`).
4. **Type mismatches**: `length` vs number — convert with `* 1px` / `/ 1px`.
5. **Ignored `padding`/`spacing`**: only effective on layout elements.
6. **Performance**: use `ListView` for long lists — it virtualizes, while
   `for` inside a `ScrollView` instantiates every row. Keep `opacity`/`clip`
   layers flat — each nested level adds a render pass.
7. **Blurry upscaled images**: `Image` smooths by default. For pixel art /
   nearest-neighbor set `image-rendering: pixelated`.

## Debug Helpers

- `debug("msg", expr)` prints to stderr at runtime.
- `SLINT_DEBUG_PERFORMANCE=refresh_lazy,console` prints frame/perf diagnostics.
- `SLINT_BACKEND=winit-skia` / `winit-femtovg` / `winit-software` selects the
  renderer. **`winit-software` is the choice for headless/CI/GPU-less machines**;
  on headless Linux run under `xvfb-run -a -s "-screen 0 1360x900x24"` (the winit
  X11 path needs `libxkbcommon-x11`).
- `Window::take_snapshot()` (Rust) renders the window to a pixel buffer — a quick
  screenshot from inside your app.

## Screenshotting a `.slint` file headlessly (`slint-viewer --screenshot`)

The simplest way to render a component to an image with **no windowing system and
no app code** (Slint >= 1.17):

```sh
slint-viewer --screenshot out.png ui/main.slint              # .png/.jpg, or - for stdout
slint-viewer --screenshot out.png --component MyCard ui/widgets.slint
slint-viewer --screenshot out.png --load-data props.json ui/main.slint
```

- With no `--backend`, the viewer installs its own headless software backend (no
  X/Wayland/Xvfb), renders at the component's preferred size, and exits.
  `--component` (default: last exported), `--style`, `-I`/`-L`, and
  `SLINT_SCALE_FACTOR` all apply.
- `--load-data file.json` sets the **root component's** properties *and* `global`
  singletons. Either dot-qualify (`{"Theme.dark": true, "AppData.rows": [...]}`)
  or nest (`{"AppData": {"rows": [...]}}`). It runs no host-language logic, so
  callbacks aren't invoked — for real application state with side effects, use
  the MCP server or `Window::take_snapshot()`.
- Install: `cargo install slint-viewer` (add `--git https://github.com/slint-ui/slint`
  for unreleased builds); `--no-default-features --features renderer-software`
  avoids GPU/windowing deps.

Rule of thumb: **viewer** for previewing components/layout/theme; **MCP server**
for the running app with real data and interactions.

## MCP Server for AI-Assisted Debugging

Slint **1.17+** ships an embedded MCP server (older releases have no `mcp`
feature — if `--features slint/mcp` fails with "unknown feature", upgrade)
that lets an AI assistant inspect and *drive* a running app in real time: walk
the UI tree, read accessibility properties, screenshot, and simulate clicks,
drags, typing, and keys — the best way to verify real interactions, not just
static rendering.

### Enabling

Build with `SLINT_EMIT_DEBUG_INFO=1` (preserves element IDs/source locations for
introspection), set `SLINT_MCP_PORT`, and pass `--features slint/mcp` on the
command line (do **not** put `mcp` in `Cargo.toml`'s `[features]`):

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_MCP_PORT=9315 cargo run --features slint/mcp
```

**Headless options:**
- *Need screenshots*: software renderer under a virtual display —
  `SLINT_BACKEND=winit-software xvfb-run -a -s "-screen 0 1360x900x24" cargo run --features slint/mcp`.
- *Inspection/interaction only, no display*: the MCP server is also hosted by
  Slint's windowless **testing backend** (`SLINT_BACKEND=testing`). Element queries,
  `click_element`, `dispatch_key_event`, etc. work with no X/Wayland — but its
  renderer is a stub, so **`take_screenshot` returns "not implemented by the
  platform"**. (The `slint` crate doesn't re-export the selector's
  `backend-testing` feature, so this currently needs a dep on
  `i-slint-backend-selector` with that feature.)

Connect to `http://localhost:9315/mcp` (Streamable HTTP / JSON-RPC). `curl` is the
most reliable shell client — include the `Accept` header:

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
`set_element_value`, `dispatch_key_event`, `start`/`stop_event_recording`. Most
take element/window handles returned by `list_windows`/the tree calls.

### Tips

- Give elements ids (`foo := Rectangle {}`) + build with `SLINT_EMIT_DEBUG_INFO=1`,
  then target via `find_elements_by_id` (`ComponentName::id`).
- Drive a flow (`click_element`, `dispatch_key_event`), then `take_screenshot` to
  verify the result.
- A `.mcp.json` with an HTTP server at `http://localhost:9315/mcp` lets Claude Code
  attach automatically while the app runs.

Use it when debugging layout/visual issues, exploring the runtime hierarchy,
testing interactions, or verifying accessibility/event handling.
