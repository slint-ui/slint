extern crate cbindgen;

use std::env;

fn main() {
    let include = ["Rectangle", "Image", "ComponentType"]
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

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
        export: cbindgen::ExportConfig { include, ..Default::default() },
        ..Default::default()
    };

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_header("#include <vtable.h>")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(env::var("OUT_DIR").unwrap() + "/sixtyfps_internal.h");
}
