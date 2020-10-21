/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use anyhow::Context;
use std::iter::Extend;
use std::path::{Path, PathBuf};

/// The root dir of the git repository
fn root_dir() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // $root/tests/driver_lib -> $root
    root.pop();
    root.pop();
    root
}

fn default_config() -> cbindgen::Config {
    cbindgen::Config {
        pragma_once: true,
        include_version: true,
        namespaces: Some(vec!["sixtyfps".into(), "cbindgen_private".into()]),
        line_length: 100,
        tab_width: 4,
        // Note: we might need to switch to C if we need to generate bindings for language that needs C headers
        language: cbindgen::Language::Cxx,
        cpp_compat: true,
        documentation: true,
        export: cbindgen::ExportConfig {
            rename: [("Signal".into(), "Signal<>".into())].iter().cloned().collect(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn gen_corelib(include_dir: &Path) -> anyhow::Result<()> {
    let mut config = default_config();
    config.export.include = [
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
        "Window",
        "TextInput",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();

    config.export.exclude = [
        "SharedString",
        "SharedArray",
        "Resource",
        "Color",
        "PathData",
        "PathElement",
        "sixtyfps_new_path_elements",
        "sixtyfps_new_path_events",
        "Property",
        "Slice",
        "PropertyHandleOpaque",
        "Signal",
        "sixtyfps_property_listener_scope_evaluate",
        "sixtyfps_property_listener_scope_is_dirty",
        "PropertyTrackerOpaque",
        "SignalOpaque",
        "ComponentWindow",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();

    let mut crate_dir = root_dir();
    crate_dir.extend(["sixtyfps_runtime", "corelib"].iter());

    let mut string_config = config.clone();
    string_config.export.exclude = vec!["SharedString".into()];
    cbindgen::Builder::new()
        .with_config(string_config)
        .with_src(crate_dir.join("string.rs"))
        .with_src(crate_dir.join("slice.rs"))
        .with_after_include("namespace sixtyfps { struct SharedString; }")
        .generate()
        .context("Unable to generate bindings for sixtyfps_string_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_string_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("sharedarray.rs"))
        .with_after_include("namespace sixtyfps { template<typename T> struct SharedArray; }")
        .generate()
        .context("Unable to generate bindings for sixtyfps_sharedarray_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_sharedarray_internal.h"));

    let mut properties_config = config.clone();
    properties_config.export.exclude.clear();
    cbindgen::Builder::new()
        .with_config(properties_config)
        .with_src(crate_dir.join("properties.rs"))
        .with_src(crate_dir.join("signals.rs"))
        .with_after_include("namespace sixtyfps { class Color; }")
        .generate()
        .context("Unable to generate bindings for sixtyfps_properties_internal.h")?
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
            "sixtyfps_component_window_free_graphics_resources",
            "sixtyfps_component_window_set_focus_item",
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
            Some(vec!["sixtyfps".into(), "cbindgen_private".into(), "types".into()]);
        cbindgen::Builder::new()
            .with_config(special_config)
            .with_src(crate_dir.join("graphics.rs"))
            .with_src(crate_dir.join("animations.rs"))
            //            .with_src(crate_dir.join("input.rs"))
            .with_src(crate_dir.join("item_rendering.rs"))
            .with_src(crate_dir.join("eventloop.rs"))
            .generate()
            .with_context(|| format!("Unable to generate bindings for {}", internal_header))?
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
    config.export.include.push("StandardListViewItem".into());
    cbindgen::Builder::new()
        .with_config(config)
        .with_src(crate_dir.join("lib.rs"))
        .with_include("vtable.h")
        .with_include("sixtyfps_string.h")
        .with_include("sixtyfps_sharedarray.h")
        .with_include("sixtyfps_properties.h")
        .with_include("sixtyfps_signals.h")
        .with_include("sixtyfps_resource.h")
        .with_include("sixtyfps_color.h")
        .with_include("sixtyfps_pathdata.h")
        .with_after_include(format!(
            "namespace sixtyfps {{ namespace private_api {{ enum class VersionCheck {{ Major = {}, Minor = {}, Patch = {} }}; struct ComponentWindow; }} namespace cbindgen_private {{ using sixtyfps::private_api::ComponentWindow; }} }}",
            0, 0, 2,
        ))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_internal.h"));

    Ok(())
}

fn gen_backend_qt(include_dir: &Path) -> anyhow::Result<()> {
    let mut config = default_config();
    config.export.include = [
        "NativeButton",
        "NativeSpinBox",
        "NativeCheckBox",
        "NativeSlider",
        "NativeGroupBox",
        "NativeLineEdit",
        "NativeScrollView",
        "NativeStandardListViewItem",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();

    let mut crate_dir = root_dir();
    crate_dir.extend(["sixtyfps_runtime", "rendering_backends", "qt"].iter());
    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_header("#include <sixtyfps_internal.h>")
        .generate()
        .context("Unable to generate bindings for sixtyfps_qt_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_qt_internal.h"));

    Ok(())
}

fn gen_backend_default(include_dir: &Path) -> anyhow::Result<()> {
    let config = default_config();
    let mut crate_dir = root_dir();
    crate_dir.extend(["sixtyfps_runtime", "rendering_backends", "default"].iter());
    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_header("#include <sixtyfps_internal.h>")
        .generate()
        .context("Unable to generate bindings for sixtyfps_default_backend_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_default_backend_internal.h"));

    Ok(())
}

/// Generate the headers.
/// `include_dir` is the output directory
pub fn gen_all(include_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(include_dir).context("Could not create the include directory")?;
    gen_corelib(include_dir)?;
    gen_backend_qt(include_dir)?;
    gen_backend_default(include_dir)?;
    Ok(())
}
