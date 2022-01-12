// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use anyhow::Context;
use std::iter::Extend;
use std::path::{Path, PathBuf};

// cspell::ignore compat constexpr corelib sharedvector pathdata

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
        namespaces: Some(vec!["sixtyfps".into(), "cbindgen_private".into()]),
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
            ]
            .iter()
            .cloned()
            .collect(),
            ..Default::default()
        },
        defines: [
            ("target_pointer_width = 64".into(), "SIXTYFPS_TARGET_64".into()),
            ("target_pointer_width = 32".into(), "SIXTYFPS_TARGET_32".into()),
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
namespace sixtyfps::private_api {{
#define SIXTYFPS_DECL_ITEM(ItemName) \
    extern const cbindgen_private::ItemVTable ItemName##VTable; \
    extern SIXTYFPS_DLL_IMPORT const cbindgen_private::ItemVTable* sixtyfps_get_##ItemName##VTable();

extern "C" {{
{}
}}

#undef SIXTYFPS_DECL_ITEM
}}
"#,
        items
            .iter()
            .map(|item_name| format!("SIXTYFPS_DECL_ITEM({});", item_name))
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
    ];

    config.export.include = [
        "ComponentVTable",
        "Slice",
        "WindowRcOpaque",
        "PropertyAnimation",
        "EasingCurve",
        "TextHorizontalAlignment",
        "TextVerticalAlignment",
        "TextOverflow",
        "TextWrap",
        "ImageFit",
        "FillRule",
        "MouseCursor",
        "StandardButtonKind",
        "DialogButtonRole",
        "PointerEventKind",
        "PointerEventButton",
        "PointerEvent",
    ]
    .iter()
    .chain(items.iter())
    .map(|x| x.to_string())
    .collect();

    config.export.exclude = [
        "SharedString",
        "SharedVector",
        "ImageInner",
        "Image",
        "Color",
        "PathData",
        "PathElement",
        "Brush",
        "sixtyfps_new_path_elements",
        "sixtyfps_new_path_events",
        "Property",
        "Slice",
        "PropertyHandleOpaque",
        "Callback",
        "sixtyfps_property_listener_scope_evaluate",
        "sixtyfps_property_listener_scope_is_dirty",
        "PropertyTrackerOpaque",
        "CallbackOpaque",
        "WindowRc",
        "VoidArg",
        "KeyEventArg",
        "PointerEventArg",
        "PointArg",
        "Point",
        "sixtyfps_color_brighter",
        "sixtyfps_color_darker",
        "sixtyfps_image_size",
        "sixtyfps_image_path",
        "TimerMode", // included in generated_public.h
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();

    let mut crate_dir = root_dir.to_owned();
    crate_dir.extend(["sixtyfps_runtime", "corelib"].iter());

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
        .with_after_include("namespace sixtyfps { struct SharedString; }")
        .generate()
        .context("Unable to generate bindings for sixtyfps_string_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_string_internal.h"));

    cbindgen::Builder::new()
        .with_config(config.clone())
        .with_src(crate_dir.join("sharedvector.rs"))
        .with_after_include("namespace sixtyfps { template<typename T> struct SharedVector; }")
        .generate()
        .context("Unable to generate bindings for sixtyfps_sharedvector_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_sharedvector_internal.h"));

    let mut properties_config = config.clone();
    properties_config.export.exclude.clear();
    properties_config.export.include.push("StateInfo".into());
    properties_config
        .export
        .pre_body
        .insert("StateInfo".to_owned(), "    using Instant = uint64_t;".into());
    properties_config.structure.derive_eq = true;
    properties_config.structure.derive_neq = true;
    cbindgen::Builder::new()
        .with_config(properties_config)
        .with_src(crate_dir.join("properties.rs"))
        .with_src(crate_dir.join("callbacks.rs"))
        .with_after_include("namespace sixtyfps { class Color; class Brush; }")
        .generate()
        .context("Unable to generate bindings for sixtyfps_properties_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_properties_internal.h"));

    for (rust_types, extra_excluded_types, internal_header) in [
        (
            vec![
                "ImageInner",
                "Image",
                "Size",
                "IntSize",
                "sixtyfps_image_size",
                "sixtyfps_image_path",
                "SharedPixelBuffer",
                "SharedImageBuffer",
            ],
            vec!["Color"],
            "sixtyfps_image_internal.h",
        ),
        (
            vec!["Color", "sixtyfps_color_brighter", "sixtyfps_color_darker"],
            vec![],
            "sixtyfps_color_internal.h",
        ),
        (
            vec![
                "PathData",
                "PathElement",
                "sixtyfps_new_path_elements",
                "sixtyfps_new_path_events",
            ],
            vec![],
            "sixtyfps_pathdata_internal.h",
        ),
        (
            vec!["Brush", "LinearGradient", "GradientStop"],
            vec!["Color"],
            "sixtyfps_brush_internal.h",
        ),
    ]
    .iter()
    {
        let mut special_config = config.clone();
        special_config.export.include = rust_types.iter().map(|s| s.to_string()).collect();
        special_config.export.exclude = [
            "sixtyfps_visit_item_tree",
            "sixtyfps_windowrc_drop",
            "sixtyfps_windowrc_clone",
            "sixtyfps_windowrc_show",
            "sixtyfps_windowrc_hide",
            "sixtyfps_windowrc_get_scale_factor",
            "sixtyfps_windowrc_set_scale_factor",
            "sixtyfps_windowrc_free_graphics_resources",
            "sixtyfps_windowrc_set_focus_item",
            "sixtyfps_windowrc_set_component",
            "sixtyfps_windowrc_show_popup",
            "sixtyfps_new_path_elements",
            "sixtyfps_new_path_events",
            "sixtyfps_color_brighter",
            "sixtyfps_color_darker",
            "sixtyfps_image_size",
            "sixtyfps_image_path",
        ]
        .iter()
        .filter(|exclusion| !rust_types.iter().any(|inclusion| inclusion == *exclusion))
        .chain(extra_excluded_types.iter())
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
        // Property<> fields uses the public `sixtyfps::Blah` type
        special_config.namespaces =
            Some(vec!["sixtyfps".into(), "cbindgen_private".into(), "types".into()]);
        cbindgen::Builder::new()
            .with_config(special_config)
            .with_src(crate_dir.join("graphics.rs"))
            .with_src(crate_dir.join("graphics/color.rs"))
            .with_src(crate_dir.join("graphics/path.rs"))
            .with_src(crate_dir.join("graphics/brush.rs"))
            .with_src(crate_dir.join("graphics/image.rs"))
            .with_src(crate_dir.join("animations.rs"))
            //            .with_src(crate_dir.join("input.rs"))
            .with_src(crate_dir.join("item_rendering.rs"))
            .with_src(crate_dir.join("window.rs"))
            .generate()
            .with_context(|| format!("Unable to generate bindings for {}", internal_header))?
            .write_to_file(include_dir.join(internal_header));
    }

    // Generate a header file with some public API (enums, etc.)
    let mut public_config = config.clone();
    public_config.namespaces = Some(vec!["sixtyfps".into()]);
    public_config.export.item_types = vec![cbindgen::ItemType::Enums];
    public_config.export.include = vec!["TimerMode".into()];
    public_config.export.exclude.clear();

    cbindgen::Builder::new()
        .with_config(public_config)
        .with_src(crate_dir.join("timers.rs"))
        .with_after_include(format!(
            r"
/// This macro expands to the to the numeric value of the major version of SixtyFPS you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 1.
#define SIXTYFPS_VERSION_MAJOR {}
/// This macro expands to the to the numeric value of the minor version of SixtyFPS you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 5.
#define SIXTYFPS_VERSION_MINOR {}
/// This macro expands to the to the numeric value of the patch version of SixtyFPS you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 2.
#define SIXTYFPS_VERSION_PATCH {}
",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ))
        .generate()
        .context("Unable to generate bindings for sixtyfps_generated_public.h")?
        .write_to_file(include_dir.join("sixtyfps_generated_public.h"));

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
    friend inline LayoutInfo operator+(const LayoutInfo &a, const LayoutInfo &b) { return a.merge(b); }".into(),
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
        .with_include("sixtyfps_config.h")
        .with_include("vtable.h")
        .with_include("sixtyfps_string.h")
        .with_include("sixtyfps_sharedvector.h")
        .with_include("sixtyfps_properties.h")
        .with_include("sixtyfps_callbacks.h")
        .with_include("sixtyfps_color.h")
        .with_include("sixtyfps_image.h")
        .with_include("sixtyfps_pathdata.h")
        .with_include("sixtyfps_brush.h")
        .with_include("sixtyfps_generated_public.h")
        .with_after_include(
            r"
namespace sixtyfps {
    namespace private_api { class WindowRc; }
    namespace cbindgen_private {
        using sixtyfps::private_api::WindowRc;
        using namespace vtable;
        struct KeyEvent; struct PointerEvent;
        using private_api::Property;
        using private_api::PathData;
        using private_api::Point;
    }
}",
        )
        .with_trailer(gen_item_declarations(&items))
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_internal.h"));

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
        "NativeComboBox",
        "NativeComboBoxPopup",
        "NativeTabWidget",
        "NativeTab",
    ];

    config.export.include = items.iter().map(|x| x.to_string()).collect();

    config.export.body.insert(
        "NativeStyleMetrics".to_owned(),
        "    inline NativeStyleMetrics(); inline ~NativeStyleMetrics();".to_owned(),
    );

    let mut crate_dir = root_dir.to_owned();
    crate_dir.extend(["sixtyfps_runtime", "rendering_backends", "qt"].iter());

    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_include("sixtyfps_internal.h")
        .with_trailer(gen_item_declarations(&items))
        .generate()
        .context("Unable to generate bindings for sixtyfps_qt_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_qt_internal.h"));

    Ok(())
}

fn gen_backend(
    root_dir: &Path,
    include_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let config = default_config();
    let mut crate_dir = root_dir.to_owned();
    crate_dir.extend(["api", "sixtyfps-cpp"].iter());

    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_header("#include <sixtyfps_internal.h>")
        .generate()
        .context("Unable to generate bindings for sixtyfps_backend_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_backend_internal.h"));

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

    crate_dir.extend(["sixtyfps_runtime", "interpreter"].iter());
    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    // Generate a header file with some public API (enums, etc.)
    let mut public_config = config.clone();
    public_config.namespaces = Some(vec!["sixtyfps".into(), "interpreter".into()]);
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
        .context("Unable to generate bindings for sixtyfps_interpreter_generated_public.h")?
        .write_to_file(include_dir.join("sixtyfps_interpreter_generated_public.h"));

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_include("sixtyfps_internal.h")
        .with_include("sixtyfps_interpreter_generated_public.h")
        .with_after_include(
            r"
            namespace sixtyfps::cbindgen_private {
                struct Value;
                using sixtyfps::interpreter::ValueType;
                using sixtyfps::interpreter::PropertyDescriptor;
                using sixtyfps::interpreter::Diagnostic;
            }",
        )
        .generate()
        .context("Unable to generate bindings for sixtyfps_interpreter_internal.h")?
        .write_to_file(include_dir.join("sixtyfps_interpreter_internal.h"));

    Ok(())
}

/// Generate the headers.
/// `root_dir` is the root directory of the sixtyfps git repo
/// `include_dir` is the output directory
/// Returns the list of all paths that contain dependencies to the generated output. If you call this
/// function from build.rs, feed each entry to stdout prefixed with `cargo:rerun-if-changed=`.
pub fn gen_all(root_dir: &Path, include_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    proc_macro2::fallback::force(); // avoid a abort if panic=abort is set
    std::fs::create_dir_all(include_dir).context("Could not create the include directory")?;
    let mut deps = Vec::new();
    gen_corelib(root_dir, include_dir, &mut deps)?;
    gen_backend_qt(root_dir, include_dir, &mut deps)?;
    gen_backend(root_dir, include_dir, &mut deps)?;
    gen_interpreter(root_dir, include_dir, &mut deps)?;
    Ok(deps)
}
