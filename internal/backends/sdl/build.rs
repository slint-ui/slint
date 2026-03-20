// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    // Link against SDL3
    if let Ok(lib) = pkg_config::probe_library("sdl3") {
        for path in &lib.include_paths {
            println!("cargo:include={}", path.display());
        }
    } else {
        // Fallback: try to find SDL3 via sdl3-config or standard paths
        println!("cargo:rustc-link-lib=SDL3");
    }

    // Link against SDL3_ttf
    if let Ok(lib) = pkg_config::probe_library("SDL3_ttf") {
        for path in &lib.include_paths {
            println!("cargo:include={}", path.display());
        }
    } else {
        println!("cargo:rustc-link-lib=SDL3_ttf");
    }
}
