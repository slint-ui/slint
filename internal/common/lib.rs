// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!
    This crate contains internal data structures and code that is shared between
    the slint-core-internal and the slint-compiler-internal crates.

**NOTE**: This library is an **internal** crate for the [Slint project](https://sixtyfps.io).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

*/

#![no_std]

pub mod key_codes;
