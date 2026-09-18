# Custom Renderer Implementation Guide

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `internal/renderers/`, adding
> rendering backends, fixing drawing bugs, or implementing custom graphics output.
> For general build commands and project structure, see `/AGENTS.md`.

This document covers how to implement a custom renderer for Slint. This is intended for developers extending Slint's rendering capabilities or debugging existing renderers.

## Overview

Slint includes three built-in renderers:
- **Software Renderer** (`internal/renderers/software/`) - Pure Rust CPU-based rendering
- **FemtoVG Renderer** (`internal/renderers/femtovg/`) - OpenGL ES 2.0 via FemtoVG library
- **Skia Renderer** (`internal/renderers/skia/`) - GPU-accelerated via Skia library

## Core Traits

### RendererSealed (`internal/core/renderer.rs`)

The fundamental trait all renderers must implement. Uses the sealed trait pattern—`RendererSealed` is internal, while `Renderer` is the public re-export that external code uses.

**Key methods:**

| Method | Purpose |
|--------|---------|
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

The software renderer builds a scene graph then rasterizes it. `SoftwareRenderer::render()` draws
into a whole pixel buffer, while `render_by_line()` drives a `LineBufferProvider` one line at a
time for memory-constrained devices. Both return the `PhysicalRegion` that was painted.
See `internal/renderers/software/lib.rs`.

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

Renderers are enabled via Cargo features in `api/rs/slint/Cargo.toml`:

```toml
renderer-femtovg = ["i-slint-backend-selector/renderer-femtovg"]
renderer-skia = ["i-slint-backend-selector/renderer-skia"]
renderer-software = ["i-slint-backend-selector/renderer-software"]
```

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
    ├── lib.rs           # SoftwareRenderer, scene building
    ├── scene.rs         # Scene graph structures
    └── draw_functions.rs
```

## Example: Studying Existing Implementations

The software renderer is the simplest to study as it has no external dependencies:

- Entry point: `internal/renderers/software/lib.rs`
- Scene builder implements `ItemRenderer`: builds a scene graph from draw calls
- `render()` method rasterizes the scene to a pixel buffer

For GPU rendering patterns, study `internal/renderers/skia/itemrenderer.rs` which shows:
- Texture caching strategies
- Transformation matrix handling
- Clipping with GPU-accelerated paths
