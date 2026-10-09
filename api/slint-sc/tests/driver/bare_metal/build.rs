// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

//! Sets `cortex_m` for the M-profile targets, the Thumb-only ones, which
//! Rust has no stable `cfg` for.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(cortex_m)");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.starts_with("thumb") && target.contains("-none-") {
        println!("cargo::rustc-cfg=cortex_m");
    }
}
