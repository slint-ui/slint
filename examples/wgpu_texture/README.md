<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# WGPU Texture Import Example

This example application demonstrates how import a WGPU texture into a Slint scene:

1. First a graphical effect is rendered using WGPU, into a texture.
2. The texture is imported into a `slint::Image` and set on an `Image` element.
3. A scene of Slint elements is rendered with the texture shown in the `Image`.

This is implemented using the `set_rendering_notifier` function on the `slint::Window` type. It takes a callback as a parameter and that is invoked during different phases of the rendering. In this example the invocation during the setup phase is used to prepare the pipeline for WGPU rendering later. Then the `BeforeRendering` phase is used to render the graphical effect with WGPU into a texture. Then the texture is imported and Slint will render the scene of elements with the texture.

Since the graphical effect is continuous, the code in the callback requests a redraw of the contents by calling `slint::Window::request_redraw()`.

