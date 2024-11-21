// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    napi_build::setup();

    // workaround bug that the `#[napi]` macro generate some invalid `#[cfg(feature="...")]`
    println!("cargo:rustc-check-cfg=cfg(feature,values(\"noop\", \"used_linker\"))");
}
