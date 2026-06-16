# Visual Editor Agent Notes

This directory contains the `.slint` sources for the visual editor UI.

## Default Runtime

Use the real `slint-editor` app. Do not use `slint-viewer` for visual-editor
work, because it bypasses the embedded editor/LSP plumbing.

Run the app with Skia:

```sh
SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 \
SLINT_BACKEND=winit-skia \
cargo run -p slint-lsp --example slint-editor \
  --no-default-features \
  --features backend-winit,renderer-skia,renderer-software,preview \
  -- examples/gallery/ui/pages/controls_page.slint
```

Important:

- Run GUI launches outside the sandbox when needed so the macOS window is
  actually visible.
- Use Skia. Do not silently switch to the software renderer. If Skia needs a
  first-time `skia-bindings` download, ask for network approval and keep using
  Skia.
- Use `-p slint-lsp --no-default-features`. The unqualified
  `cargo run --example slint-editor` route can pull Qt on macOS and fail in
  sandboxes when `ccache` writes under `~/Library/Caches/ccache`.
- `SLINT_ENABLE_EXPERIMENTAL_FEATURES=1` is required because the visual editor
  uses internal/experimental types such as `component-factory`.

## MCP

MCP is not the default launch path for this app. Do not change core/backend Rust
code just to make MCP reachable while launching the visual editor.

Only use MCP if the user explicitly asks for it. Then launch with:

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

If `http://127.0.0.1:9315/mcp` is not reachable, report that MCP is unavailable
for the current run. Do not switch to headless mode; this app is expected to run
as a visible GUI.

## Architecture

- Entry point: `tools/lsp/editor_main.rs` starts embedded LSP state, then calls
  `preview::run(..., use_editor_ui: true)`.
- UI creation: `tools/lsp/preview/ui.rs` creates `EditorUi`, wires the shared
  `Api` global, and registers callbacks.
- Live preview surface: `EditorCanvas` embeds the compiled target component via
  `ComponentContainer { component-factory: Api.preview-area; }`.
- Component drag/drop should use real editor callbacks:
  `Api.new-component-data`, `Api.can-drop`, and `Api.drop`.
- Resize/move should follow the existing preview-view path:
  `Api.selected-element-resize`, `Api.selected-element-can-move-to`, and
  `Api.selected-element-move`.

## Move, Resize, Rotate, And Key Handling

- For screen-space interactions, compare parent/window-space pointer positions,
  not raw local deltas.
- Do not use `self.mouse-x - press-x` from a `TouchArea` inside a rotated
  wrapper for move or resize.
- For rotated resize, reconstruct the parent/window-space pointer position from
  the rotated handle-local point, then convert that delta into local item axes
  using the press-time rotation.
- Keep final bounds and minimum-size clamping in `EditorState`.
- If a pointer interaction depends on keyboard state, focus the editor
  `FocusScope` when the interaction starts. Use `Key.Shift` and `Key.ShiftR` in
  `capture-key-pressed` / `capture-key-released` for live Shift state.
