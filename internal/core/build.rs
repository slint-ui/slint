// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

fn main() {
    println!("cargo:rustc-check-cfg=cfg(slint_debug_property)");
    println!("cargo:rustc-check-cfg=cfg(cbindgen)");
    println!("cargo:rustc-check-cfg=cfg(slint_int_coord)");
}
