
# The Slint Live Preview

Internal crate that implements helpers for the Slint live previews.
This includes the protocol for communication between the LSP and the
preview, file watching, live component reloading, and remote preview
support.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
