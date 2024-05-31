<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

parser_test: a proc macro attribute that generate tests for the parser functions

The parser_test macro will look at the documentation of a function for a
markdown block delimited by ` ```test` and will feeds each line to the parser
function, checking that no error are reported, and that everything was consumed

A parser function must have the signature `fn(&mut impl Parser)`

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.
