// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cfg_aliases::cfg_aliases;

fn main() {
    // Aliases collapsing the orthogonal axes of
    //   (skia-gl vs. skia-wgpu vs. none) x (wgpu-27 vs. wgpu-28).
    //
    //   enable_skia      = any skia backend compiled in
    //   enable_skia_wgpu = skia uses a wgpu surface
    //   skia_wgpu_28     = wgpu-28 path active inside new_wgpu
    //   skia_wgpu_27     = wgpu-27 path active inside new_wgpu
    cfg_aliases! {
        enable_skia: { any(
            feature = "renderer-skia-opengl",
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-27",
            feature = "unstable-wgpu-28"
        ) },
        enable_skia_wgpu: { any(
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-27",
            feature = "unstable-wgpu-28"
        ) },
        skia_wgpu_27: { feature = "unstable-wgpu-27" },
        skia_wgpu_28: { all(
            any(feature = "renderer-skia-vulkan", feature = "unstable-wgpu-28"),
            not(feature = "unstable-wgpu-27")
        ) },
    }
}
