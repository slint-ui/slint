// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cfg_aliases::cfg_aliases;

fn main() {
    // Aliases collapsing the orthogonal axes of
    //   (skia-gl vs. skia-wgpu vs. none) x (wgpu-29 vs. wgpu-30).
    //
    //   enable_skia      = any skia backend compiled in
    //   enable_skia_wgpu = skia uses a wgpu surface
    //   skia_wgpu_30     = wgpu-30 path active inside new_wgpu (preferred when available)
    //   skia_wgpu_29     = wgpu-29 path active inside new_wgpu (used when -30 unavailable)
    cfg_aliases! {
        enable_skia: { any(
            feature = "renderer-skia-opengl",
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-29",
            feature = "unstable-wgpu-30"
        ) },
        enable_skia_wgpu: { any(
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-29",
            feature = "unstable-wgpu-30"
        ) },
        skia_wgpu_30: { any(feature = "renderer-skia-vulkan", feature = "unstable-wgpu-30") },
        skia_wgpu_29: { all(
            feature = "unstable-wgpu-29",
            not(any(feature = "renderer-skia-vulkan", feature = "unstable-wgpu-30"))
        ) },
        wgpu_surface: { any(
            feature = "unstable-wgpu-29",
            feature = "renderer-femtovg-wgpu",
            feature = "unstable-wgpu-30"
        ) },
    }
}
