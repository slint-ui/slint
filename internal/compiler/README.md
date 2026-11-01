
# The Slint Compiler Library

This crate holds the internals of the Slint compiler,
which parses `.slint` UI files and generates Rust or C++ code.
`slint-build`, the `slint!` macro, the `slint-compiler` tool, the interpreter, and the language server use it.

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
