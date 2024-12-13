// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
       skia_backend_opengl: { any(feature = "opengl", not(any(target_vendor = "apple", target_family = "windows", target_arch = "wasm32"))) },
       skia_backend_metal: { all(target_vendor = "apple", not(feature = "opengl")) },
       skia_backend_d3d: { all(target_family = "windows", not(feature = "opengl")) },
       skia_backend_vulkan: { feature = "vulkan" },
       skia_backend_software: { not(target_os = "android") },
    }
}
