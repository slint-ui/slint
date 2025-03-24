---
<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
// cSpell: ignore libx libxcursor libxkbcommon
title: Winit Backend
description: Winit Backend
next: false
---

The Winit backend uses the [winit](https://docs.rs/winit/latest/winit/) library to interact with the
windowing system.

The Winit backend supports practically all relevant operating systems and windowing systems, including
macOS, Windows, Linux with Wayland and X11.

The Winit backend supports different renderers. They can be explicitly selected for use through the
`SLINT_BACKEND` environment variable.

| Renderer name | Supported/Required Graphics APIs            | `SLINT_BACKEND` value to select renderer |
|---------------|---------------------------------------------|------------------------------------------|
| FemtoVG       | OpenGL                                      | `winit-femtovg`                          |
| Skia          | OpenGL, Metal, Direct3D, Software-rendering | `winit-skia`                             |
| Skia Software | Software-only rendering with Skia           | `winit-skia-software`                    |
| Skia OpenGL   | OpenGL rendering with Skia                  | `winit-skia-opengl`                      |
| software      | Software-rendering, no GPU required         | `winit-software`                         |

If no renderer is explicitly set, the backend will first try to use the Skia renderer, if it was enabled at compile time.
If that fails, it will fall back to the FemtoVG renderer, and if that also fails, it will use the software renderer.


## Configuration Options

The Winit backend reads and interprets the following environment variables:

| Name               | Accepted Values | Description                                                        |
|--------------------|-----------------|--------------------------------------------------------------------|
| `SLINT_FULLSCREEN` | any value       | If this variable is set, every window is shown in fullscreen mode. |

## Linux Dependencies

On Linux, the Winit backend requires either X11 or Wayland to be available.
Support of either can be enabled or disabled at compile time by setting the
`backend-winit-x11` or `backend-winit-wayland` features (instead of `backend-winit`).

For X11 the following runtime dependencies are required: libx11-xcb, xinput, libxcursor, libxkbcommon-x11, libx11.
On Debian-based systems, these can be installed with:

```sh
sudo apt install libx11-xcb-dev xinput libxcursor-dev libxkbcommon-x11-dev libx11-dev
```
