// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=SAFE_UI_WIDTH");
    println!("cargo:rerun-if-env-changed=SAFE_UI_HEIGHT");
    println!("cargo:rerun-if-env-changed=SAFE_UI_SCALE_FACTOR");

    let bindings = bindgen::Builder::default()
        .header("src/slint-safeui-platform-interface.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .use_core()
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("bindings.rs")).expect("Couldn't write bindings!");

    // Compile the `.slint` UI through the Slint SC generator.  Slint SC
    // draws directly into physical pixels, so we bake the scale factor in
    // at compile time: every `Npx` literal becomes `N * scale_factor`
    // physical pixels.  Keep this in sync with `SCALE_FACTOR` in `lib.rs`.
    let scale_factor: f32 = match option_env!("SAFE_UI_SCALE_FACTOR") {
        Some(s) => s.parse().unwrap_or(2.0),
        None => 2.0,
    };
    let config = slint_build::CompilerConfiguration::new()
        .with_safety_critical(true)
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer)
        .with_scale_factor(scale_factor);
    slint_build::compile_with_config("../ui/app-window.slint", config).unwrap();
}
