// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
       skia_backend_opengl: { any(feature = "opengl", not(any(target_os = "macos", target_family = "windows", target_arch = "wasm32"))) },
       skia_backend_metal: { all(target_os = "macos", not(feature = "opengl")) },
       skia_backend_d3d: { all(target_family = "windows", not(feature = "opengl")) },
       skia_backend_vulkan: { feature = "vulkan" },
    }
}
