// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use cfg_aliases::cfg_aliases;

fn main() {
    // Remove when cfg_aliases supports this
    println!("cargo:rustc-check-cfg=cfg(enable_skia_renderer)");
    println!("cargo:rustc-check-cfg=cfg(enable_accesskit)");

    // Setup cfg aliases
    cfg_aliases! {
       enable_skia_renderer: { any(feature = "renderer-skia", feature = "renderer-skia-opengl", feature = "renderer-skia-vulkan")},
       enable_accesskit: { all(feature = "accessibility", not(target_arch = "wasm32")) },
    }
}
