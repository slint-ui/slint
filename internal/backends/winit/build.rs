// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
       ios_and_friends: { all(target_vendor = "apple", not(target_os = "macos"))},
       enable_skia_gpu: { any(feature = "renderer-skia", feature = "renderer-skia-opengl", feature = "renderer-skia-vulkan", ios_and_friends) },
       enable_skia_renderer: { any(enable_skia_gpu, feature = "renderer-skia-software") },
       enable_skia_wgpu: { all(enable_skia_renderer, any(enable_skia_gpu, feature = "unstable-wgpu-29", feature = "unstable-wgpu-30")) },
       skia_wgpu_30: { all(enable_skia_renderer, any(enable_skia_gpu, feature = "unstable-wgpu-30")) },
       skia_software_only: { all(feature = "renderer-skia-software", not(any(enable_skia_gpu, target_os = "android"))) },
       enable_femtovg_renderer: { any(feature = "renderer-femtovg", feature = "renderer-femtovg-wgpu") },
       enable_accesskit: { all(feature = "accessibility", not(target_arch = "wasm32")) },
       supports_opengl: { all(any(feature = "renderer-skia-opengl", feature = "renderer-femtovg"), not(ios_and_friends)) },
       supports_metal: { all(target_vendor = "apple", enable_skia_wgpu) },
       supports_direct3d: { all(target_family = "windows", enable_skia_wgpu) },
       supports_vulkan: { all(enable_skia_wgpu, any(all(target_family = "unix", not(target_vendor = "apple")), all(any(target_vendor = "apple", target_family = "windows"), feature = "renderer-skia-vulkan"))) },
       xdg_desktop_settings: { not(any(target_family = "windows", target_vendor = "apple", target_arch = "wasm32", target_os = "android")) },
       muda: { all(feature = "muda", any(target_os = "windows", target_os = "macos")) },
    }
    println!("cargo:rustc-check-cfg=cfg(slint_nightly_test)");
}
