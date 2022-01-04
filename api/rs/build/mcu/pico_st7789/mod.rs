// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

pub fn configure_linker() {
    println!("cargo:rustc-link-arg=--nmagic");
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-Tdefmt.x");
    let memory_x_path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "mcu", "pico_st7789"].iter().collect();
    println!("cargo:rustc-link-search={}", memory_x_path.to_string_lossy());
}
