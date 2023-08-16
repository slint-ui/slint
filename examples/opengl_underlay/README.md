<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# OpenGL Underlay Example

This example application demonstrates how layer two scenes together in a window:

1. First a graphical effect is rendered using low-level OpenGL code (underlay).
2. A scene of Slint elements is rendered above.

This is implemented using the `set_rendering_notifier` function on the `slint::Window` type. It takes a callback as a parameter and that is invoked during different phases of the rendering. In this example the invocation during the setup phase is used to prepare the pipeline for OpenGL rendering later. Then the `BeforeRendering` phase is used to render the graphical effect with OpenGL. Afterwards, Slint will render the scene of elements into the same back-buffer as the previous OpenGL code rendered into.

Since the graphical effect is continuous, the code in the callback requests a redraw of the contents by calling `slint::Window::request_redraw()`.
