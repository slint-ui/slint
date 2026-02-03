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

    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent-light".into())
        .with_sdf_fonts(true)
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("../ui/app-window.slint", config).unwrap();
}
