extern crate cbindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    let include = [
        "Rectangle",
        "BorderRectangle",
        "Image",
        "TouchArea",
        "Flickable",
        "Text",
        "Path",
        "ComponentVTable",
        "Slice",
        "ComponentWindowOpaque",
        "PropertyAnimation",
        "EasingCurve",
        "TextHorizontalAlignment",
        "TextVerticalAlignment",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect::<Vec<String>>();

    let exclude = [
        "SharedString",
        "SharedArray",
        "Resource",
        "Color",
        "PathData",
        "PathElement",
        "sixtyfps_new_path_elements",
        "sixtyfps_new_path_events",
        "PinnedOptionalProp",
    ]
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
        .with_src(crate_dir.join("string.rs"))
        .with_src(crate_dir.join("slice.rs"))
        .with_after_include("namespace sixtyfps { struct SharedString; }")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_string_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("sharedarray.rs"))
        .with_after_include("namespace sixtyfps { template<typename T> struct SharedArray; }")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_sharedarray_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("properties.rs"))
        .with_src(crate_dir.join("signals.rs"))
        .with_after_include("namespace sixtyfps { struct Color; }")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_properties_internal.h"));

    for (rust_types, internal_header) in [
        (vec!["Resource"], "sixtyfps_resource_internal.h"),
        (vec!["Color"], "sixtyfps_color_internal.h"),
        (
            vec![
                "PathData",
                "PathElement",
                "sixtyfps_new_path_elements",
                "sixtyfps_new_path_events",
            ],
            "sixtyfps_pathdata_internal.h",
        ),
    ]
    .iter()
    {
        let mut special_config = config.clone();
        special_config.export.include = rust_types.iter().map(|s| s.to_string()).collect();
        special_config.export.exclude = [
            "sixtyfps_visit_item_tree",
            "sixtyfps_component_window_drop",
            "sixtyfps_component_window_run",
            "sixtyfps_component_window_get_scale_factor",
            "sixtyfps_component_window_set_scale_factor",
            "sixtyfps_new_path_elements",
            "sixtyfps_new_path_events",
        ]
        .iter()
        .filter(|exclusion| rust_types.iter().find(|inclusion| inclusion == exclusion).is_none())
        .map(|s| s.to_string())
        .collect();

        special_config.enumeration = cbindgen::EnumConfig {
            derive_tagged_enum_copy_assignment: true,
            derive_tagged_enum_copy_constructor: true,
            derive_tagged_enum_destructor: true,
            derive_helper_methods: true,
            private_default_tagged_enum_constructor: true,
            ..Default::default()
        };
        // Put the rust type in a deeper "types" namespace, so the use of same type in for example generated
        // Property<> fields uses the public `sixtyfps::Blah` type
        special_config.namespaces =
            Some(vec!["sixtyfps".into(), "internal".into(), "types".into()]);
        cbindgen::Builder::new()
            .with_config(special_config)
            .with_src(crate_dir.join("abi/datastructures.rs"))
            .with_src(crate_dir.join("graphics.rs"))
            .with_src(crate_dir.join("animations.rs"))
            //            .with_src(crate_dir.join("input.rs"))
            .with_src(crate_dir.join("item_rendering.rs"))
            .with_src(crate_dir.join("eventloop.rs"))
            .generate()
            .expect("Unable to generate bindings")
            .write_to_file(include_dir.join(internal_header));
    }

    config.export.body.insert(
        "ItemTreeNode".to_owned(),
        "    constexpr ItemTreeNode(Item_Body x) : item {x} {}
    constexpr ItemTreeNode(DynamicTree_Body x) : dynamic_tree{x} {}"
            .to_owned(),
    );
    config.export.body.insert(
        "CachedRenderingData".to_owned(),
        "    constexpr CachedRenderingData() : cache_index{}, cache_ok{} {}".to_owned(),
    );
    config.export.body.insert(
        "EasingCurve".to_owned(),
        "    constexpr EasingCurve() : tag(Tag::Linear), cubic_bezier{{0,0,1,1}} {}
    constexpr explicit EasingCurve(EasingCurve::Tag tag, float a, float b, float c, float d) : tag(tag), cubic_bezier{{a,b,c,d}} {}".into()
    );
    config
        .export
        .body
        .insert("Flickable".to_owned(), "    inline Flickable(); inline ~Flickable();".into());
    config.export.pre_body.insert("FlickableDataBox".to_owned(), "struct FlickableData;".into());
    cbindgen::Builder::new()
        .with_config(config)
        .with_src(crate_dir.join("abi/datastructures.rs"))
        .with_src(crate_dir.join("graphics.rs"))
        .with_src(crate_dir.join("animations.rs"))
        .with_src(crate_dir.join("input.rs"))
        .with_src(crate_dir.join("item_tree.rs"))
        .with_src(crate_dir.join("item_rendering.rs"))
        .with_src(crate_dir.join("items.rs"))
        .with_src(crate_dir.join("eventloop.rs"))
        .with_src(crate_dir.join("model.rs"))
        .with_src(crate_dir.join("tests.rs"))
        .with_src(crate_dir.join("layout.rs")) // FIXME: move in ABI?
        .with_include("vtable.h")
        .with_include("sixtyfps_string.h")
        .with_include("sixtyfps_sharedarray.h")
        .with_include("sixtyfps_properties.h")
        .with_include("sixtyfps_signals.h")
        .with_include("sixtyfps_resource.h")
        .with_include("sixtyfps_color.h")
        .with_include("sixtyfps_pathdata.h")
        .with_after_include(format!(
            "namespace sixtyfps {{ enum class VersionCheck {{ Major = {}, Minor = {}, Patch = {} }}; }}\nnamespace sixtyfps {{ namespace internal {{ template <typename T> using PinnedOptionalProp = Property<T> *; }} }}",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH")
        ))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_internal.h"));
}
