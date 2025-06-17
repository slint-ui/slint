<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# WGPU Underlay Example

This example application demonstrates how layer two scenes together in a window:

1. First a graphical effect is rendered using low-level WGPU code (underlay).
2. A scene of Slint elements is rendered above.

This is implemented using the `set_rendering_notifier` function on the `slint::Window` type. It takes a callback as a parameter and that is invoked during different phases of the rendering. In this example the invocation during the setup phase is used to prepare the pipeline for WGPU rendering later. Then the `BeforeRendering` phase is used to render the graphical effect with WGPU. Afterwards, Slint will render the scene of elements into the same surface texture as the previous WGPU code rendered into.

Since the graphical effect is continuous, the code in the callback requests a redraw of the contents by calling `slint::Window::request_redraw()`.

![Screenshot of WGPU Underlay](https://slint.dev/resources/opengl_underlay_screenshot.png "OpenGL Underlay screenshot")
