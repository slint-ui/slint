// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore fobjc

fn main() {
    println!("cargo:rerun-if-changed=native_scroll.m");
    println!("cargo:rerun-if-changed=scene_delegate.m");
    // Lets `cargo check` run on other hosts.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("ios") {
        return;
    }
    cc::Build::new()
        .files(["native_scroll.m", "scene_delegate.m"])
        .flag("-fobjc-arc")
        .compile("native_scroll");
    println!("cargo:rustc-link-lib=framework=UIKit");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
}
