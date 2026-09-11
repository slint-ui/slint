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
        // The DRM wgpu-29 surface target is not skia specific: the vello
        // renderer uses it too.
        wgpu_29_surface_target: { any(feature = "unstable-wgpu-29", feature = "renderer-vello") },
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
