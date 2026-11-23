
# Slint Skia Renderer

This crate renders Slint GUIs with Skia, the graphics library of Chrome and Android.
It draws through OpenGL, Vulkan, Metal, or Direct3D, and falls back to software rendering.
It supports more features and renders faster than the default FemtoVG renderer,
at the cost of a larger binary and a heavier build.
To use it, enable the `renderer-skia` feature of the `slint` crate.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
