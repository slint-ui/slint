extern crate cbindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    let include = ["Rectangle", "Image", "TouchArea", "Text", "ComponentVTable"]
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    let exclude = ["SharedString"].iter().map(|x| x.to_string()).collect::<Vec<String>>();

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
        export: cbindgen::ExportConfig { include, exclude, ..Default::default() },
        ..Default::default()
    };

    let mut include_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    include_dir.pop();
    include_dir.pop();
    include_dir.pop(); // target/{debug|release}/build/package/out/ -> target/{debug|release}
    include_dir.push("include");

    std::fs::create_dir_all(include_dir.clone()).unwrap();

    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("abi/string.rs"))
        .with_after_include("namespace sixtyfps { struct SharedString; }")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_string_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("abi/properties.rs"))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_properties_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("abi/signals.rs"))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_signals_internal.h"));

    cbindgen::Builder::new()
        .with_config(config)
        .with_src(crate_dir.join("abi/datastructures.rs"))
        .with_src(crate_dir.join("abi/primitives.rs"))
        .with_src(crate_dir.join("abi/model.rs"))
        .with_include("vtable.h")
        .with_include("sixtyfps_string.h")
        .with_include("sixtyfps_properties.h")
        .with_include("sixtyfps_signals.h")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_internal.h"));
}
