
# Slint Software Renderer

This crate renders Slint GUIs on the CPU, without a GPU.
It runs on microcontrollers (MCUs) such as STM32, ESP32, and RP2040 in `no_std` environments,
and on any device with a framebuffer.
With Rust, it can render line by line, so the device doesn't need RAM for a full framebuffer.
The `slint` crate enables it by default, with the `renderer-software` feature.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
