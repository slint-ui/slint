
# The Slint winit Backend

This crate is the default backend of Slint on desktop, iOS, and the web with WebAssembly.
Built on winit, it opens and manages the app's windows, or a canvas element in the browser.
With the `accessibility` feature, it exposes the UI to screen readers and other assistive technologies through [AccessKit](https://accesskit.dev).
Apps get it through the `slint` crate, which enables it by default.
For direct access to the winit API, such as the underlying `winit::window::Window`,
enable the `unstable-winit-030` feature of the `slint` crate and use its [`slint::winit_030`](https://docs.rs/slint/latest/slint/winit_030/) module.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
