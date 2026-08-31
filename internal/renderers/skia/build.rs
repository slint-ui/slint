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
       // Targets where wgpu and Skia both have their Vulkan backend compiled in.
       // This only happens if wgpu has been enabled, and, on windows an mac only, the "vulkan"
       // feature is enabled. On linux and other non-macos unixes, skia_wgpu_vulkan is true by
       // default, and enabling "vulkan" feature would only serve to have it prioritized over
       // opengl at runtime.
       skia_wgpu_vulkan: { all(skia_backend_wgpu, any(all(target_family = "unix", not(target_vendor = "apple")), all(any(target_vendor = "apple", target_family = "windows"), feature = "vulkan"))) },
       skia_windowed: { any(skia_backend_wgpu, skia_backend_opengl, skia_backend_softbuffer) },
    }

    println!("cargo:rustc-check-cfg=cfg(slint_nightly_test)");
}
