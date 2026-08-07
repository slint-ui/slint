// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
       // Targets where the wgpu-30 dependency has its Vulkan backend compiled in. On Apple
       // platforms that takes the `vulkan-portability` feature (MoltenVK); the wgpu-29
       // dependency has no such opt-in, so it keeps the plain target check.
       skia_wgpu_30_vulkan: { any(all(target_family = "unix", not(target_vendor = "apple")), feature = "wgpu-30-vulkan-portability") },
    }

    println!("cargo:rustc-check-cfg=cfg(slint_nightly_test)");
}
