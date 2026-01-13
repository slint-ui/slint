# Slint + Bevy Integration Example

This example demonstrates how to integrate Slint UI into a [Bevy](https://bevyengine.org/) application.

It shows how to render a Slint component into a Bevy `Image` (texture) and display it as a sprite within the 3D scene, complete with mouse interaction handling.

## Features

- **Custom Platform Backend**: Implements a `SlintBevyPlatform` to bridge Slint's windowing to Bevy.
- **Texture Rendering**: Uses Slint's software renderer to draw the UI into a pixel buffer, which is then copied to a Bevy texture.
- **Input Forwarding**: Captures mouse events (movement and clicks) in Bevy and translates them to Slint pointer events, enabling interactive UI elements like buttons and sliders.
- **3D Scene Integration**: The UI is rendered as a 2D sprite alongside 3D objects (a monkey model).

## Prerequisites

- Rust (latest stable recommended)
- System dependencies for Bevy (see [Bevy's setup guide](https://bevyengine.org/learn/book/getting-started/setup/))

## Running the Example

```bash
cargo run
```

## Architecture

The integration consists of a few key parts:

1.  **`SlintBevyPlatform`**: A custom implementation of `slint::platform::Platform` that manages window adapters.
2.  **`BevyWindowAdapter`**: Implements `slint::platform::WindowAdapter`. It doesn't create a native OS window but instead maintains a buffer for the software renderer.
3.  **`render_slint` System**: A Bevy system that runs every frame. It triggers the Slint renderer and updates the Bevy image with the new pixel data.
4.  **`handle_input` System**: Converts Bevy's mouse input events into Slint's logical coordinates and dispatches them to the Slint window.

## Code Structure

- `main.rs`: Contains the entire example code, including the Slint UI definition (in the `slint!` macro), the Bevy systems, and the adapter logic.
