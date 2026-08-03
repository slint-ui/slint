# Slint + Bevy GPU Integration Example

This example demonstrates how to integrate Slint UI into a [Bevy](https://bevyengine.org/) application.

It shows how to render a Slint component with WGPU into a Bevy `Image` (texture) and display it on a 3D quad mesh attached to a rotating cube, complete with mouse interaction handling via raycasting.

## Features

- **Custom Platform Backend**: Implements a `slint::platform::Platform` to bridge Slint's windowing to Bevy.
- **Texture Rendering**: Uses Slint's `SkiaWGPURenderer` to render the UI straight into a Bevy-owned `wgpu::Texture` via `render_to_texture()`. There is no CPU pixel buffer and no upload step, which is the difference from the `bevy-hosts-slint`.
- **Input Forwarding**: Raycasts from the camera through the mouse position to detect intersection with the UI quad, then translates hit coordinates to Slint pointer events, enabling interactive UI elements like buttons and sliders.
- **3D Scene Integration**: The UI is rendered on a quad mesh attached to a rotating cube. The Slint component's transparent background (`background: #00000000`) combines with the material's `alpha_mode: Blend` so the scene shows through the UI.
- **Keyboard Controls**: Use arrow keys to rotate the cube and observe the UI from different angles.
- **Animations**: The slider automatically animates to demonstrate Slint's animation system working within Bevy.

## Prerequisites

- System dependencies for Bevy (see [Bevy's setup guide](https://bevyengine.org/learn/book/getting-started/setup/))
- Slint built with the `renderer-skia` and `unstable-wgpu-29` features. The latter exposes the WGPU interop API and pins the wgpu major version, so it must match the version Bevy uses — wgpu 29 for Bevy 0.19.

## Running the Example

```bash
cargo run --release
```

## Architecture

The integration consists of a few key parts:

1.  **`SlintBevyPlatform`**: A custom implementation of `slint::platform::Platform` that manages window adapters.
2.  **`BevyWindowAdapter`**: Implements `slint::platform::WindowAdapter`. It doesn't create a native OS window but instead owns a `SkiaWGPURenderer` built on Bevy's wgpu device, queue, adapter and instance.
3.  **`SlintSharedTexture`**: Gets the `wgpu::Texture` behind the Bevy `Image` across the world boundary — see below.
4.  **`render_slint` System**: A Bevy system that runs every frame. It calls `slint::platform::update_timers_and_animations()`, then `render_to_texture()` on the shared texture.
5.  **`handle_input` System**: Raycasts from the camera through the mouse position, checks for intersection with the UI quad, converts the 3D hit point to UV coordinates, then to Slint's logical coordinates, and dispatches pointer events to the Slint window.

## Code Structure

- `main.rs`: Contains the entire example code, including the Slint UI definition (in the `slint!` macro), the Bevy systems, and the adapter logic.
