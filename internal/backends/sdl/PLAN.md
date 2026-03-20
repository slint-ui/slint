# SDL3 Backend Implementation Plan

## Goal
Add a built-in SDL3 backend + renderer for Slint, using SDL_Renderer for drawing
and SDL_ttf (3.x renderer-based API) for text. Targeted at C++ game developers
who want Slint UI in their SDL3 games.

## Architecture

```
internal/backends/sdl/
├── Cargo.toml          - [DONE] Package definition
├── build.rs            - [DONE] Links SDL3 + SDL3_ttf via pkg-config
├── sdl3_bindings.rs    - [DONE] Minimal FFI for SDL3 + SDL_ttf
├── fonts.rs            - [DONE] Font loading/caching/measurement via SDL_ttf
├── renderer.rs         - [DONE] ItemRenderer impl using SDL_Renderer
├── lib.rs              - [DONE] Platform impl, WindowAdapter, RendererSealed, public API
└── PLAN.md             - This file
```

## Status: Initial implementation complete

All files compile cleanly. The backend is integrated into:
- Workspace Cargo.toml (member + dependency)
- Backend selector (Cargo.toml, lib.rs, api.rs)
- C++ crate (Cargo.toml feature, CMakeLists.txt option)

### Activation
- Set `SLINT_BACKEND=sdl` environment variable, or
- Enable `backend-sdl` cargo feature, or
- Use `SLINT_FEATURE_BACKEND_SDL=ON` in CMake

### Runtime requirements
- SDL3 (`libSDL3`) and SDL3_ttf (`libSDL3_ttf`) must be installed on the system
- Found via pkg-config at build time, or linked by name as fallback

## Key traits implemented

1. **`Platform`** (i_slint_core::platform) — in lib.rs
   - `create_window_adapter()` → creates SDL window + renderer
   - `run_event_loop()` / `process_events()` — SDL event pump
   - `duration_since_start()` — Rust Instant
   - Clipboard via SDL clipboard API

2. **`WindowAdapter`** (i_slint_core::window) — in lib.rs
   - Wraps SDL_Window, manages size/position/visibility
   - `renderer()` returns our RendererSealed impl
   - `request_redraw()` pushes SDL user event

3. **`RendererSealed`** (i_slint_core::renderer) — in lib.rs
   - Text measurement via SDL_ttf (TTF_GetStringSize)
   - Font metrics via TTF_GetFontAscent/Descent/Height
   - `set_window_adapter()` / `window_adapter()`

4. **`ItemRenderer`** (i_slint_core::item_rendering) — in renderer.rs
   - `draw_rectangle` → SDL_RenderFillRect
   - `draw_border_rectangle` → filled rect + outline
   - `draw_image` → create SDL_Texture from pixel data, SDL_RenderTexture
   - `draw_text` → SDL_ttf TTF_RenderText_Blended → texture → render
   - `draw_text_input` → same as text + cursor rect + selection
   - `draw_path` → NOT IMPLEMENTED (log warning)
   - `draw_box_shadow` → NOT IMPLEMENTED (log debug)
   - `combine_clip` → SDL_SetRenderClipRect
   - `save_state`/`restore_state` → stack of (clip, translation, opacity)
   - `translate` → offset tracking
   - `apply_opacity` → tracked in state, applied to textures
   - `rotate`/`scale` → NOT IMPLEMENTED (log debug)

## Game integration

- Pre-render callback: `backend.set_pre_render_callback(|renderer_ptr| { ... })`
  Called before Slint UI rendering so the game can draw its content using
  the raw `SDL_Renderer*` pointer.
- `process_events()` support for embedding in game loop.
- `sdl_renderer_ptr()` returns the raw `SDL_Renderer*` for C++ interop.
- C FFI functions for C++ games:
  - `slint_sdl_set_pre_render_callback(cb, user_data, drop)` — set game render callback
  - `slint_sdl_get_renderer()` — get `SDL_Renderer*`
  - `slint_sdl_get_window()` — get `SDL_Window*`

## Example: `examples/sdl_underlay/`

Both Rust and C++ versions demonstrate a game rendering animated rectangles
with SDL_Renderer, with a semi-transparent Slint UI overlaid on top.
- `main.cpp` + `CMakeLists.txt` — C++ version using the C FFI
- `main.rs` + `Cargo.toml` — Rust version using the same C FFI

## Features NOT implemented (with rationale and implementation notes)

- **Gradients** (linear, radial, conic): SDL_Renderer has no gradient primitive.
  Implementation would require rasterizing the gradient to a texture on the CPU.
  Current fallback: renders as the solid color of the first gradient stop.

- **Paths**: SDL_Renderer has no path/bezier primitive. Would need a rasterization
  library (e.g., lyon or zeno) to tessellate paths into triangles or rasterize to a
  pixel buffer, then upload as a texture.

- **Box shadows**: Requires Gaussian blur which SDL_Renderer doesn't support.
  Would need to render the shadow shape to an offscreen texture, apply a multi-pass
  Gaussian blur (horizontal + vertical), then render the blurred texture at offset.

- **Rotation/Scale transforms**: SDL_RenderTextureRotated supports texture
  rotation, but general item sub-tree rotation needs render-to-texture (via
  SDL_SetRenderTarget). Could be added incrementally.

- **Rounded rectangle rendering**: SDL_RenderFillRect only draws sharp rectangles.
  A full implementation would use SDL_RenderGeometry with triangulated corner arcs,
  or rasterize rounded corners to a texture. Currently draws sharp corners.

- **Rounded rectangle clipping**: SDL_SetRenderClipRect only supports axis-aligned
  rectangles. Rounded clipping would need per-pixel masking via an offscreen texture.

- **Layer compositing**: Would need render-to-texture (SDL_SetRenderTarget) for
  correct isolation and blending. Currently renders layers inline.
