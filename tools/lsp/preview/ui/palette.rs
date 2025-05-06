// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{
    common,
    preview::{properties, ui},
};

use lsp_types::Url;

use i_slint_compiler::{expression_tree, langtype, object_tree};
use slint::SharedString;

fn collect_colors_palette() -> Vec<ui::PaletteEntry> {
    let colors = i_slint_compiler::lookup::named_colors();
    colors
        .iter()
        .map(|(k, v)| {
            let color_code: slint::SharedString = format!("Colors.{k}").into();
            ui::PaletteEntry {
                name: color_code.clone(),
                value: ui::PropertyValue {
                    accessor_path: color_code.clone(),
                    code: color_code.clone(),
                    display_string: color_code.clone(),
                    kind: ui::PropertyValueKind::Color,
                    value_string: color_code,
                    value_brush: slint::Color::from_argb_encoded(*v).into(),
                    ..Default::default()
                },
            }
        })
        .collect::<Vec<_>>()
}

fn find_binding_expression(
    element: &object_tree::ElementRc,
    property_name: &str,
) -> Option<expression_tree::BindingExpression> {
    let property_name = smol_str::SmolStr::from(property_name);

    let elem = element.borrow();
    let be = elem.bindings.get(&property_name).map(|be| be.borrow().clone())?;
    if matches!(be.expression, expression_tree::Expression::Invalid) {
        for nr in &be.two_way_bindings {
            if let Some(be) = find_binding_expression(&nr.element(), nr.name().as_str()) {
                return Some(be);
            }
        }

        None
    } else {
        Some(be)
    }
}

fn handle_type_impl(
    full_accessor: &str,
    value: Option<slint_interpreter::Value>,
    ty: &langtype::Type,
    values: &mut Vec<ui::PaletteEntry>,
) {
    match ty {
        langtype::Type::Float32
        | langtype::Type::Int32
        | langtype::Type::String
        | langtype::Type::Color
        | langtype::Type::Duration
        | langtype::Type::PhysicalLength
        | langtype::Type::LogicalLength
        | langtype::Type::Rem
        | langtype::Type::Angle
        | langtype::Type::Percent
        | langtype::Type::Bool
        | langtype::Type::Brush => {
            if let Some(value) = ui::map_value_and_type_to_property_value(ty, &value, full_accessor)
            {
                values.push(ui::PaletteEntry { name: SharedString::from(full_accessor), value });
            }
        }
        langtype::Type::Struct(st) => {
            for (name, ty) in st.fields.iter() {
                let sub_value = match &value {
                    Some(slint_interpreter::Value::Struct(s)) => s.get_field(name).cloned(),
                    _ => None,
                };

                handle_type_impl(&format!("{full_accessor}.{name}"), sub_value, ty, values);
            }
        }
        _ => {}
    }
}

fn handle_type(
    global_name: &smol_str::SmolStr,
    element: &object_tree::ElementRc,
    property_name: &str,
    ty: &langtype::Type,
    values: &mut Vec<ui::PaletteEntry>,
) {
    let full_accessor = format!("{global_name}.{property_name}");

    let value = find_binding_expression(element, property_name)
        .map(|be| be.expression)
        .as_ref()
        .and_then(crate::preview::eval::fully_eval_expression_tree_expression);

    handle_type_impl(&full_accessor, value, ty, values);
}

fn collect_palette_from_globals(
    document_cache: &common::DocumentCache,
    document_uri: &Url,
    mut values: Vec<ui::PaletteEntry>,
) -> Vec<ui::PaletteEntry> {
    let tr = document_cache.global_type_registry();
    let tr = document_cache.get_document(document_uri).map(|d| &d.local_registry).unwrap_or(&tr);
    for (name, global) in tr.all_elements().iter().filter_map(|(n, e)| match e {
        langtype::ElementType::Component(c) => c.is_global().then_some((n, c.clone())),
        _ => None,
    }) {
        let global = global.root_element.clone();
        if !matches!(global.borrow().base_type, langtype::ElementType::Global) {
            continue;
        }

        let properties = properties::get_properties(
            &common::ElementRcNode { element: global.clone(), debug_index: 0 },
            properties::LayoutKind::None,
        );

        for property in properties.iter().filter(|p| {
            matches!(
                p.visibility,
                object_tree::PropertyVisibility::Output
                    | object_tree::PropertyVisibility::InOut
                    | object_tree::PropertyVisibility::Public
            )
        }) {
            handle_type(name, &global, &property.name, &property.ty, &mut values);
        }
    }

    values.sort_by_key(|p| p.name.clone());

    values
}

pub fn collect_palettes(
    document_cache: &common::DocumentCache,
    document_uri: &Url,
) -> Vec<ui::PaletteEntry> {
    collect_palette_from_globals(document_cache, document_uri, collect_colors_palette())
}

#[cfg(test)]
mod tests {
    use crate::preview::ui::PaletteEntry;

    use super::*;

    #[test]
    fn test_colors_palette() {
        let colors = collect_colors_palette();
        let input_colors = i_slint_compiler::lookup::named_colors();

        assert_eq!(colors.len(), input_colors.len());
        colors.iter().zip(input_colors).for_each(|(c, (ki, vi))| {
            assert_eq!(c.name, &format!("Colors.{ki}"));
            let slint::Brush::SolidColor(color_value) = c.value.value_brush else {
                panic!("Not a solid color");
            };
            assert_eq!(color_value, slint::Color::from_argb_encoded(*vi));
        });
    }

    fn compile(source: &str) -> (common::DocumentCache, lsp_types::Url) {
        let (dc, url, diag) = crate::test::loaded_document_cache(source.to_string());
        for (u, diag) in diag.iter() {
            if diag.is_empty() {
                continue;
            }
            eprintln!("Diags for {u}");
            for d in diag {
                eprintln!("{d:#?}");
                assert!(!matches!(d.severity, Some(lsp_types::DiagnosticSeverity::ERROR)));
            }
        }
        (dc, url)
    }

    #[track_caller]
    fn compare(entry: &PaletteEntry, name: &str, r: u8, g: u8, b: u8) {
        assert_eq!(entry.name, name);
        assert_eq!(
            entry.value.value_brush,
            slint::Brush::SolidColor(i_slint_core::Color::from_rgb_u8(r, g, b))
        );
    }

    #[test]
    fn test_globals_palettes() {
        let (dc, url) = compile(
            r#"
global Other {
    out property <color> color1: #1ff;
    out property <color> color2: #2ff;
    out property <color> color3: #3ff;
}

global Test {
    in property <int> index;
    out property <color> color1: #f0f;
    out property <color> color2: Other.color2;
    out property <color> color3 <=> Other.color3;
    in property <color> color4;
    out property <color> color5: index == 0 ? Other.color1 : Other.color2;
}

export component Main { }
            "#,
        );
        let result = collect_palette_from_globals(&dc, &url, Vec::new());
        assert_eq!(result.len(), 7);

        compare(&result[0], "Other.color1", 0x11, 0xff, 0xff);
        compare(&result[1], "Other.color2", 0x22, 0xff, 0xff);
        compare(&result[2], "Other.color3", 0x33, 0xff, 0xff);

        compare(&result[3], "Test.color1", 0xff, 0x00, 0xff);
        compare(&result[4], "Test.color2", 0x22, 0xff, 0xff);
        compare(&result[5], "Test.color3", 0x33, 0xff, 0xff);
        compare(&result[6], "Test.color5", 0x11, 0xff, 0xff);
    }

    #[test]
    fn test_globals_palettes_with_struct() {
        let (dc, url) = compile(
            r#"
struct Colors {
    color1: color,
    color2: color,
    color3: color,
}

global Test {
    in property <int> index;

    out property <Colors> palette: {
        color1: #1ff,
        color2: #2ff,
        color3: #3ff,
    };
}

export component Main { }
            "#,
        );
        let result = collect_palette_from_globals(&dc, &url, Vec::new());
        assert_eq!(result.len(), 3);

        compare(&result[0], "Test.palette.color1", 0x11, 0xff, 0xff);
        compare(&result[1], "Test.palette.color2", 0x22, 0xff, 0xff);
        compare(&result[2], "Test.palette.color3", 0x33, 0xff, 0xff);
    }

    #[test]
    fn test_globals_palettes_with_struct_conditional() {
        let (dc, url) = compile(
            r#"
struct Colors {
    color1: color,
    color2: color,
    color3: color,
}

global Test {
    in property <int> index;

    out property <Colors> _0: {
        color1: #1ff,
        color2: #2ff,
        color3: #3ff,
    };

    out property <Colors> _1: {
        color1: #111,
        color2: #222,
        color3: #333,
    };

    out property <Colors> palette: root.index == 0 ? root._0 : root._1;
}

export component Main { }
            "#,
        );
        let result = collect_palette_from_globals(&dc, &url, Vec::new());
        assert_eq!(result.len(), 9);

        compare(&result[0], "Test.-0.color1", 0x11, 0xff, 0xff);
        compare(&result[1], "Test.-0.color2", 0x22, 0xff, 0xff);
        compare(&result[2], "Test.-0.color3", 0x33, 0xff, 0xff);

        compare(&result[3], "Test.-1.color1", 0x11, 0x11, 0x11);
        compare(&result[4], "Test.-1.color2", 0x22, 0x22, 0x22);
        compare(&result[5], "Test.-1.color3", 0x33, 0x33, 0x33);

        compare(&result[6], "Test.palette.color1", 0x11, 0xff, 0xff);
        compare(&result[7], "Test.palette.color2", 0x22, 0xff, 0xff);
        compare(&result[8], "Test.palette.color3", 0x33, 0xff, 0xff);
    }

    #[test]
    fn test_globals_palettes_with_struct_of_structs() {
        let (dc, url) = compile(
            r#"
struct Colors {
    color1: color,
    color2: color,
    color3: color,
}

struct Theme {
    light: Colors,
    dark: Colors,
}

global Test {
    out property <Theme> palette: {
        light: { color1: #100, color2: #010, color3: #001 },
        dark: { color1: #e00, color2: #0e0, color3: #00e },
    };
}

export component Main { }
            "#,
        );
        let result = collect_palette_from_globals(&dc, &url, Vec::new());
        assert_eq!(result.len(), 6);

        compare(&result[0], "Test.palette.dark.color1", 0xee, 0x00, 0x00);
        compare(&result[1], "Test.palette.dark.color2", 0x00, 0xee, 0x00);
        compare(&result[2], "Test.palette.dark.color3", 0x00, 0x00, 0xee);

        compare(&result[3], "Test.palette.light.color1", 0x11, 0x00, 0x00);
        compare(&result[4], "Test.palette.light.color2", 0x00, 0x11, 0x00);
        compare(&result[5], "Test.palette.light.color3", 0x00, 0x00, 0x11);
    }
}
