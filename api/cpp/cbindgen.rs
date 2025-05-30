// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use anyhow::Context;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

// cSpell: ignore compat constexpr corelib deps sharedvector pathdata

fn enums(path: &Path) -> anyhow::Result<()> {
    let mut enums_priv = BufWriter::new(
        std::fs::File::create(path.join("slint_enums_internal.h"))
            .context("Error creating slint_enums_internal.h file")?,
    );
    writeln!(enums_priv, "#pragma once")?;
    writeln!(enums_priv, "// This file is auto-generated from {}", file!())?;
    writeln!(enums_priv, "#include \"slint_enums.h\"")?;
    writeln!(enums_priv, "namespace slint::cbindgen_private {{")?;
    let mut enums_pub = BufWriter::new(
        std::fs::File::create(path.join("slint_enums.h"))
            .context("Error creating slint_enums.h file")?,
    );
    writeln!(enums_pub, "#pragma once")?;
    writeln!(enums_pub, "// This file is auto-generated from {}", file!())?;
    writeln!(enums_pub, "namespace slint {{")?;
    macro_rules! enum_file {
        (PointerEventButton) => {{
            writeln!(enums_priv, "using slint::PointerEventButton;")?;
            &mut enums_pub
        }};
        (AccessibleRole) => {{
            writeln!(enums_priv, "using slint::testing::AccessibleRole;")?;
            &mut enums_pub
        }};
        ($_:ident) => {
            &mut enums_priv
        };
    }
    macro_rules! enum_sub_namespace {
        (AccessibleRole) => {{
            Some("testing")
        }};
        ($_:ident) => {
            None
        };
    }
    macro_rules! print_enums {
         ($( $(#[doc = $enum_doc:literal])* $(#[non_exhaustive])? enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
             $(
                let file = enum_file!($Name);
                let namespace: Option<&'static str> = enum_sub_namespace!($Name);
                if let Some(ns) = namespace {
                    writeln!(file, "namespace {} {{", ns)?;
                }
                $(writeln!(file, "///{}", $enum_doc)?;)*
                writeln!(file, "enum class {} {{", stringify!($Name))?;
                $(
                    $(writeln!(file, "    ///{}", $value_doc)?;)*
                    writeln!(file, "    {},", stringify!($Value).trim_start_matches("r#"))?;
                )*
                writeln!(file, "}};")?;
                if namespace.is_some() {
                    writeln!(file, "}}")?;
                }
             )*
         }
    }
    i_slint_common::for_each_enums!(print_enums);

    writeln!(enums_pub, "}}")?;
    writeln!(enums_priv, "}}")?;

    // Print the key codes constants
    // This is not an enum, but fits well in that file
    writeln!(
        enums_pub,
        r#"
/// This namespace contains constants for each special non-printable key.
///
/// Each constant can be converted to SharedString.
/// The constants are meant to be used with the slint::Window::dispatch_key_press_event() and
/// slint::Window::dispatch_key_release_event() functions.
///
/// Example:
/// ```
/// window.dispatch_key_press_event(slint::platform::key_codes::Tab);
/// ```
namespace slint::platform::key_codes {{
"#
    )?;
    macro_rules! print_key_codes {
        ($($char:literal # $name:ident # $($qt:ident)|* # $($winit:ident $(($_pos:ident))?)|* # $($_xkb:ident)|*;)*) => {
            $(
                writeln!(enums_pub, "/// A constant that represents the key code to be used in slint::Window::dispatch_key_press_event()")?;
                writeln!(enums_pub, r#"constexpr std::u8string_view {} = u8"\u{:04x}";"#, stringify!($name), $char as u32)?;
            )*
        };
    }
    i_slint_common::for_each_special_keys!(print_key_codes);
    writeln!(enums_pub, "}}")?;

    Ok(())
}

fn builtin_structs(path: &Path) -> anyhow::Result<()> {
    let mut structs_pub = BufWriter::new(
        std::fs::File::create(path.join("slint_builtin_structs.h"))
            .context("Error creating slint_builtin_structs.h file")?,
    );
    writeln!(structs_pub, "#pragma once")?;
    writeln!(structs_pub, "// This file is auto-generated from {}", file!())?;
    writeln!(structs_pub, "namespace slint {{")?;

    let mut structs_priv = BufWriter::new(
        std::fs::File::create(path.join("slint_builtin_structs_internal.h"))
            .context("Error creating slint_builtin_structs_internal.h file")?,
    );
    writeln!(structs_priv, "#pragma once")?;
    writeln!(structs_priv, "// This file is auto-generated from {}", file!())?;
    writeln!(structs_priv, "#include \"slint_builtin_structs.h\"")?;
    writeln!(structs_priv, "#include \"slint_enums_internal.h\"")?;
    writeln!(structs_priv, "namespace slint::cbindgen_private {{")?;
    writeln!(structs_priv, "enum class KeyEventType : uint8_t;")?;
    macro_rules! struct_file {
        (StandardListViewItem) => {{
            writeln!(structs_priv, "using slint::StandardListViewItem;")?;
            &mut structs_pub
        }};
        ($_:ident) => {
            &mut structs_priv
        };
    }
    macro_rules! print_structs {
        ($(
            $(#[doc = $struct_doc:literal])*
            $(#[non_exhaustive])?
            $(#[derive(Copy, Eq)])?
            struct $Name:ident {
                @name = $inner_name:literal
                export {
                    $( $(#[doc = $pub_doc:literal])* $pub_field:ident : $pub_type:ty, )*
                }
                private {
                    $( $(#[doc = $pri_doc:literal])* $pri_field:ident : $pri_type:ty, )*
                }
            }
        )*) => {
            $(
                let file = struct_file!($Name);
                $(writeln!(file, "///{}", $struct_doc)?;)*
                writeln!(file, "struct {} {{", stringify!($Name))?;
                $(
                    $(writeln!(file, "    ///{}", $pub_doc)?;)*
                    let pub_type = match stringify!($pub_type) {
                        "i32" => "int32_t",
                        "f32" | "Coord" => "float",
                        other => other,
                    };
                    writeln!(file, "    {} {};", pub_type, stringify!($pub_field))?;
                )*
                $(
                    $(writeln!(file, "    ///{}", $pri_doc)?;)*
                    let pri_type = stringify!($pri_type).replace(' ', "");
                    let pri_type = match pri_type.as_str() {
                        "usize" => "uintptr_t",
                        "crate::animations::Instant" => "uint64_t",
                        // This shouldn't be accessed by the C++ anyway, just need to have the same ABI in a struct
                        "Option<i32>" => "std::pair<int32_t, int32_t>",
                        "Option<core::ops::Range<i32>>" => "std::tuple<int32_t, int32_t, int32_t>",
                        other => other,
                    };
                    writeln!(file, "    {} {};", pri_type, stringify!($pri_field))?;
                )*
                writeln!(file, "    /// \\private")?;
                writeln!(file, "    {}", format!("friend bool operator==(const {name}&, const {name}&) = default;", name = stringify!($Name)))?;
                writeln!(file, "    /// \\private")?;
                writeln!(file, "    {}", format!("friend bool operator!=(const {name}&, const {name}&) = default;", name = stringify!($Name)))?;
                writeln!(file, "}};")?;
            )*
        };
    }
    i_slint_common::for_each_builtin_structs!(print_structs);
    writeln!(structs_priv, "}}")?;
    writeln!(structs_pub, "}}")?;
    Ok(())
}

fn ensure_cargo_rerun_for_crate(
    crate_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    dependencies.push(crate_dir.to_path_buf());
    for entry in std::fs::read_dir(crate_dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|e| e == "rs") {
            dependencies.push(entry.path());
        }
    }
    Ok(())
}

fn default_config() -> cbindgen::Config {
    let mut config = cbindgen::Config::default();
    config.macro_expansion.bitflags = true;
    config.pragma_once = true;
    config.include_version = true;
    config.namespaces = Some(vec!["slint".into(), "cbindgen_private".into()]);
    config.line_length = 100;
    config.tab_width = 4;
    // Note: we might need to switch to C if we need to generate bindings for language that needs C headers
    config.language = cbindgen::Language::Cxx;
    config.cpp_compat = true;
    config.documentation = true;
    config.export = cbindgen::ExportConfig {
        rename: [
            ("Callback".into(), "private_api::CallbackHelper".into()),
            ("VoidArg".into(), "void".into()),
            ("FocusReasonArg".into(), "FocusReason".into()),
            ("KeyEventArg".into(), "KeyEvent".into()),
            ("PointerEventArg".into(), "PointerEvent".into()),
            ("PointerScrollEventArg".into(), "PointerScrollEvent".into()),
            ("PointArg".into(), "slint::LogicalPosition".into()),
            ("FloatArg".into(), "float".into()),
            ("IntArg".into(), "int".into()),
            ("MenuEntryArg".into(), "MenuEntry".into()),
            // Note: these types are not the same, but they are only used in callback return types that are only used in C++ (set and called)
            // therefore it is ok to reinterpret_cast
            ("MenuEntryModel".into(), "std::shared_ptr<slint::Model<MenuEntry>>".into()),
            ("Coord".into(), "float".into()),
        ]
        .iter()
        .cloned()
        .collect(),
        ..Default::default()
    };
    config.defines = [
        ("target_pointer_width = 64".into(), "SLINT_TARGET_64".into()),
        ("target_pointer_width = 32".into(), "SLINT_TARGET_32".into()),
        // Disable any wasm guarded code in C++, too - so that there are no gaps in enums.
        ("target_arch = wasm32".into(), "SLINT_TARGET_WASM".into()),
        ("target_os = android".into(), "__ANDROID__".into()),
        // Disable Rust WGPU specific API feature
        ("feature = unstable-wgpu-24".into(), "SLINT_DISABLED_CODE".into()),
    ]
    .iter()
    .cloned()
    .collect();
    config.structure.associated_constants_in_body = true;
    config.constant.allow_constexpr = true;
    config
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
            .map(|item_name| format!("SLINT_DECL_ITEM({item_name});"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn gen_corelib(
    root_dir: &Path,
    include_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
    enabled_features: EnabledFeatures,
) -> anyhow::Result<()> {
    let mut config = default_config();

    let items = [
        "Empty",
        "Rectangle",
        "BasicBorderRectangle",
        "BorderRectangle",
        "ImageItem",
        "ClippedImage",
        "TouchArea",
        "FocusScope",
        "SwipeGestureHandler",
        "Flickable",
        "SimpleText",
        "ComplexText",
        "Path",
        "WindowItem",
        "TextInput",
        "Clip",
        "BoxShadow",
        "Rotate",
        "Opacity",
        "Layer",
        "ContextMenu",
        "MenuItem",
    ];

    config.export.include = [
        "Clipboard",
        "ItemTreeVTable",
        "Slice",
        "WindowAdapterRcOpaque",
        "PropertyAnimation",
        "AnimationDirection",
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
        "FocusReason",
        "PointerEventKind",
        "PointerEventButton",
        "PointerEvent",
        "PointerScrollEvent",
        "Rect",
        "SortOrder",
        "BitmapFont",
        "PhysicalRegion",
    ]
    .iter()
    .chain(items.iter())
    .map(|x| x.to_string())
    .collect();

    let mut private_exported_types: std::collections::HashSet<String> =
        config.export.include.iter().cloned().collect();

    // included in generated_public.h
    let public_exported_types = [
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
        "Timer",
        "TimerMode",
        "PropertyHandleOpaque",
        "Callback",
        "slint_property_listener_scope_evaluate",
        "slint_property_listener_scope_is_dirty",
        "PropertyTrackerOpaque",
        "CallbackOpaque",
        "WindowAdapterRc",
        "VoidArg",
        "FocusReasonArg",
        "KeyEventArg",
        "PointerEventArg",
        "PointerScrollEventArg",
        "PointArg",
        "Point",
        "MenuEntryModel",
        "MenuEntryArg",
        "slint_color_brighter",
        "slint_color_darker",
        "slint_color_transparentize",
        "slint_color_mix",
        "slint_color_with_alpha",
        "slint_color_to_hsva",
        "slint_color_from_hsva",
        "slint_image_size",
        "slint_image_path",
        "slint_image_load_from_path",
        "slint_image_load_from_embedded_data",
        "slint_image_from_embedded_textures",
        "slint_image_compare_equal",
        "slint_image_set_nine_slice_edges",
        "slint_image_to_rgb8",
        "slint_image_to_rgba8",
        "slint_image_to_rgba8_premultiplied",
        "slint_timer_start",
        "slint_timer_singleshot",
        "slint_timer_destroy",
        "slint_timer_stop",
        "slint_timer_restart",
        "slint_timer_running",
        "Coord",
        "LogicalRect",
        "LogicalPoint",
        "LogicalPosition",
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
        "    const T &operator[](int i) const { return ptr[i]; }
        /// Note: this doesn't initialize Slice properly, but we need to keep the struct as compatible with C
        constexpr Slice() = default;
        /// Rust uses a NonNull, so even empty slices shouldn't use nullptr
        constexpr Slice(const T *ptr, uintptr_t len) : ptr(ptr ? const_cast<T*>(ptr) : reinterpret_cast<T*>(sizeof(T))), len(len) {}
        "
            .to_owned(),
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

    // slint_timer_internal.h:
    let timer_config = {
        let mut tmp = config.clone();
        tmp.export.include = [
            "TimerMode",
            "slint_timer_start",
            "slint_timer_singleshot",
            "slint_timer_destroy",
            "slint_timer_stop",
            "slint_timer_restart",
            "slint_timer_running",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        tmp.export.exclude = config
            .export
            .exclude
            .iter()
            .filter(|exclusion| !tmp.export.include.iter().any(|inclusion| inclusion == *exclusion))
            .cloned()
            .collect();
        tmp
    };
    cbindgen::Builder::new()
        .with_config(timer_config)
        .with_src(crate_dir.join("timers.rs"))
        .generate()
        .context("Unable to generate bindings for slint_timer_internal.h")?
        .write_to_file(include_dir.join("slint_timer_internal.h"));

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
                "slint_image_compare_equal",
                "slint_image_set_nine_slice_edges",
                "slint_image_to_rgb8",
                "slint_image_to_rgba8",
                "slint_image_to_rgba8_premultiplied",
                "SharedPixelBuffer",
                "SharedImageBuffer",
                "StaticTextures",
                "BorrowedOpenGLTextureOrigin"
            ],
            vec!["Color"],
            "slint_image_internal.h",
            "namespace slint::cbindgen_private { struct ParsedSVG{}; struct HTMLImage{}; using namespace vtable; namespace types{ struct NineSliceImage{}; } }",
        ),
        (
            vec!["Color", "slint_color_brighter", "slint_color_darker",
            "slint_color_transparentize",
            "slint_color_mix",
            "slint_color_with_alpha",
            "slint_color_to_hsva",
            "slint_color_from_hsva",],
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
            "slint_windowrc_close_popup",
            "slint_windowrc_set_rendering_notifier",
            "slint_windowrc_request_redraw",
            "slint_windowrc_on_close_requested",
            "slint_windowrc_position",
            "slint_windowrc_set_logical_position",
            "slint_windowrc_set_physical_position",
            "slint_windowrc_size",
            "slint_windowrc_set_logical_size",
            "slint_windowrc_set_physical_size",
            "slint_windowrc_color_scheme",
            "slint_windowrc_supports_native_menu_bar",
            "slint_windowrc_setup_native_menu_bar",
            "slint_windowrc_default_font_size",
            "slint_windowrc_dispatch_pointer_event",
            "slint_windowrc_dispatch_key_event",
            "slint_windowrc_dispatch_event",
            "slint_windowrc_set_fullscreen",
            "slint_windowrc_set_minimized",
            "slint_windowrc_set_maximized",
            "slint_windowrc_is_fullscreen",
            "slint_windowrc_is_minimized",
            "slint_windowrc_is_maximized",
            "slint_windowrc_take_snapshot",
            "slint_new_path_elements",
            "slint_new_path_events",
            "slint_color_brighter",
            "slint_color_darker",
            "slint_color_transparentize",
            "slint_color_mix",
            "slint_color_with_alpha",
            "slint_color_to_hsva",
            "slint_color_from_hsva",
            "slint_image_size",
            "slint_image_path",
            "slint_image_load_from_path",
            "slint_image_load_from_embedded_data",
            "slint_image_set_nine_slice_edges",
            "slint_image_to_rgb8",
            "slint_image_to_rgba8",
            "slint_image_to_rgba8_premultiplied",
            "slint_image_from_embedded_textures",
            "slint_image_compare_equal",
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
            .with_context(|| format!("Unable to generate bindings for {internal_header}"))?
            .write_to_file(include_dir.join(internal_header));
    }

    // Generate a header file with some public API (enums, etc.)
    let mut public_config = config.clone();
    public_config.namespaces = Some(vec!["slint".into()]);
    public_config.export.item_types = vec![cbindgen::ItemType::Enums, cbindgen::ItemType::Structs];
    // Previously included types are now excluded (to avoid duplicates)
    public_config.export.exclude = private_exported_types.into_iter().collect();
    public_config.export.exclude.push("LogicalPosition".into());
    public_config.export.exclude.push("MenuVTable".into());
    public_config.export.include = public_exported_types.into_iter().map(str::to_string).collect();
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
        .with_src(crate_dir.join("graphics.rs"))
        .with_src(crate_dir.join("window.rs"))
        .with_src(crate_dir.join("api.rs"))
        .with_src(crate_dir.join("model.rs"))
        .with_src(crate_dir.join("graphics/image.rs"))
        .with_include("slint_string.h")
        .with_after_include(format!(
            r#"
/// This macro expands to the to the numeric value of the major version of Slint you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 1.
#define SLINT_VERSION_MAJOR {x}
/// This macro expands to the to the numeric value of the minor version of Slint you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 5.
#define SLINT_VERSION_MINOR {y}
/// This macro expands to the to the numeric value of the patch version of Slint you're
/// developing against. For example if you're using version 1.5.2, this macro will expand to 2.
#define SLINT_VERSION_PATCH {z}
/// This macro expands to the string representation of the version of Slint you're developing against.
/// For example if you're using version 1.5.2, this macro will expand to "1.5.2".
#define SLINT_VERSION_STRING "{x}.{y}.{z}"

{features}
"#,
            x = env!("CARGO_PKG_VERSION_MAJOR"),
            y = env!("CARGO_PKG_VERSION_MINOR"),
            z = env!("CARGO_PKG_VERSION_PATCH"),
            features = enabled_features.defines()
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
        "    constexpr EasingCurve(EasingCurve::Tag tag = Tag::Linear, float a = 0, float b = 0, float c = 1, float d = 1) : tag(tag), cubic_bezier{{a,b,c,d}} {}".into()
    );
    config.export.body.insert(
        "LayoutInfo".to_owned(),
        "    inline LayoutInfo merge(const LayoutInfo &other) const;
    friend inline LayoutInfo operator+(const LayoutInfo &a, const LayoutInfo &b) { return a.merge(b); }
    friend bool operator==(const LayoutInfo&, const LayoutInfo&) = default;".into(),
    );
    config.export.body.insert(
        "WindowEvent".to_owned(),
        "/* Some members of the WindowEvent enum have destructors (with SharedString), but thankfully we don't use these so we can have an empty constructor */
    ~WindowEvent() {}"
            .into(),
    );
    config
        .export
        .body
        .insert("Flickable".to_owned(), "    inline Flickable(); inline ~Flickable();".into());
    config.export.pre_body.insert("FlickableDataBox".to_owned(), "struct FlickableData;".into());

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
        .with_include("slint_timer.h")
        .with_include("slint_builtin_structs_internal.h")
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
        struct ItemTreeVTable;
        struct ItemVTable;
        using types::IntRect;
    }
    template<typename ModelData> class Model;
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
        "NativeProgressIndicator",
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
        "NativePalette",
    ];

    config.export.include = items.iter().map(|x| x.to_string()).collect();
    config.export.exclude = vec!["FloatArg".into(), "IntArg".into()];

    config.export.body.insert(
        "NativeStyleMetrics".to_owned(),
        "    inline explicit NativeStyleMetrics(void* = nullptr); inline ~NativeStyleMetrics();"
            .to_owned(),
    );

    config.export.body.insert(
        "NativePalette".to_owned(),
        "    inline explicit NativePalette(void* = nullptr); inline ~NativePalette();".to_owned(),
    );

    let mut crate_dir = root_dir.to_owned();
    crate_dir.extend(["internal", "backends", "qt"].iter());

    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_include("slint_internal.h")
        .with_after_include(
            r"
            namespace slint::cbindgen_private {
                // HACK ALERT: This struct declaration is duplicated in internal/backend/qt/qt_widgets.rs - keep in sync.
                struct SlintTypeErasedWidget
                {
                    virtual ~SlintTypeErasedWidget() = 0;
                    SlintTypeErasedWidget(const SlintTypeErasedWidget&) = delete;
                    SlintTypeErasedWidget& operator=(const SlintTypeErasedWidget&) = delete;

                    virtual void *qwidget() const = 0;
                };
                using SlintTypeErasedWidgetPtr = std::unique_ptr<SlintTypeErasedWidget>;
            }
            ",
        )
        .with_trailer(gen_item_declarations(&items))
        .generate()
        .context("Unable to generate bindings for slint_qt_internal.h")?
        .write_to_file(include_dir.join("slint_qt_internal.h"));

    Ok(())
}

fn gen_testing(
    root_dir: &Path,
    include_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let config = default_config();

    let mut crate_dir = root_dir.to_owned();
    crate_dir.extend(["internal", "backends", "testing"].iter());

    ensure_cargo_rerun_for_crate(&crate_dir, dependencies)?;

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_include("slint_testing_internal.h")
        .generate()
        .context("Unable to generate bindings for slint_testing_internal.h")?
        .write_to_file(include_dir.join("slint_testing_internal.h"));

    Ok(())
}

fn gen_platform(
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
        .with_after_include(
            r"
namespace slint::platform { struct Rgb565Pixel; }
namespace slint::cbindgen_private {
    struct WindowProperties; using slint::platform::Rgb565Pixel;
    using slint::cbindgen_private::types::TexturePixelFormat;
    struct DrawTextureArgs;
    struct DrawRectangleArgs;
}
",
        )
        .generate()
        .context("Unable to generate bindings for slint_platform_internal.h")?
        .write_to_file(include_dir.join("slint_platform_internal.h"));

    Ok(())
}

fn gen_interpreter(
    root_dir: &Path,
    include_dir: &Path,
    dependencies: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut config = default_config();
    config.export.exclude = IntoIterator::into_iter([
        "Value",
        "ValueType",
        "PropertyDescriptor",
        "Diagnostic",
        "PropertyDescriptor",
        "Box",
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
        "Value",
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
                template <typename T> using Box = T*;
            }",
        )
        .generate()
        .context("Unable to generate bindings for slint_interpreter_internal.h")?
        .write_to_file(include_dir.join("slint_interpreter_internal.h"));

    Ok(())
}

macro_rules! declare_features {
    ($($f:ident)+) => {
        #[derive(Clone, Copy)]
        pub struct EnabledFeatures {
            $(pub $f: bool,)*
        }
        impl EnabledFeatures {
            /// Generate the `#define`
            pub fn defines(self) -> String {
                let mut defines = String::new();
                $(
                    if self.$f {
                        defines = format!("{defines}///This macro is defined when Slint was configured with the SLINT_FEATURE_{0} flag enabled\n#define SLINT_FEATURE_{0}\n", stringify!($f).to_ascii_uppercase());
                    };
                )*
                defines
            }

            /// Get the feature from the environment variable set by cargo when building running the slint-cpp's build script
            #[allow(unused)]
            pub fn from_env() -> Self {
                Self {
                    $(
                        $f: std::env::var(format!("CARGO_FEATURE_{}", stringify!($f).to_ascii_uppercase())).is_ok(),
                    )*
                }
            }
        }
    };
}

declare_features! {
    interpreter
    testing
    backend_qt
    backend_winit
    backend_winit_x11
    backend_winit_wayland
    backend_linuxkms
    backend_linuxkms_noseat
    renderer_femtovg
    renderer_skia
    renderer_skia_opengl
    renderer_skia_vulkan
    renderer_software
    gettext
    accessibility
    system_testing
    freestanding
    experimental
}

/// Generate the headers.
/// `root_dir` is the root directory of the slint git repo
/// `include_dir` is the output directory
/// Returns the list of all paths that contain dependencies to the generated output. If you call this
/// function from build.rs, feed each entry to stdout prefixed with `cargo:rerun-if-changed=`.
pub fn gen_all(
    root_dir: &Path,
    include_dir: &Path,
    enabled_features: EnabledFeatures,
) -> anyhow::Result<Vec<PathBuf>> {
    proc_macro2::fallback::force(); // avoid a abort if panic=abort is set
    std::fs::create_dir_all(include_dir).context("Could not create the include directory")?;
    let mut deps = Vec::new();
    enums(include_dir)?;
    builtin_structs(include_dir)?;
    gen_corelib(root_dir, include_dir, &mut deps, enabled_features)?;
    gen_backend_qt(root_dir, include_dir, &mut deps)?;
    gen_platform(root_dir, include_dir, &mut deps)?;
    if enabled_features.testing {
        gen_testing(root_dir, include_dir, &mut deps)?;
    }
    if enabled_features.interpreter {
        gen_interpreter(root_dir, include_dir, &mut deps)?;
    }
    Ok(deps)
}
