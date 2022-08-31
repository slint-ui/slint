// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
       enable_skia_renderer: { any(feature = "renderer-skia", feature = "renderer-skia-opengl")},
       skia_backend_opengl: { feature = "renderer-skia-opengl" },
       skia_backend_metal: { all(target_os = "macos", not(feature = "renderer-skia-opengl")) },
       skia_backend_d3d: { all(target_family = "windows", not(feature = "renderer-skia-opengl")) },
    }
}
