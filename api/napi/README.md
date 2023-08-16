<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->
# Slint-napi (pre-Alpha)

This is a restart of `Slint-node` based on [napi-rs](https://github.com/napi-rs/napi-rs). The current state is `pre-Alpha` what means it is not yet ready for testing and use.

## Implemented features

* js/ts wrapper for the `Slint` interpreter infrastructure
* js/ts wrapper for `Slint` types
    * `ImageData`
    * `Color`
    * `Brush`

## Missing features

* Possibility to run js/ts `async` code after window run call
* Generate a js/ts object-wrapper for the exported  `Slint` component
* Public access to Slint `globals`
* CI: Generate prebuild platform `node` packages
* Documentation generation
* js/ts wrapper for `Slint` types
    * `Model` (wip)