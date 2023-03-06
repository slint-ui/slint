// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

fn main() {
    println!("cargo:rerun-if-env-changed=RUST_FONTCONFIG_DLOPEN");
    let dlopen = std::env::var("RUST_FONTCONFIG_DLOPEN").is_ok();
    if dlopen {
        println!("cargo:rustc-cfg=feature=\"fontconfig-dlopen\"");
    }
}
