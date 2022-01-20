# OpenGL Underlay Example

This example application demonstrates how layer two scenes together in a window:

1. First a graphical effect is rendered using low-level OpenGL code (underlay).
2. A scene of SixtyFPS elements is rendered above.

This is implemented using the `set_rendering_notifier` function on the `sixtyfps::Window` type. It takes a callback as a parameter and that is invoked during different phases of the rendering. In this example the invocation during the setup phase is used to prepare the pipeline for OpenGL rendering later. Then the `BeforeRendering` phase is used to render the graphical effect with OpenGL. Afterwards, SixtyFPS will render the scene of elements into the same back-buffer as the previous OpenGL code rendered into.

Since the graphical effect is continuous, the code in the callback requests a redraw of the contents by calling `sixtyfps::Window::request_redraw()`.
