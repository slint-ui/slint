// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore bufs dmabuf

use cfg_aliases::cfg_aliases;

fn main() {
    // The cursor tests set `MouseCursor.custom()` in `slint!` macros, which is experimental.
    // The macro reads this while compiling the crate, and only the tests use it.
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=custom-mouse-cursor");

    // Aliases collapsing the orthogonal axes of
    //   (skia-gl vs. skia-wgpu vs. none) x (wgpu-29 vs. wgpu-30).
    //
    //   enable_skia      = any skia backend compiled in
    //   enable_skia_wgpu = skia uses a wgpu surface
    //   skia_wgpu_30     = wgpu-30 path active inside new_wgpu (preferred when available)
    //   skia_wgpu_29     = wgpu-29 path active inside new_wgpu (used when -30 unavailable)
    //   gbm_dmabuf       = the GBM dma-buf fallback display is compiled in
    cfg_aliases! {
        enable_skia: { any(
            feature = "renderer-skia-opengl",
            feature = "renderer-skia-software",
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
        // The dma-buf import is a wgpu-30 API.
        gbm_dmabuf: { any(
            feature = "renderer-skia-vulkan",
            feature = "unstable-wgpu-30",
            feature = "renderer-femtovg-wgpu"
        ) },
        wgpu_30_surface_target: { any(
            feature = "unstable-wgpu-30",
            feature = "renderer-femtovg-wgpu",
            feature = "renderer-vello"
        ) },
        wgpu_surface: { any(
            feature = "unstable-wgpu-29",
            feature = "renderer-femtovg-wgpu",
            feature = "unstable-wgpu-30",
            feature = "renderer-vello"
        ) },
    }

    // Like the Qt backend, a missing library is a warning, not an error, so that
    // --all-features builds work everywhere. libdlmclient.pc ships with the AGL
    // drm-lease-manager.
    println!("cargo:rustc-check-cfg=cfg(have_libdlmclient)");
    if std::env::var_os("CARGO_FEATURE_LIBDLMCLIENT").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
    {
        match pkg_config::probe_library("libdlmclient") {
            Ok(_) => println!("cargo:rustc-cfg=have_libdlmclient"),
            Err(_) => println!(
                "cargo:warning=pkg-config can't find libdlmclient, so the LinuxKMS backend won't be able to obtain DRM leases from the drm-lease-manager. Point PKG_CONFIG_PATH at the directory of libdlmclient.pc"
            ),
        }
    }
}
