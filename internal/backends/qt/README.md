
# Slint Qt Backend

This crate lets Slint apps use Qt for windowing and rendering.
With the `qt` style, Slint's widgets look like the native widgets of the desktop.
Apps get it through the `backend-qt` feature of the `slint` crate.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
