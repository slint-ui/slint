// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    cc::Build::new().file("native_scroll.m").flag("-fobjc-arc").compile("native_scroll");
    println!("cargo:rustc-link-lib=framework=UIKit");
    println!("cargo:rerun-if-changed=native_scroll.m");
}
