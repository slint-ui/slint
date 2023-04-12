// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use anyhow::Context;
use std::io::Write;
use std::iter::Extend;
use std::path::{Path, PathBuf};

// cSpell: ignore compat constexpr corelib deps sharedvector pathdata

fn enums(path: &Path) -> anyhow::Result<()> {
    let mut enums =
        std::fs::File::create(path).context("Error creating slint_internal_enums.h file")?;
    writeln!(enums, "#pragma once")?;
    writeln!(enums, "// This file is auto-generated from {}", file!())?;
    writeln!(enums, "namespace slint::cbindgen_private {{")?;
    macro_rules! print_enums {
         ($( $(#[doc = $enum_doc:literal])* $(#[non_exhaustive])? enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
             $(
                $(writeln!(enums, "///{}", $enum_doc)?;)*
                writeln!(enums, "enum class {} {{", stringify!($Name))?;
                $(
                    $(writeln!(enums, "    ///{}", $value_doc)?;)*
                    writeln!(enums, "    {},", stringify!($Value).trim_start_matches("r#"))?;
                )*
                writeln!(enums, "}};")?;
             )*
         }
    }
    i_slint_common::for_each_enums!(print_enums);
    writeln!(enums, "}}")?;
    Ok(())
}

fn ensure_cargo_rerun_for_crate(
    crate_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    dependencies.push(crate_dir.to_path_buf());
    for entry in std::fs::read_dir(crate_dir)? {
        let entry = entry?;
        if entry.path().extension().map_or(false, |e| e == "rs") {
            dependencies.push(entry.path());
        }
    }
    Ok(())
}

fn default_config() -> cbindgen::Config {
    cbindgen::Config {
        pragma_once: true,
        include_version: true,
        namespaces: Some(vec!["slint".into(), "cbindgen_private".into()]),
        line_length: 100,
        tab_width: 4,
        // Note: we might need to switch to C if we need to generate bindings for language that needs C headers
        language: cbindgen::Language::Cxx,
        cpp_compat: true,
        documentation: true,
        export: cbindgen::ExportConfig {
            rename: [
                ("Callback".into(), "private_api::CallbackHelper".into()),
                ("VoidArg".into(), "void".into()),
                ("KeyEventArg".into(), "KeyEvent".into()),
                ("PointerEventArg".into(), "PointerEvent".into()),
                ("PointArg".into(), "Point".into()),
                ("FloatArg".into(), "float".into()),
                ("Coord".into(), "float".into()),
            ]
            .iter()
            .cloned()
            .collect(),
            ..Default::default()
        },
        defines: [
            ("target_pointer_width = 64".into(), "SLINT_TARGET_64".into()),
            ("target_pointer_width = 32".into(), "SLINT_TARGET_32".into()),
            ("target_arch = wasm32".into(), "SLINT_CPP_NEVER_WASM".into()), // Disable any wasm guarded code in C++, too - so that there are no gaps in enums.
        ]
        .iter()
        .cloned()
        .collect(),
        ..Default::default()
    }
}

fn gen_item_declarations(items: &[&str]) -> String {
    format!(
        r#"
namespace slint::private_api {{
#define SLINT_DECL_ITEM(ItemName) \
    extern const cbindgen_private::ItemVTable ItemName##VTable; \
    extern SLINT_DLL_IMPORT const cbindgen_private::ItemVTable* slint_get_##ItemName##VTable();

extern "C" {{
{}
}}

#undef SLINT_DECL_ITEM
}}
"#,
        items
            .iter()
            .map(|item_name| format!("SLINT_DECL_ITEM({});", item_name))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn gen_corelib(
    root_dir: &Path,
    include_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut config = default_config();

    let items = [
        "Empty",
        "Rectangle",
        "BorderRectangle",
        "ImageItem",
        "ClippedImage",
        "TouchArea",
        "FocusScope",
        "Flickable",
        "Text",
        "Path",
        "WindowItem",
        "TextInput",
        "Clip",
        "BoxShadow",
        "Rotate",
        "Opacity",
        "Layer",
    ];

    config.export.include = [
        "ComponentVTable",
        "Slice",
        "WindowAdapterRcOpaque",
        "PropertyAnimation",
        "EasingCurve",
        "TextHorizontalAlignment",
        "TextVerticalAlignment",
        "TextOverflow",
        "TextWrap",
        "ImageFit",
        "FillRule",
        "MouseCursor",
        "InputType",
        "StandardButtonKind",
        "DialogButtonRole",
        "PointerEventKind",
        "PointerEventButton",
        "PointerEvent",
        "Rect",
        "SortOrder",
    ]
    .iter()
    .chain(items.iter())
    .map(|x| x.to_string())
    .collect();

    let mut private_exported_types: std::collections::HashSet<String> =
        config.export.include.iter().cloned().collect();

    // included in generated_public.h
    let public_exported_types = [
        "TimerMode",
        "RenderingState",
        "SetRenderingNotifierError",
        "GraphicsAPI",
        "CloseRequestResponse",
        "StandardListViewItem",
        "Rgb8Pixel",
        "Rgba8Pixel",
    ];

    config.export.exclude = [
        "SharedString",
        "SharedVector",
        "ImageInner",
        "ImageCacheKey",
        "Image",
        "Color",
        "PathData",
        "PathElement",
        "Brush",
        "slint_new_path_elements",
        "slint_new_path_events",
        "Property",
        "Slice",
        "PropertyHandleOpaque",
        "Callback",
        "slint_property_listener_scope_evaluate",
        "slint_property_listener_scope_is_dirty",
        "PropertyTrackerOpaque",
        "CallbackOpaque",
        "WindowAdapterRc",
        "VoidArg",
        "KeyEventArg",
        "PointerEventArg",
        "PointArg",
        "Point",
        "slint_color_brighter",
        "slint_color_darker",
        "slint_image_size",
        "slint_image_path",
        "slint_image_load_from_path",
        "slint_image_load_from_embedded_data",
        "slint_image_from_embedded_textures",
        "Coord",
        "LogicalRect",
        "LogicalPoint",
        "LogicalLength",
    ]
    .iter()
    .chain(public_exported_types.iter())
    .map(|x| x.to_string())
    .collect();

    let mut crate_dir = root_dir.to_owned();
    crate_dir.extend(["internal", "core"].iter());

    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    let mut string_config = config.clone();
    string_config.export.exclude = vec!["SharedString".into()];
    string_config.export.body.insert(
        "Slice".to_owned(),
        "    const T &operator[](int i) const { return ptr[i]; }".to_owned(),
    );
    cbindgen::Builder::new()
        .with_config(string_config)
        .with_src(crate_dir.join("string.rs"))
        .with_src(crate_dir.join("slice.rs"))
        .with_after_include("namespace slint { struct SharedString; }")
        .generate()
        .context("Unable to generate bindings for slint_string_internal.h")?
        .write_to_file(include_dir.join("slint_string_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("sharedvector.rs"))
        .with_after_include("namespace slint { template<typename T> struct SharedVector; }")
        .generate()
        .context("Unable to generate bindings for slint_sharedvector_internal.h")?
        .write_to_file(include_dir.join("slint_sharedvector_internal.h"));

    let mut properties_config = config.clone();
    properties_config.export.exclude.clear();
    properties_config.export.include.push("StateInfo".into());
    properties_config
        .export
        .pre_body
        .insert("StateInfo".to_owned(), "    using Instant = uint64_t;".into());
    properties_config.structure.derive_eq = true;
    properties_config.structure.derive_neq = true;
    private_exported_types.extend(properties_config.export.include.iter().cloned());
    cbindgen::Builder::new()
        .with_config(properties_config)
        .with_src(crate_dir.join("properties.rs"))
        .with_src(crate_dir.join("properties/ffi.rs"))
        .with_src(crate_dir.join("callbacks.rs"))
        .with_after_include("namespace slint { class Color; class Brush; }")
        .generate()
        .context("Unable to generate bindings for slint_properties_internal.h")?
        .write_to_file(include_dir.join("slint_properties_internal.h"));

    for (rust_types, extra_excluded_types, internal_header, prelude) in [
        (
            vec![
                "ImageInner",
                "Image",
                "ImageCacheKey",
                "Size",
                "slint_image_size",
                "slint_image_path",
                "slint_image_load_from_path",
                "slint_image_load_from_embedded_data",
                "slint_image_from_embedded_textures",
                "SharedPixelBuffer",
                "SharedImageBuffer",
                "StaticTextures",
            ],
            vec!["Color"],
            "slint_image_internal.h",
            "namespace slint::cbindgen_private { struct ParsedSVG{}; struct HTMLImage{}; using namespace vtable; }",
        ),
        (
            vec!["Color", "slint_color_brighter", "slint_color_darker"],
            vec![],
            "slint_color_internal.h",
            "",
        ),
        (
            vec!["PathData", "PathElement", "slint_new_path_elements", "slint_new_path_events"],
            vec![],
            "slint_pathdata_internal.h",
            "",
        ),
        (
            vec!["Brush", "LinearGradient", "GradientStop", "RadialGradient"],
            vec!["Color"],
            "slint_brush_internal.h",
            "",
        ),
    ]
    .iter()
    {
        let mut special_config = config.clone();
        special_config.export.include = rust_types.iter().map(|s| s.to_string()).collect();
        special_config.export.exclude = [
            "slint_visit_item_tree",
            "slint_windowrc_drop",
            "slint_windowrc_clone",
            "slint_windowrc_show",
            "slint_windowrc_hide",
            "slint_windowrc_is_visible",
            "slint_windowrc_get_scale_factor",
            "slint_windowrc_set_scale_factor",
            "slint_windowrc_get_text_input_focused",
            "slint_windowrc_set_text_input_focused",
            "slint_windowrc_set_focus_item",
            "slint_windowrc_set_component",
            "slint_windowrc_show_popup",
            "slint_windowrc_set_rendering_notifier",
            "slint_windowrc_request_redraw",
            "slint_windowrc_on_close_requested",
            "slint_windowrc_position",
            "slint_windowrc_set_logical_position",
            "slint_windowrc_set_physical_position",
            "slint_windowrc_size",
            "slint_windowrc_set_logical_size",
            "slint_windowrc_set_physical_size",
            "slint_windowrc_dark_color_scheme",
            "slint_windowrc_dispatch_pointer_event",
            "slint_windowrc_dispatch_key_event",
            "slint_new_path_elements",
            "slint_new_path_events",
            "slint_color_brighter",
            "slint_color_darker",
            "slint_image_size",
            "slint_image_path",
            "slint_image_load_from_path",
            "slint_image_load_from_embedded_data",
            "slint_image_from_embedded_textures",
        ]
        .iter()
        .filter(|exclusion| !rust_types.iter().any(|inclusion| inclusion == *exclusion))
        .chain(extra_excluded_types.iter())
        .chain(public_exported_types.iter())
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
        special_config.structure.derive_eq = true;
        special_config.structure.derive_neq = true;
        // Put the rust type in a deeper "types" namespace, so the use of same type in for example generated
        // Property<> fields uses the public `slint::Blah` type
        special_config.namespaces =
            Some(vec!["slint".into(), "cbindgen_private".into(), "types".into()]);

        private_exported_types.extend(special_config.export.include.iter().cloned());

        special_config.after_includes = (!prelude.is_empty()).then(|| prelude.to_string());

        cbindgen::Builder::new()
            .with_config(special_config)
            .with_src(crate_dir.join("graphics.rs"))
            .with_src(crate_dir.join("graphics/color.rs"))
            .with_src(crate_dir.join("graphics/path.rs"))
            .with_src(crate_dir.join("graphics/brush.rs"))
            .with_src(crate_dir.join("graphics/image.rs"))
            .with_src(crate_dir.join("graphics/image/cache.rs"))
            .with_src(crate_dir.join("animations.rs"))
            //            .with_src(crate_dir.join("input.rs"))
            .with_src(crate_dir.join("item_rendering.rs"))
            .with_src(crate_dir.join("window.rs"))
            .with_include("slint_enums_internal.h")
            .generate()
            .with_context(|| format!("Unable to generate bindings for {}", internal_header))?
            .write_to_file(include_dir.join(internal_header));
    }

    // Generate a header file with some public API (enums, etc.)
    let mut public_config = config.clone();
    public_config.namespaces = Some(vec!["slint".into()]);
    public_config.export.item_types = vec![cbindgen::ItemType::Enums, cbindgen::ItemType::Structs];
    // Previously included types are now excluded (to avoid duplicates)
    public_config.export.exclude = private_exported_types.into_iter().collect();
    public_config.export.exclude.push("Point".into());
    public_config.export.include = public_exported_types.into_iter().map(str::to_string).collect();
    public_config.export.body.insert(
        "StandardListViewItem".to_owned(),
        "/// \\private\nfriend bool operator==(const StandardListViewItem&, const StandardListViewItem&) = default;".into(),
    );
    public_config.export.body.insert(
        "Rgb8Pixel".to_owned(),
        "/// \\private\nfriend bool operator==(const Rgb8Pixel&, const Rgb8Pixel&) = default;"
            .into(),
    );
    public_config.export.body.insert(
        "Rgba8Pixel".to_owned(),
        "/// \\private\nfriend bool operator==(const Rgba8Pixel&, const Rgba8Pixel&) = default;"
            .into(),
    );

    cbindgen::Builder::new()
        .with_config(public_config)
        .with_src(crate_dir.join("timers.rs"))
        .with_src(crate_dir.join("graphics.rs"))
        .with_src(crate_dir.join("window.rs"))
        .with_src(crate_dir.join("api.rs"))
        .with_src(crate_dir.join("model.rs"))
        .with_src(crate_dir.join("graphics/image.rs"))
        .with_include("slint_string.h")
        .with_after_include(format!(
            r"
/// This macro expands to the to the numeric value of the major version of Slint you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 1.
#define SLINT_VERSION_MAJOR {}
/// This macro expands to the to the numeric value of the minor version of Slint you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 5.
#define SLINT_VERSION_MINOR {}
/// This macro expands to the to the numeric value of the patch version of Slint you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 2.
#define SLINT_VERSION_PATCH {}
",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ))
        .generate()
        .context("Unable to generate bindings for slint_generated_public.h")?
        .write_to_file(include_dir.join("slint_generated_public.h"));

    config.export.body.insert(
        "ItemTreeNode".to_owned(),
        "    constexpr ItemTreeNode(Item_Body x) : item {x} {}
    constexpr ItemTreeNode(DynamicTree_Body x) : dynamic_tree{x} {}"
            .to_owned(),
    );
    config.export.body.insert(
        "CachedRenderingData".to_owned(),
        "    constexpr CachedRenderingData() : cache_index{}, cache_generation{} {}".to_owned(),
    );
    config.export.body.insert(
        "EasingCurve".to_owned(),
        "    constexpr EasingCurve() : tag(Tag::Linear), cubic_bezier{{0,0,1,1}} {}
    constexpr explicit EasingCurve(EasingCurve::Tag tag, float a, float b, float c, float d) : tag(tag), cubic_bezier{{a,b,c,d}} {}".into()
    );
    config.export.body.insert(
        "LayoutInfo".to_owned(),
        "    inline LayoutInfo merge(const LayoutInfo &other) const;
    friend inline LayoutInfo operator+(const LayoutInfo &a, const LayoutInfo &b) { return a.merge(b); }
    friend bool operator==(const LayoutInfo&, const LayoutInfo&) = default;".into(),
    );
    config.export.body.insert(
        "TableColumn".to_owned(),
        "friend bool operator==(const TableColumn&, const TableColumn&) = default;".into(),
    );
    config
        .export
        .body
        .insert("Flickable".to_owned(), "    inline Flickable(); inline ~Flickable();".into());
    config.export.pre_body.insert("FlickableDataBox".to_owned(), "struct FlickableData;".into());
    config.export.include.push("TableColumn".into());
    cbindgen::Builder::new()
        .with_config(config)
        .with_src(crate_dir.join("lib.rs"))
        .with_include("slint_config.h")
        .with_include("vtable.h")
        .with_include("slint_string.h")
        .with_include("slint_sharedvector.h")
        .with_include("slint_properties.h")
        .with_include("slint_callbacks.h")
        .with_include("slint_color.h")
        .with_include("slint_image.h")
        .with_include("slint_pathdata.h")
        .with_include("slint_brush.h")
        .with_include("slint_generated_public.h")
        .with_include("slint_enums_internal.h")
        .with_include("slint_point.h")
        .with_after_include(
            r"
namespace slint {
    namespace private_api { class WindowAdapterRc; }
    namespace cbindgen_private {
        using slint::private_api::WindowAdapterRc;
        using namespace vtable;
        struct KeyEvent; struct PointerEvent;
        using private_api::Property;
        using private_api::PathData;
        using private_api::Point;
        struct Rect;
        using LogicalRect = Rect;
        using LogicalPoint = Point2D<float>;
        using LogicalLength = float;
    }
}",
        )
        .with_trailer(gen_item_declarations(&items))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("slint_internal.h"));

    Ok(())
}

fn gen_backend_qt(
    root_dir: &Path,
    include_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut config = default_config();

    let items = [
        "NativeButton",
        "NativeSpinBox",
        "NativeCheckBox",
        "NativeSlider",
        "NativeGroupBox",
        "NativeLineEdit",
        "NativeScrollView",
        "NativeStandardListViewItem",
        "NativeTableHeaderSection",
        "NativeComboBox",
        "NativeComboBoxPopup",
        "NativeTabWidget",
        "NativeTab",
        "NativeStyleMetrics",
    ];

    config.export.include = items.iter().map(|x| x.to_string()).collect();
    config.export.exclude = vec!["FloatArg".into()];

    config.export.body.insert(
        "NativeStyleMetrics".to_owned(),
        "    inline explicit NativeStyleMetrics(void* = nullptr); inline ~NativeStyleMetrics();"
            .to_owned(),
    );

    let mut crate_dir = root_dir.to_owned();
    crate_dir.extend(["internal", "backends", "qt"].iter());

    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_include("slint_internal.h")
        .with_trailer(gen_item_declarations(&items))
        .generate()
        .context("Unable to generate bindings for slint_qt_internal.h")?
        .write_to_file(include_dir.join("slint_qt_internal.h"));

    Ok(())
}

fn gen_backend(
    root_dir: &Path,
    include_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let config = default_config();
    let mut crate_dir = root_dir.to_owned();
    crate_dir.extend(["api", "cpp"].iter());

    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_include("slint_image_internal.h")
        .with_include("slint_internal.h")
        .generate()
        .context("Unable to generate bindings for slint_backend_internal.h")?
        .write_to_file(include_dir.join("slint_backend_internal.h"));

    Ok(())
}

fn gen_interpreter(
    root_dir: &Path,
    include_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut config = default_config();
    // Avoid Value, just export ValueOpaque.
    config.export.exclude = IntoIterator::into_iter([
        "Value",
        "ValueType",
        "PropertyDescriptor",
        "Diagnostic",
        "PropertyDescriptor",
    ])
    .map(String::from)
    .collect();
    let mut crate_dir = root_dir.to_owned();

    crate_dir.extend(["internal", "interpreter"].iter());
    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    // Generate a header file with some public API (enums, etc.)
    let mut public_config = config.clone();
    public_config.namespaces = Some(vec!["slint".into(), "interpreter".into()]);
    public_config.export.item_types = vec![cbindgen::ItemType::Enums, cbindgen::ItemType::Structs];

    public_config.export.exclude = IntoIterator::into_iter([
        "ComponentCompilerOpaque",
        "ComponentDefinitionOpaque",
        "ModelAdaptorVTable",
        "StructIteratorOpaque",
        "ComponentInstance",
        "StructIteratorResult",
        "ValueOpaque",
        "StructOpaque",
        "ModelNotifyOpaque",
    ])
    .map(String::from)
    .collect();

    cbindgen::Builder::new()
        .with_config(public_config)
        .with_crate(crate_dir.clone())
        .generate()
        .context("Unable to generate bindings for slint_interpreter_generated_public.h")?
        .write_to_file(include_dir.join("slint_interpreter_generated_public.h"));

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_include("slint_internal.h")
        .with_include("slint_interpreter_generated_public.h")
        .with_after_include(
            r"
            namespace slint::cbindgen_private {
                struct Value;
                using slint::interpreter::ValueType;
                using slint::interpreter::PropertyDescriptor;
                using slint::interpreter::Diagnostic;
            }",
        )
        .generate()
        .context("Unable to generate bindings for slint_interpreter_internal.h")?
        .write_to_file(include_dir.join("slint_interpreter_internal.h"));

    Ok(())
}

/// Generate the headers.
/// `root_dir` is the root directory of the slint git repo
/// `include_dir` is the output directory
/// Returns the list of all paths that contain dependencies to the generated output. If you call this
/// function from build.rs, feed each entry to stdout prefixed with `cargo:rerun-if-changed=`.
pub fn gen_all(root_dir: &Path, include_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    proc_macro2::fallback::force(); // avoid a abort if panic=abort is set
    std::fs::create_dir_all(include_dir).context("Could not create the include directory")?;
    let mut deps = Vec::new();
    enums(&include_dir.join("slint_enums_internal.h"))?;
    gen_corelib(root_dir, include_dir, &mut deps)?;
    gen_backend_qt(root_dir, include_dir, &mut deps)?;
    gen_backend(root_dir, include_dir, &mut deps)?;
    gen_interpreter(root_dir, include_dir, &mut deps)?;
    Ok(deps)
}
