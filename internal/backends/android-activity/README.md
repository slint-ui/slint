<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

# Slint Android Activity Backend

This crate implements the Android backend/platform for Slint.

It uses the [android-activity](https://github.com/rust-mobile/android-activity) crate
to initialize the app and provide events handling.

It can be used by using functions from the [slint::android](https://slint.dev/docs/rust/slint/android/) module
