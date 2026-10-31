
# Slint Runtime Library

This crate holds the internals of the Slint runtime: properties and bindings, layouts, animations, and input handling.
It runs everywhere Slint does, including `no_std` environments on microcontrollers.
Apps get it through the `slint` crate.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
