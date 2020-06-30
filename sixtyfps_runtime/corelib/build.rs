extern crate cbindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    let include = [
        "Rectangle",
        "Image",
        "TouchArea",
        "Text",
        "ComponentVTable",
        "Slice",
        "ComponentWindowOpaque",
        "PropertyAnimation",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect::<Vec<String>>();

    let exclude = ["SharedString", "SharedArray", "Resource", "Color"]
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    let mut config = cbindgen::Config {
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
        .with_src(crate_dir.join("abi/slice.rs"))
        .with_after_include("namespace sixtyfps { struct SharedString; }")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_string_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("abi/sharedarray.rs"))
        .with_after_include("namespace sixtyfps { template<typename T> struct SharedArray; }")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_sharedarray_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("abi/properties.rs"))
        .with_src(crate_dir.join("abi/signals.rs"))
        .with_after_include("namespace sixtyfps { struct Color; }")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_properties_internal.h"));

    let mut resource_config = config.clone();
    resource_config.export.include = vec!["Resource".into()];
    resource_config.export.exclude = vec![
        "sixtyfps_visit_item_tree".into(),
        "sixtyfps_component_window_drop".into(),
        "sixtyfps_component_window_run".into(),
    ];
    resource_config.enumeration = cbindgen::EnumConfig {
        derive_tagged_enum_copy_assignment: true,
        derive_tagged_enum_copy_constructor: true,
        derive_tagged_enum_destructor: true,
        derive_helper_methods: true,
        private_default_tagged_enum_constructor: true,
        ..Default::default()
    };
    // Put the "Recources" in a deeper "types" namespace, so the use of "Resource" in internal
    // uses the public `sixtyfps::Resource` type
    resource_config.namespaces = Some(vec!["sixtyfps".into(), "internal".into(), "types".into()]);
    cbindgen::Builder::new()
        .with_config(resource_config)
        .with_src(crate_dir.join("abi/datastructures.rs"))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_resource_internal.h"));

    let mut color_config = config.clone();
    color_config.export.include = vec!["Color".into()];
    color_config.export.exclude = vec![
        "sixtyfps_visit_item_tree".into(),
        "sixtyfps_component_window_drop".into(),
        "sixtyfps_component_window_run".into(),
    ];

    // Put the "Color" in a deeper "types" namespace, so the use of "Color" in internal
    // uses the public `sixtyfps::Color` type
    color_config.namespaces = Some(vec!["sixtyfps".into(), "internal".into(), "types".into()]);
    cbindgen::Builder::new()
        .with_config(color_config)
        .with_src(crate_dir.join("abi/datastructures.rs"))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_color_internal.h"));

    config.export.body.insert(
        "ItemTreeNode".to_owned(),
        "    constexpr ItemTreeNode(Item_Body x) : item {x} {}
    constexpr ItemTreeNode(DynamicTree_Body x) : dynamic_tree{x} {}"
            .to_owned(),
    );
    cbindgen::Builder::new()
        .with_config(config)
        .with_src(crate_dir.join("abi/datastructures.rs"))
        .with_src(crate_dir.join("abi/primitives.rs"))
        .with_src(crate_dir.join("abi/model.rs"))
        .with_src(crate_dir.join("layout.rs")) // FIXME: move in ABI?
        .with_include("vtable.h")
        .with_include("sixtyfps_string.h")
        .with_include("sixtyfps_sharedarray.h")
        .with_include("sixtyfps_properties.h")
        .with_include("sixtyfps_signals.h")
        .with_include("sixtyfps_resource.h")
        .with_include("sixtyfps_color.h")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_internal.h"));
}
