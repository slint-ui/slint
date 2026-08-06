// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
       skia_backend_opengl: { any(feature = "opengl", not(any(target_vendor = "apple", target_family = "windows", target_arch = "wasm32"))) },
       skia_backend_wgpu: { any(feature = "wgpu-29", feature = "wgpu-30") },
       skia_backend_software: { not(target_os = "android") },
       skia_backend_softbuffer: { all(skia_backend_software, feature = "softbuffer") },
       skia_windowed: { any(skia_backend_wgpu, skia_backend_opengl, skia_backend_softbuffer) },
    }

    println!("cargo:rustc-check-cfg=cfg(slint_nightly_test)");
}
