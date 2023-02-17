// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
       enable_skia_renderer: { any(feature = "renderer-winit-skia", feature = "renderer-winit-skia-opengl")},
       // Upgrade to wasm once https://github.com/rust-windowing/winit/pull/2687 is merged & released.
       use_winit_theme: { any(target_family = "windows", target_os = "macos", target_os = "ios") },
    }

    println!("cargo:rerun-if-env-changed=RUST_FONTCONFIG_DLOPEN");
    let dlopen = std::env::var("RUST_FONTCONFIG_DLOPEN").is_ok();
    if dlopen {
        println!("cargo:rustc-cfg=feature=\"fontconfig-dlopen\"");
    }
}
