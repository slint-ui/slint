# The Slint Editor Preview

Internal crate shared between the Slint language server and the visual editor.
It holds the document model of a Slint project as it is being edited, the
editing session that keeps previews in sync with it, and the protocol helpers
used by the language server and visual editor.

The language server and visual editor own separate preview engines and user
interfaces.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
