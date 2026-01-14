# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a working Slint + Bevy integration example demonstrating how to embed Slint UI components within a Bevy game engine application. The example renders a Slint UI (with buttons, text, sliders) as a texture in a Bevy scene alongside 3D content.

This example is part of the larger Slint repository and has been updated to work with:
- Bevy 0.17 (Required Components pattern)
- Current Slint API (platform adapter pattern)

## Key Architecture

### Integration Approach

The integration works by implementing a custom Slint platform backend (`SlintBevyPlatform`) and window adapter (`BevyWindowAdapter`):

1. **Custom Platform**: `SlintBevyPlatform` creates window adapters without opening native OS windows
2. **Software Rendering**: Uses Slint's `SoftwareRenderer` to render UI into a pixel buffer
3. **Texture Bridge**: The pixel buffer is copied to a Bevy `Image` texture each frame
4. **Thread-Local Windows**: Slint window instances are stored in `thread_local!` storage and accessed by Bevy systems

### Main Components

- **`BevyWindowAdapter`** (main.rs:53-105): Implements `slint::platform::WindowAdapter` to bridge Slint's windowing model to Bevy's texture-based rendering
- **`SlintBevyPlatform`** (main.rs:111-121): Custom platform implementation that creates window adapters
- **`render_slint` system** (main.rs:208-246): Bevy system that renders Slint UI to texture each frame
- **`Demo` component** (main.rs:18-38): Inline Slint UI definition using the `slint!` macro

### Slint UI Definition

The Slint UI is defined inline using the `slint!` macro (main.rs:18-38). This macro compiles `.slint` markup at Rust compile time into native structures.

## Development Commands

### Building and Running

```bash
# Build the example
cargo build

# Run the example
cargo run --bin bevy_example

# Build release version
cargo build --release

# Run release version
cargo run --bin bevy_example --release
```

### Parent Repository Context

Since this is part of the Slint monorepo, you can also build from the repository root:

```bash
# From /Users/till/Code/Rust/slint/slint
cargo build -p bevy-hosts-slint
cargo run -p bevy-hosts-slint

# Build entire workspace (excluding UEFI demo)
cargo build --workspace --exclude uefi-demo
```

### Testing

This example doesn't have dedicated tests, but you can:

```bash
# Check that it compiles
cargo check

# Build and verify it runs without panicking
cargo run --release
```

## Dependencies

- **slint**: Uses local path dependency (`../../../slint/api/rs/slint`) with software renderer
  - Features: `compat-1-2`, `renderer-software`, `software-renderer-systemfonts`, `std`, `unstable-wgpu-26`
- **bevy**: Game engine (0.17.0) with minimal feature set for 2D/3D rendering
  - Required features: `bevy_core_pipeline`, `bevy_pbr`, `bevy_sprite`, `bevy_winit`, `bevy_window`, `bevy_scene`, `bevy_gltf`
  - Note: `bevy_sprite` is essential for rendering the Slint UI as a 2D sprite
  - Note: `bevy_winit` is **required** to create an actual OS window (without it, app runs headless)
- **bytemuck**: For safe transmutation of pixel buffer to byte slice
- **spin_on**, **smol**, **async-compat**: For async/event loop integration
- **reqwest**: HTTP client (likely for asset loading)

### Bevy 0.17 Compatibility Notes

This example has been updated for Bevy 0.17, which introduced several breaking changes:

- **Required Components**: Bundles have been replaced with Required Components pattern
  - Old: `SceneBundle`, `PointLightBundle`, `SpriteBundle`, `Camera2dBundle`, `Camera3dBundle`
  - New: Spawn with individual components like `Camera2d`, `Camera3d`, `Sprite`, etc.
- **Sprite API**: Changed to `Sprite::from_image(handle)` instead of separate `Sprite` and `Handle<Image>` components
- **Image Data**: `image.data` is now `Option<Vec<u8>>` instead of `Vec<u8>`
- **Removed**: `close_on_esc` utility function (was in `bevy::window`)

### Slint API Updates

The example has been updated to work with current Slint platform adapter patterns:

- Window adapter initialization moved out of `Rc::new_cyclic` closure to avoid panics
- Initial `Resized` event dispatched in `create_window_adapter` after Rc is fully constructed
- No explicit `.show()` call needed - window is initialized automatically
- `set_visible` implementation simplified to always dispatch resize event

## File Structure

```
bevy-hosts-slint/
├── main.rs              # All code in single file
├── Cargo.toml           # Package manifest
├── assets/
│   └── Monkey.gltf      # 3D model rendered in Bevy scene
├── src/                 # Empty (binary uses main.rs)
└── target/              # Build artifacts
```

## Important Implementation Details

### Rendering Flow

1. Bevy's `render_slint` system runs every frame
2. System retrieves the `BevyWindowAdapter` from thread-local storage
3. Checks if window size changed and dispatches resize event to Slint
4. If `needs_redraw` flag is set, renders Slint UI to pixel buffer
5. Copies pixel buffer to Bevy `Image` texture using `bytemuck::cast_slice`
6. Bevy renders the texture as a 2D sprite overlaid on the 3D scene

### Platform Integration Pattern

This example demonstrates a general pattern for integrating Slint with any custom rendering backend:
1. Implement `slint::platform::Platform` trait
2. Implement `slint::platform::WindowAdapter` trait
3. Use `slint::platform::set_platform()` to register your platform
4. Call `renderer().render()` to get pixels
5. Upload pixels to your graphics API

### Multi-Camera Setup

The example uses two Bevy cameras:
- `Camera3d`: Renders the 3D monkey model (at z=6.0 looking at origin)
- `Camera2d`: Renders the Slint UI sprite with `order: 2` (renders on top of 3D scene)

## Slint Language Features

The inline `.slint` code demonstrates:
- Component inheritance (`inherits Window`)
- Layout containers (`VerticalBox`)
- Standard widgets (`Text`, `Button`, `Slider`)
- Property bindings (`background: #ff00ff3f`)
- Widget imports (`import { VerticalBox, Button, Slider } from "std-widgets.slint"`)

## Common Modifications

### Changing the UI

Edit the `slint!` macro content (main.rs:18-38). The Slint code is compiled at Rust compile-time.

### Adjusting Texture Size

Modify the `Extent3d` in `setup()` (main.rs:147-151). The texture will auto-resize on the next frame.

### Adding UI Callbacks

Use the generated `Demo` struct methods to connect callbacks:

```rust
let instance = Demo::new().unwrap();
instance.on_button_clicked(|| {
    println!("Button clicked!");
});
```

## Related Documentation

- Main Slint docs: https://slint.dev/docs/slint
- Slint Rust API: https://slint.dev/latest/docs/rust/slint/
- Platform integration guide: Check `api/rs/slint` README in parent repo
- Parent repo build instructions: `/Users/till/Code/Rust/slint/slint/docs/building.md`
