// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore fobjc

fn main() {
    cc::Build::new().file("native_overlays.m").flag("-fobjc-arc").compile("native_overlays");
    println!("cargo:rustc-link-lib=framework=UIKit");
    println!("cargo:rerun-if-changed=native_overlays.m");
}
