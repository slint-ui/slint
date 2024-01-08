<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Winit Backend

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
| software      | Software-rendering, no GPU required         | `winit-software`                         |

