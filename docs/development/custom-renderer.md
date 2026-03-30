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

FemtoVG abstracts over graphics APIs using generics:

```rust
pub struct FemtoVGRenderer<B: GraphicsBackend> { ... }

pub trait GraphicsBackend {
    type Renderer: femtovg::Renderer + TextureImporter;
    type WindowSurface: WindowSurface<Self::Renderer>;

    fn new_suspended() -> Self;
    fn begin_surface_rendering(&self) -> Result<Self::WindowSurface, ...>;
    fn submit_commands(&self, commands: ...);
    fn present_surface(&self, surface: Self::WindowSurface) -> Result<(), ...>;
    fn resize(&self, width: NonZeroU32, height: NonZeroU32) -> Result<(), ...>;
}
```

### Skia Pattern: Trait Object Surfaces

Skia uses trait objects for dynamic surface selection:

```rust
pub trait Surface {
    fn new(
        shared_context: &SkiaSharedContext,
        window_handle: Arc<dyn HasWindowHandle + Sync + Send>,
        display_handle: Arc<dyn HasDisplayHandle + Sync + Send>,
        size: PhysicalWindowSize,
        requested_graphics_api: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, PlatformError>;

    fn name(&self) -> &'static str;
    fn render(&self, window: &Window, size: PhysicalWindowSize,
              render_callback: &dyn Fn(&Canvas, ...), ...) -> Result<(), ...>;
    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), ...>;
    fn use_partial_rendering(&self) -> bool { false }
}
```

Available surface implementations: `OpenGLSurface`, `MetalSurface`, `VulkanSurface`, `D3DSurface`, `SoftwareSurface`

### Software Renderer Pattern: Scene Building

The software renderer builds a scene graph then rasterizes:

```rust
pub struct SoftwareRenderer { ... }

impl SoftwareRenderer {
    pub fn render(&self, buffer: &mut [impl TargetPixel], pixel_stride: usize);
    pub fn render_by_line(&self, line_callback: impl FnMut(&mut [impl TargetPixel]));
}
```

Supports memory-constrained devices via line-by-line rendering.

## Backend Integration

### WinitCompatibleRenderer (`internal/backends/winit/`)

For winit-based applications, renderers implement:

```rust
pub trait WinitCompatibleRenderer: std::any::Any {
    fn render(&self, window: &Window) -> Result<(), PlatformError>;
    fn as_core_renderer(&self) -> &dyn Renderer;
    fn suspend(&self) -> Result<(), PlatformError>;
    fn resume(&self, event_loop: &ActiveEventLoop,
              attrs: WindowAttributes) -> Result<Arc<winit::window::Window>, ...>;
}
```

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

The selector (`internal/backends/selector/lib.rs`) chooses renderer at runtime:

1. Check `SLINT_BACKEND` environment variable (e.g., `winit-skia`, `winit-femtovg`)
2. Fall back to compile-time feature priority

To add a new renderer:
1. Add feature flag to `internal/backends/selector/Cargo.toml`
2. Update `try_create_renderer()` in `internal/backends/selector/lib.rs`
3. Wire up in the appropriate backend (e.g., `internal/backends/winit/`)

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

```sh
# Run screenshot comparison tests
cargo test -p test-driver-screenshots

# Generate new reference screenshots (run when intentionally changing rendering)
SLINT_CREATE_SCREENSHOTS=1 cargo test -p test-driver-screenshots
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
# Run gallery to visually inspect rendering
cargo run -p gallery

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
│   └── wgpu.rs          # WebGPU backend
├── skia/
│   ├── lib.rs           # SkiaRenderer, Surface trait
│   ├── itemrenderer.rs  # SkiaItemRenderer (ItemRenderer impl)
│   ├── opengl_surface.rs
│   ├── metal_surface.rs
│   ├── vulkan_surface.rs
│   └── software_surface.rs
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
