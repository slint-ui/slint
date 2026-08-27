// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore cppdocs

use anyhow::Context;

#[path = "../../api/cpp/cbindgen.rs"]
mod cbindgen;

/// Generate the cbindgen C++ headers used by the documentation build into
/// `target/cppdocs/generated_include`. The documentation itself is built by the
/// Astro site in `docs/cpp` (Doxygen XML + a Markdown converter); this task only
/// produces the generated headers Doxygen needs. It is normally invoked by that
/// site's `gen:api` script (`docs/cpp/scripts/generate-api.ts`) rather than by
/// hand.
pub fn generate(experimental: bool) -> Result<(), Box<dyn std::error::Error>> {
    let root = super::root_dir();

    let docs_build_dir = root.join("target/cppdocs");
    std::fs::create_dir_all(docs_build_dir.as_path()).context("Error creating docs build dir")?;

    let generated_headers_dir = docs_build_dir.join("generated_include");
    let enabled_features = cbindgen::EnabledFeatures {
        interpreter: true,
        live_preview: false,
        testing: true,
        backend_qt: true,
        backend_winit: true,
        backend_winit_x11: false,
        backend_winit_wayland: false,
        backend_linuxkms: true,
        backend_linuxkms_noseat: false,
        renderer_femtovg: true,
        renderer_skia: true,
        renderer_skia_opengl: false,
        renderer_skia_vulkan: false,
        renderer_software: true,
        gettext: true,
        accessibility: true,
        system_testing: true,
        mcp: false,
        freestanding: true,
        experimental,
    };
    cbindgen::gen_all(&root, &generated_headers_dir, enabled_features)?;

    println!("Generated C++ headers in {}", generated_headers_dir.display());

    Ok(())
}
