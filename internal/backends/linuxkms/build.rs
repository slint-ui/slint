// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cfg_aliases::cfg_aliases;

fn main() {
    // Aliases collapsing the orthogonal axes of
    //   (skia-gl vs. skia-wgpu vs. none).
    //
    //   enable_skia      = any skia backend compiled in
    //   enable_skia_wgpu = skia uses a wgpu surface
    cfg_aliases! {
        enable_skia: { any(
            feature = "renderer-skia-opengl",
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-29"
        ) },
        enable_skia_wgpu: { any(
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-29"
        ) },
    }
}
