// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_REMOTE");
    println!("cargo:rustc-check-cfg=cfg(slint_nightly_test)");
    // The slint!{} macro in remote.rs needs the experimental
    // new_with_existing_window constructor.
    if std::env::var_os("CARGO_FEATURE_REMOTE").is_some() {
        println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
    }
}
