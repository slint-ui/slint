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
        backend_linuxkms_libseat: true,
        backend_linuxkms_libinput: true,
        renderer_femtovg: true,
        renderer_skia: true,
        renderer_skia_opengl: false,
        renderer_skia_software: false,
        renderer_skia_vulkan: false,
        renderer_software: true,
        gettext: true,
        image_pixel_format_rgb565: true,
        image_pixel_format_gray8: true,
        accessibility: true,
        system_testing: true,
        mcp: false,
        freestanding: true,
        experimental,
        tracing: false,
    };
    cbindgen::gen_all(&root, &generated_headers_dir, enabled_features)?;

    println!("Generated C++ headers in {}", generated_headers_dir.display());

    Ok(())
}

/// Rust's bare-metal Arm targets size a `#[repr(C)]` enum to fit its values,
/// while a C++ `enum class` without an underlying type is always an `int`.
#[test]
fn generated_enums_have_fixed_size() {
    const HAND_WRITTEN: &[&str] =
        &["slint_enums.h", "slint_enums_internal.h", "slint_live_preview_enums.h"];

    let root = super::root_dir();
    let out_dir = root.join("target/cbindgen_enum_test");
    cbindgen::gen_all(&root, &out_dir, cbindgen::EnabledFeatures::all()).unwrap();

    for entry in std::fs::read_dir(out_dir.join("private")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if !name.ends_with(".h") || HAND_WRITTEN.contains(&name) {
            continue;
        }
        for line in std::fs::read_to_string(&path).unwrap().lines().map(str::trim) {
            if let Some(declaration) = line.strip_prefix("enum class ")
                && declaration.ends_with('{')
            {
                assert!(
                    declaration.contains(':'),
                    "{name}: `{line}` has no underlying type. Use `#[repr(u8)]` or `#[repr(C, u8)]` on the Rust enum"
                );
            }
        }
    }
}
