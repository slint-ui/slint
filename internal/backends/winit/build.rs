// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
       ios_and_friends: { all(target_vendor = "apple", not(target_os = "macos"))},
       enable_skia_renderer: { any(feature = "renderer-skia", feature = "renderer-skia-opengl", feature = "renderer-skia-vulkan", ios_and_friends) },
       enable_femtovg_renderer: { any(feature = "renderer-femtovg", feature = "renderer-femtovg-wgpu") },
       enable_accesskit: { all(feature = "accessibility", not(target_arch = "wasm32")) },
       supports_opengl: { all(any(enable_skia_renderer, feature = "renderer-femtovg"), not(ios_and_friends)) },
       use_winit_theme: { any(target_family = "windows", target_vendor = "apple", target_arch = "wasm32", target_os = "android") },
       muda: { all(feature = "muda", any(target_os = "windows", target_os = "macos")) },
    }
    // This uses `web_sys_unstable_api`, which is typically set via `RUST_FLAGS`
    println!("cargo:rustc-check-cfg=cfg(web_sys_unstable_apis)");
}
