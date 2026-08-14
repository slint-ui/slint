# The Slint Editor Preview

Internal crate shared between the Slint language server and the visual editor.
It holds the document model of a Slint project as it is being edited and the
editing session that keeps the preview in sync with it.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
