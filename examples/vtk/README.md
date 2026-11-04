# VTK Example

This C++ example embeds an interactive [VTK](https://vtk.org) scene in a Slint UI.
VTK renders with OpenGL into a texture, which an `Image` element shows next to Slint widgets.
It works with Slint's FemtoVG and Skia OpenGL renderers.

VTK renders from the rendering notifier, sharing the OpenGL context with Slint.
`gl_state.h` saves and restores the OpenGL state around VTK's rendering, because both renderers expect it unchanged.
Debug builds verify that the restored state matches.

## Building

Install VTK 9.4 or newer, for example with `brew install vtk` on macOS.
Many Linux distributions ship an older VTK; build VTK from source there.
Then build the example with CMake:

```sh
cmake -B build -DCMAKE_PREFIX_PATH="<slint-install-dir>;<vtk-install-dir>"
cmake --build build
```

## Running

Select the renderer with the `SLINT_BACKEND` environment variable:

```sh
SLINT_BACKEND=winit-femtovg ./build/vtk_example
SLINT_BACKEND=winit-skia-opengl ./build/vtk_example
```

Drag to rotate the surface, drag with Shift held to pan, and scroll to zoom.

![Screenshot of the VTK Example on macOS](https://github.com/user-attachments/assets/a9b8672a-5d98-4fcc-abd4-eee09b6d38f9 "VTK Example")
