extern crate cbindgen;

use std::env;

fn main() {
    let config = cbindgen::Config {
        pragma_once: true,
        include_version: true,
        namespaces: Some(vec!["sixtyfps".into(), "internal".into()]),
        line_length: 100,
        tab_width: 4,
        // Note: we might need to switch to C if we need to generate bindings for language that needs C headers
        language: cbindgen::Language::Cxx,
        cpp_compat: true,
        documentation: true,
        export: cbindgen::ExportConfig { ..Default::default() },
        ..Default::default()
    };

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_header("#include <sixtyfps_internal.h>")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(env::var("OUT_DIR").unwrap() + "/sixtyfps_gl_internal.h");
}
