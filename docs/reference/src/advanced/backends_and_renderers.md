<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Backends & Renderers

In Slint, a backend is the module that encapsulates the interaction with the operating system,
in particular the windowing sub-system. Multiple backends can be compiled into Slint and one
backend is selected for use at run-time on application start-up. You can configure Slint without
any built-in backends, and instead develop your own backend by implementing Slint's platform
abstraction and window adapter interfaces.

The backend is selected as follows:

1. The developer provides their own backend and sets it programmatically.
2. Else, the backend is selected by the value of the `SLINT_BACKEND` environment variable, if it is set.
3. Else, backends are tried for initialization in the following order:
   1. qt
   2. winit
   3. linuxkms

The following table provides an overview over the built-in backends. For more information about the backend's
capabilities and their configuration options, see the respective sub-pages.

| Backend Name | Description                                                                                             | Built-in by Default   |
|--------------|---------------------------------------------------------------------------------------------------------|-----------------------|
| qt           | The Qt library is used for windowing system integration, rendering, and native widget styling.          | Yes (if Qt installed) |
| winit        | The [winit](https://docs.rs/winit/latest/winit/) library is used to interact with the windowing system. | Yes                   |
| linuxkms     | Linux's KMS/DRI infrastructure is used for rendering. No windowing system or compositor is required.    | No                    |

A backend is also responsible for selecting a renderer. See the [Renderers](#renderers) section
for an overview. Override the choice of renderer by adding the name to the `SLINT_BACKEND` environment variable, separated by a dash.
For example if you want to choose the `winit` backend in combination with the `software` renderer, set `SLINT_BACKEND=winit-software`.
Similarly, `SLINT_BACKEND=linuxkms-skia` chooses the `linuxkms` backend and then instructs the LinuxKMS backend to use Skia for rendering.

```{toctree}
:hidden:
:maxdepth: 2

backend_qt.md
backend_winit.md
backend_linuxkms.md
```

## Renderers

Slint comes with different renderers that use different techniques and libraries to turn
your scene of elements into pixels. Slint picks a renderer backend on your choice of Backend
as well as the features you've selected at Slint compilation time.


### Qt Renderer

The Qt renderer comes with the [Qt backend](backend_qt.md) and renders using QPainter:

 - Software rendering, no GPU acceleration.
 - Available only in the Qt backend.

### Software Renderer

- Runs anywhere, highly portable, and lightweight.
- Software rendering, no GPU acceleration.
- Supports partial rendering.
- Suitable for Microcontrollers.
- No support for `Path` rendering and image rotation yet.
- Text rendering currently limited to western scripts.
- Available in the [Winit backend](backend_winit.md).
- Public [Rust](slint-rust:platform/software_renderer/) and [C++](slint-cpp:api/classslint_1_1platform_1_1SoftwareRenderer) API.

### FemtoVG Renderer

 - Highly portable.
 - GPU acceleration with OpenGL (required).
 - Text and path rendering quality sometimes sub-optimal.
 - Available in the [Winit backend](backend_winit.md) and [LinuxKMS backend](backend_linuxkms.md).
 - Public [Rust](slint-rust:platform/femtovg_renderer/) API.

### Skia Renderer

 - Sophisticated GPU acceleration with OpenGL, Metal, Vulkan, and Direct3D.
 - Heavy disk-footprint compared to other renderers.
 - Available in the [Winit backend](backend_winit.md) and [LinuxKMS backend](backend_linuxkms.md).
 - Public [C++](slint-cpp:api/classslint_1_1platform_1_1SkiaRenderer) API.

