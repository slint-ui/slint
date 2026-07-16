// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The expansion of cfg_aliases! places macro-emitted semicolons in expression
// position, which nightly rejects by default since 2026-07. Allow it until a
// fixed cfg-aliases release is available.
#![allow(semicolon_in_expressions_from_macros)]

use cfg_aliases::cfg_aliases;

fn main() {
    // Aliases collapsing the orthogonal axes of
    //   (skia-gl vs. skia-wgpu vs. none) x (wgpu-28 vs. wgpu-29).
    //
    //   enable_skia      = any skia backend compiled in
    //   enable_skia_wgpu = skia uses a wgpu surface
    //   skia_wgpu_29     = wgpu-29 path active inside new_wgpu (preferred when available)
    //   skia_wgpu_28     = wgpu-28 path active inside new_wgpu (used when -29 unavailable)
    cfg_aliases! {
        enable_skia: { any(
            feature = "renderer-skia-opengl",
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-28",
            feature = "unstable-wgpu-29"
        ) },
        enable_skia_wgpu: { any(
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-28",
            feature = "unstable-wgpu-29"
        ) },
        skia_wgpu_29: { any(feature = "renderer-skia-vulkan", feature = "unstable-wgpu-29") },
        skia_wgpu_28: { all(
            feature = "unstable-wgpu-28",
            not(any(feature = "renderer-skia-vulkan", feature = "unstable-wgpu-29"))
        ) },
        wgpu_surface: { any(
            feature = "unstable-wgpu-28",
            feature = "renderer-femtovg-wgpu",
            feature = "unstable-wgpu-29"
        ) },
    }
}
