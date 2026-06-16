# Visual Editor Agent Notes

This directory contains the `.slint` sources for the visual editor UI.
When you need to inspect the UI at runtime, use the real `slint-editor`
example with MCP enabled. Do not use `slint-viewer` for this area; it bypasses
the embedded editor/LSP plumbing that drag, drop, move, and resize now depend
on.

## MCP-Enabled Real Editor

Launch the real editor against the gallery controls page:

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
SLINT_EMIT_DEBUG_INFO=1 \
SLINT_MCP_PORT=9315 \
SLINT_BACKEND=winit-skia \
cargo run -p slint-lsp --example slint-editor \
  --no-default-features \
  --features backend-winit,renderer-skia,renderer-software,preview,slint/mcp \
  -- examples/gallery/ui/pages/controls_page.slint
```

Use `-p slint-lsp --no-default-features` on macOS. The unqualified
`cargo run --example slint-editor` route can unify in workspace/default
features, pull Qt, and then fail in sandboxes when `ccache` tries to write under
`~/Library/Caches/ccache`.

`SLINT_ENABLE_EXPERIMENTAL_FEATURES=1` is required because the visual editor uses
internal/experimental types such as `component-factory`.
Without it, the editor fails with `Unknown type 'component-factory'`.

The first Skia build may download or compile `skia-bindings` artifacts. If the
app stays alive but `127.0.0.1:9315` refuses connections, investigate MCP
startup through `i-slint-backend-selector` and the testing backend's
window-shown hook before doing visual editor interaction work.

## MCP Calls

The embedded MCP server listens at:

```text
http://127.0.0.1:9315/mcp
```

List MCP tools first. This confirms the server is reachable:

```sh
curl -s -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

List windows next. This confirms the real `EditorUi` window is tracked:

```sh
curl -s -X POST http://127.0.0.1:9315/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_windows","arguments":{}}}'
```

Take a screenshot of the first window. This confirms the real app rendered:

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

## Real Editor Architecture

- Entry point: `tools/lsp/editor_main.rs` starts the embedded LSP state, then
  calls `preview::run(..., use_editor_ui: true)`.
- UI creation: `tools/lsp/preview/ui.rs` creates `EditorUi`, wires the shared
  `Api` global, and registers callbacks.
- Live preview surface: `EditorCanvas` embeds the compiled target component via
  `ComponentContainer { component-factory: Api.preview-area; }`.
- Component drag/drop should use real editor callbacks: palette rows produce
  `DataTransfer` via `Api.new-component-data(component.index)`, canvas
  `DropArea` calls `Api.can-drop(...)` and `Api.drop(...)`, and Rust handles
  those in `preview::can_drop_component` and `preview::drop_component`.
- Resize/move should follow the existing preview-view path: selection chrome
  emits geometry, UI calls `Api.selected-element-resize`,
  `Api.selected-element-can-move-to`, and `Api.selected-element-move`, and Rust
  applies edits through `resize_selected_element` and `move_selected_element`.

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

- For screen-space interactions, compare parent/window-space pointer positions,
  not raw local deltas. If the hit target is inside the rotated selection wrapper,
  convert `self.mouse-x` / `self.mouse-y` back through the same transform helpers
  that draw the chrome before comparing against the captured press pointer.
- Do not use `self.mouse-x - press-x` from a `TouchArea` inside a rotated wrapper
  for move or resize. Those local coordinates are affected by the transform and
  have repeatedly broken normal drag/reposition behavior.
- For rotated resize, reconstruct the parent/window-space pointer position from
  the rotated handle-local point, then convert that delta into local item axes
  using the press-time rotation before applying width/height changes. At `0deg`,
  the math must match the old unrotated resize behavior exactly.
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
real editor via MCP: drag-to-reposition, normal corner resize,
Shift-proportional resize, rotation, Shift-rotation snapping, and radius
editing.
