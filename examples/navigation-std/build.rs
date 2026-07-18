// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    // The navigator is experimental; enable it for the `slint!` macro that
    // compiles navigation.slint at build time.
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
}
