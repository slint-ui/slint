# Custom Renderer Implementation Guide

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `internal/renderers/`, adding
> rendering backends, fixing drawing bugs, or implementing custom graphics output.
> For general build commands and project structure, see `/AGENTS.md`.

This document covers how to implement a custom renderer for Slint. This is intended for developers extending Slint's rendering capabilities or debugging existing renderers.

## Overview

Slint includes these renderer implementations:

| Renderer | Location | Graphics Path |
|----------|----------|---------------|
| **Software** | `internal/renderers/software/` | Pure Rust CPU rendering |
| **FemtoVG** | `internal/renderers/femtovg/` | OpenGL or WGPU through FemtoVG |
| **Skia** | `internal/renderers/skia/` | GPU or software rendering through Skia |
| **Anyrender** | `internal/renderers/anyrender/` | Anyrender drawing interface, with Vello over WGPU for window rendering |
| **Qt** | `internal/backends/qt/qt_window.rs` | QPainter drawing integrated into the Qt backend |

## Core Traits

### RendererSealed (`internal/core/renderer.rs`)

All renderers implement the internal `RendererSealed` trait.
The public `Renderer` trait extends it and has a blanket implementation for types implementing `RendererSealed`.
Custom renderer implementations therefore use internal APIs.

With `shared-parley` enabled, `RendererSealed` provides shared implementations for text measurement, font metrics, cursor geometry, and hit-testing.
Return a `TextLayoutCache` from `text_layout_cache()` to reuse shaped paragraphs across these operations and drawing.
Implement `sharedparley::GlyphRenderer` for glyph drawing through the same layout path.
See [Text Layout System](text-layout.md) for the shared pipeline and the software bitmap-font fallback.

**Key methods:**

| Method | Purpose |
|--------|---------|
| `text_layout_cache()` | Provide the cache for shared text layout |
| `text_size()` | Measure text dimensions with optional wrapping |
| `font_metrics()` | Query font ascent, descent, line height |
| `text_input_byte_offset_for_position()` | Hit-testing for text input cursor placement |
| `text_input_cursor_rect_for_byte_offset()` | Get cursor rectangle for a byte offset |
| `set_window_adapter()` / `window_adapter()` | Associate renderer with a window |
| `free_graphics_resources()` | Cleanup when components are destroyed |
| `mark_dirty_region()` | Manual dirty region marking for partial rendering |
| `register_font_from_memory()` / `register_font_from_path()` | Custom font registration |
| `set_rendering_notifier()` | Lifecycle callbacks (BeforeRendering, AfterRendering, etc.) |
| `resize()` | Handle window resize events |
| `take_snapshot()` | Capture rendered frame to pixel buffer |

### ItemRenderer (`internal/core/item_rendering.rs`)

The drawing interface for all UI elements. Each renderer provides its own implementation.

**Drawing methods:**
- `draw_rectangle()` - Solid/gradient rectangles
- `draw_border_rectangle()` - Rectangles with borders and border-radius
- `draw_image()` - Images with fit, alignment, tiling options
- `draw_text()` - Text with colors, alignment, wrapping
- `draw_text_input()` - Text input fields with selection/cursor
- `draw_path()` - Custom vector paths
- `draw_box_shadow()` - Shadow effects

**Clipping and transformations:**
- `combine_clip()` - Set clip region (supports rounded corners)
- `get_current_clip()` - Query current clip bounds
- `translate()` / `rotation()` / `scale()` - 2D transformations
- `apply_opacity()` - Alpha blending

**State management:**
- `save_state()` / `restore_state()` - State stack for nested rendering
- `filter_item()` - Early-out clipping test
- `scale_factor()` - DPI scaling factor

## Renderer Architecture Patterns

### FemtoVG Pattern: Generic Backend

FemtoVG abstracts over graphics APIs with the `GraphicsBackend` trait: a backend names its
femtovg renderer and window surface types, creates itself suspended, hands out a surface to draw
into, submits the command buffer, presents, resizes, and optionally exposes the native graphics
API and a snapshot path. `FemtoVGRenderer<B>` is generic over it.
See `GraphicsBackend` in `internal/renderers/femtovg/lib.rs`.

### Skia Pattern: Trait Object Surfaces

Skia selects its surface dynamically through the `Surface` trait: construct from a window and a
display handle plus the requested graphics API, render through a callback that gets the Skia
canvas and direct context, resize, and report the bits per pixel and whether partial rendering is
available. `render()` returns a `DrawOutcome`, so a surface can say it was occluded or timed out
instead of drawing. See `Surface` in `internal/renderers/skia/lib.rs`.

Available surface implementations: `OpenGLSurface`, `SoftwareSurface`, and one `WGPUSurface`
per supported wgpu version.

### Software Renderer Pattern: Scene Building

`SceneBuilder` implements `ItemRenderer` and hands each draw call to a `ProcessScene` implementation.
`SoftwareRenderer::render()` uses `RenderToBuffer`, which draws each item straight into a whole pixel buffer.
`render_by_line()` uses `PrepareScene`, which collects the items into a `Scene`.
The renderer then rasterizes that scene into a `LineBufferProvider` one line at a time, for memory-constrained devices.
Both return the `PhysicalRegion` that was painted.
See `internal/renderers/software/api.rs` for the entry points and `lib.rs` for the implementation.

### Anyrender Pattern: Window Renderer Abstraction

`AnyrenderSlintRenderer<W>` delegates surface handling and presentation to `SlintWindowRenderer`.
`AnyrenderItemRenderer` translates Slint drawing operations to the Anyrender interface.
The Vello implementation lives in `internal/renderers/anyrender/vello.rs`.

### Qt Pattern: Backend-Owned Renderer

`QtWindow` implements `RendererSealed`, and `QtItemRenderer` draws through QPainter.
Both live in `internal/backends/qt/qt_window.rs` and share the Qt window and event-loop integration.

## Backend Integration

### WinitCompatibleRenderer (`internal/backends/winit/`)

For winit-based applications a renderer implements `WinitCompatibleRenderer`: render a frame
(returning a `DrawOutcome`), expose itself as a core `Renderer`, react to the window becoming
occluded, and suspend/resume around the winit `Resumed` event — `resume()` is what creates the
actual `winit::window::Window`.
See `WinitCompatibleRenderer` in `internal/backends/winit/lib.rs`.

## Key Supporting Types

| Type | Location | Purpose |
|------|----------|---------|
| `ItemCache<T>` | `internal/core/` | Per-item graphics caching with automatic invalidation |
| `DirtyRegion` | `internal/core/` | Partial rendering dirty tracking |
| `RenderingNotifier` | `internal/core/` | Lifecycle event callbacks |
| `CachedRenderingData` | `internal/core/` | Per-item cached rendering state |
| `BorderRadius` | `internal/core/` | Rounded corner support |
| `Brush` | `internal/core/` | Color and gradient fills |
| `SharedPixelBuffer` | `internal/core/` | Pixel buffer for snapshots |

## Implementation Checklist

To implement a custom renderer:

1. **Implement `RendererSealed`** - Text measurement, font handling, window association
2. **Implement `ItemRenderer`** - Drawing all UI element types
3. **Handle graphics API abstraction** - Surface/backend trait if supporting multiple APIs
4. **Integrate with `WindowAdapter`** - Register renderer and handle window events
5. **Support `RenderingNotifier`** - For BeforeRendering/AfterRendering hooks
6. **Implement partial rendering** (optional) - Dirty region tracking for performance
7. **Implement caching** - Texture/image caching via `ItemCache`

## Renderer Registration & Selection

### Feature Flags

Renderer selection starts with Cargo features in `api/rs/slint/Cargo.toml`:

| Feature | Selection |
|---------|-----------|
| `renderer-software` | Software renderer |
| `renderer-femtovg` | FemtoVG with OpenGL |
| `renderer-femtovg-wgpu` | FemtoVG with WGPU |
| `renderer-skia` | Skia with its default surface selection |
| `renderer-skia-opengl`, `renderer-skia-vulkan`, `renderer-skia-software` | Skia surface variants |
| `renderer-vello` | Anyrender with Vello |
| `backend-qt` | Qt backend, including its renderer |

### Backend Selector

`create_backend()` (`internal/backends/selector/lib.rs`) chooses the event-loop backend
(winit/Qt/linuxkms/testing/headless) at runtime:

1. Parse the `SLINT_BACKEND` environment variable with `parse_backend_env_var()`, which
   splits it into an event-loop name and a renderer name (e.g. `winit-skia` → `("winit",
   "skia")`).
2. Dispatch to the matching event-loop backend's constructor, passing the renderer name
   along (e.g. `i_slint_backend_winit::Backend::new_with_renderer_by_name`).
3. If no backend/renderer was requested (or the requested one isn't compiled in), fall
   back to `create_default_backend()`'s compile-time feature priority.

The renderer name itself is resolved *inside* each event-loop backend, not in the
selector. For winit, that's `create_renderer()` in `internal/backends/winit/lib.rs`, which
matches on the renderer name (`"skia"`, `"software"`, `"gl"`/`"femtovg"`, ...) against the
renderers compiled in via Cargo features.

To add a new renderer:
1. Add a feature flag to the relevant backend's `Cargo.toml` (e.g.
   `internal/backends/winit/Cargo.toml`) and to `api/rs/slint/Cargo.toml`.
2. Add a match arm for its name in that backend's renderer-dispatch function (e.g.
   `create_renderer()` in `internal/backends/winit/lib.rs`).
3. Implement `WinitCompatibleRenderer` (or the equivalent trait for other backends) for
   the new renderer.

### Runtime Selection

```sh
SLINT_BACKEND=winit-software cargo run    # Force software renderer
SLINT_BACKEND=winit-skia cargo run        # Force Skia renderer
SLINT_BACKEND=winit-vello cargo run       # Anyrender with Vello (requires renderer-vello)
SLINT_BACKEND=qt cargo run                # Qt backend and renderer (requires backend-qt)
```

## Window & Event Loop Integration

Renderers integrate with the platform through `WindowAdapter`:

```
Platform (winit/qt/linuxkms)
    └── WindowAdapter
            ├── window() -> Window (Slint window abstraction)
            └── renderer() -> &dyn Renderer
                    └── render() called by event loop on redraw
```

**Render lifecycle:**
1. Event loop receives redraw request
2. Backend calls `WindowAdapter::renderer().render()`
3. Renderer traverses item tree via `ItemRenderer` methods
4. Renderer presents to screen/surface

**Key integration points:**
- `internal/backends/winit/winitwindowadapter.rs` - Winit integration
- `internal/core/window.rs` - Platform-agnostic window logic
- `internal/core/api.rs` - Public `Window` API

## Testing Renderer Changes

### Screenshot Tests

`test-driver-screenshots` lives in the separate `tests/` Cargo workspace, so these need
`--manifest-path tests/Cargo.toml` when run from the repository root:

```sh
# Run screenshot comparison tests
cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots

# Generate new reference screenshots (run when intentionally changing rendering)
SLINT_CREATE_SCREENSHOTS=1 cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots
```

The FemtoVG screenshot tests render offscreen through WGPU and run with the `femtovg` feature, which CI enables through `--all-features`.
They need a WGPU adapter but no window or display server, and pass without rendering when there's no adapter.
Set `SLINT_TEST_REQUIRE_WGPU` to make them fail instead, as CI does.
On Linux, CI uses llvmpipe from the `mesa-vulkan-drivers` package.
Every case runs unless it has a `//ignore: femtovg` marker.

```sh
cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots --no-default-features --features femtovg
```

The generator honors `SIZE`, `SLINT_SCALE_FACTOR`, `BASE_THRESHOLD`, and `//ignore: femtovg` markers.
`SKIP_WARP` skips a case on WARP, the D3D12 software adapter on Windows CI.
`SKIP_LLVMPIPE` skips a case on llvmpipe, Mesa's Vulkan software adapter on Linux CI.
Every case accepts a color difference of at least 4.5, since GPU drivers round differently.
Ignored cases run when the command includes `-- --ignored`.

References live under `tests/screenshots/references/femtovg/`.
The tests use bundled fonts and the Fluent style for consistent output across platforms.
Separate shadow checks verify cache invalidation and corner geometry.

### Testing Backend

Use the headless testing backend for automated tests:

```sh
SLINT_BACKEND=testing cargo test
```

The testing backend (`internal/backends/testing/`) provides:
- Headless rendering without display
- Simulated input events
- Screenshot capture for comparison

### Visual Verification

```sh
# Run gallery to visually inspect rendering (in the separate examples/ workspace)
cargo run --manifest-path examples/Cargo.toml -p gallery

# View specific .slint file with hot reload
cargo run --bin slint-viewer -- path/to/file.slint
```

## Directory Structure

```
internal/renderers/
├── anyrender/
│   ├── lib.rs           # AnyrenderSlintRenderer, SlintWindowRenderer trait
│   ├── itemrenderer.rs  # AnyrenderItemRenderer (ItemRenderer impl)
│   ├── vello.rs         # Vello window renderer on WGPU
│   └── recording.rs     # Records draw commands for the screenshot tests
├── femtovg/
│   ├── lib.rs           # FemtoVGRenderer, GraphicsBackend trait
│   ├── itemrenderer.rs  # GLItemRenderer (ItemRenderer impl)
│   ├── opengl.rs        # OpenGL backend
│   ├── wgpu.rs          # WebGPU backend
│   ├── font_cache.rs
│   └── images.rs
├── skia/
│   ├── lib.rs           # SkiaRenderer, Surface trait
│   ├── itemrenderer.rs  # SkiaItemRenderer (ItemRenderer impl)
│   ├── opengl_surface.rs
│   ├── software_surface.rs
│   ├── wgpu_29_surface.rs / wgpu_30_surface.rs
│   └── wgpu_renderer.rs
└── software/
    ├── api.rs           # SoftwareRenderer, LineBufferProvider
    ├── lib.rs           # SceneBuilder (ItemRenderer impl), rendering
    ├── scene.rs         # Scene and scene item structures
    └── draw_functions.rs
```

The Qt renderer isn't in this directory: it's part of `internal/backends/qt/qt_window.rs`.

## Example: Studying Existing Implementations

The software renderer is the simplest to study as it has no external dependencies:

- Entry points: `SoftwareRenderer::render()` and `render_by_line()` in `internal/renderers/software/api.rs`
- `SceneBuilder` in `lib.rs` implements `ItemRenderer` and forwards draw calls to a `ProcessScene`
- `render()` draws them straight into a pixel buffer, while `render_by_line()` collects a `Scene` first

For GPU rendering patterns, study `internal/renderers/skia/itemrenderer.rs` which shows:
- Texture caching strategies
- Transformation matrix handling
- Clipping with GPU-accelerated paths
