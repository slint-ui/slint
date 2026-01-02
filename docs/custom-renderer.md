# Custom Renderer Implementation Guide

This document covers how to implement a custom renderer for Slint. This is intended for developers extending Slint's rendering capabilities.

## Overview

Slint includes three built-in renderers:
- **Software Renderer** (`internal/renderers/software/`) - Pure Rust CPU-based rendering
- **FemtoVG Renderer** (`internal/renderers/femtovg/`) - OpenGL ES 2.0 via FemtoVG library
- **Skia Renderer** (`internal/renderers/skia/`) - GPU-accelerated via Skia library

## Core Traits

### RendererSealed (`internal/core/renderer.rs`)

The fundamental trait all renderers must implement. Uses the sealed trait pattern to prevent external implementations while exposing a public `Renderer` trait.

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
