// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The expansion of cfg_aliases! places macro-emitted semicolons in expression
// position, which nightly rejects by default since 2026-07. Allow it until a
// fixed cfg-aliases release is available.
#![allow(semicolon_in_expressions_from_macros)]

use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
       skia_backend_opengl: { any(feature = "opengl", not(any(target_vendor = "apple", target_family = "windows", target_arch = "wasm32"))) },
       skia_backend_metal: { all(target_vendor = "apple", not(feature = "opengl")) },
       skia_backend_vulkan: { feature = "vulkan" },
       skia_backend_software: { not(target_os = "android") },
       skia_backend_softbuffer: { all(skia_backend_software, feature = "softbuffer") },
       skia_windowed: { any(skia_backend_vulkan, skia_backend_opengl, skia_backend_metal, skia_backend_softbuffer) },
    }

    println!("cargo:rustc-check-cfg=cfg(slint_nightly_test)");
}
