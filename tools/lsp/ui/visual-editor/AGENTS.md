# Visual Editor Agent Notes

This directory contains the `.slint` sources for the visual editor UI.
When you need to inspect the UI at runtime, use an MCP-enabled `slint-viewer`.

## MCP-Enabled Viewer

Build `slint-viewer` with Skia and the MCP backend selector feature:

```sh
SLINT_EMIT_DEBUG_INFO=1 cargo build -p slint-viewer --no-default-features --features backend-default,renderer-skia,renderer-software,i-slint-backend-selector/mcp
```

Use `--no-default-features` so the viewer doesn't pull in the `remote` feature.
On macOS, that avoids the Bonjour/libclang path, which isn't needed for MCP.
The first Skia build may download `skia-bindings` binaries.

Launch the visual editor through the viewer like this:

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
SLINT_EMIT_DEBUG_INFO=1 \
SLINT_MCP_PORT=9315 \
SLINT_BACKEND=winit-skia \
target/debug/slint-viewer --component EditorUi tools/lsp/ui/main.slint
```

`SLINT_ENABLE_EXPERIMENTAL_FEATURES=1` is required because the visual editor uses
internal/experimental types such as `component-factory`.
Without it, `slint-viewer` fails with `Unknown type 'component-factory'`.

## MCP Calls

The embedded MCP server listens at:

```text
http://127.0.0.1:9315/mcp
```

List windows:

```sh
curl -s -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_windows","arguments":{}}}'
```

Take a screenshot of the first window:

```sh
curl -s -o /private/tmp/slint-mcp-shot.json -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"take_screenshot","arguments":{"windowHandle":{"index":"1","generation":"1"}}}}'
```

Decode the returned image payload:

```sh
node -e "const fs=require('fs'); const j=JSON.parse(fs.readFileSync('/private/tmp/slint-mcp-shot.json','utf8')); const c=j.result.content.find(x=>x.type==='image'); fs.writeFileSync('/private/tmp/slint-mcp-shot.png', Buffer.from(c.data,'base64'));"
```

## Useful Interaction Flow

Use `get_window_properties` to get the root element handle.
Then use `get_element_tree` to locate element handles for controls.
For example, the orientation controls are `OrientationIconButton` entries in
the canvas toolbar; call `click_element` on the matching `TouchArea` handle to
switch between the portrait and landscape editor states.

After every interaction, call `take_screenshot` again and inspect the image.
If the client supports it always then show the image inline in the chat.

## Move, Resize, Rotate, and Key Handling

The visual editor uses transformed selection chrome. Be careful with pointer
coordinates:

- Use absolute pointer coordinates for screen-space interactions such as moving
  items and ordinary corner resize. Store the absolute pointer position on press
  with `self.absolute-position.x + self.mouse-x` / `self.absolute-position.y +
  self.mouse-y`, then compare later absolute pointer positions against that.
- Do not use `self.mouse-x - press-x` from a `TouchArea` inside a rotated wrapper
  for move or resize. Those local coordinates are affected by the transform and
  have repeatedly broken normal drag/reposition behavior.
- For rotated resize, convert the absolute screen-space delta into local item
  axes using the inverse rotation before applying width/height changes. At
  `0deg`, the math must match the old unrotated resize behavior exactly.
- Keep final bounds and minimum-size clamping in `EditorState`; `MoveResizeFrame`
  should emit requested geometry and let state clamp it.
- If a pointer interaction depends on keyboard state, focus the editor
  `FocusScope` when the interaction starts. Use `Key.Shift` and `Key.ShiftR` in
  `capture-key-pressed` / `capture-key-released` for live Shift state, then pass
  that bool down. `event.modifiers.shift` on `pointer-event` is only a fallback
  snapshot; it does not update when the user presses or releases Shift without a
  pointer event.
- Use `KeyBinding` / `@keys(...)` for one-shot shortcuts. Use
  press/release state for modal interactions such as proportional resize,
  rotation snapping, and radius editing.

Before declaring changes in this area done, manually verify all of these in the
MCP viewer: drag-to-reposition, normal corner resize, Shift-proportional resize,
rotation, Shift-rotation snapping, and radius editing.
