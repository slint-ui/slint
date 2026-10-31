
# Slint FemtoVG Renderer

This crate renders Slint GUIs on the GPU with FemtoVG, through OpenGL,
or through WGPU with Metal, Vulkan, and Direct3D.
Built for WebAssembly, it renders in the browser through WebGL.
It's portable and easier to compile than the Skia renderer,
so the `slint` crate enables it by default, with the `renderer-femtovg` feature.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
