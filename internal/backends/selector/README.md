<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

**NOTE**: This library is an **internal** crate of the [Slint project](https://slint.dev).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.


The purpose of this crate is to select the default backend for [Slint](https://slint.dev)

The backend can either be a runtime or a build time decision.  The runtime decision is decided
by the `SLINT_BACKEND` environment variable. The built time default depends on the platform.
In order for the crate to be available at runtime, they need to be added as feature
