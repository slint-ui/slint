
# The Slint Live Preview

This crate powers the live preview of the Slint language server,
which lets you see and edit a `.slint` UI live in VS Code and other editors.
It also powers hot reload: built with the `live-preview` feature of the `slint` crate and `SLINT_LIVE_PREVIEW=1`,
a running app reloads its `.slint` files when they change on disk, without a rebuild.
It holds the protocol between the language server and the preview, file watching,
live component reloading, and remote preview support.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
