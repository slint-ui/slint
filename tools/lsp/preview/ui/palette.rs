// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::{Rc, Weak};

use crate::{
    common,
    preview::{properties, ui},
};

use lsp_types::Url;

use i_slint_compiler::{expression_tree, langtype, object_tree};
use slint::{ComponentHandle, Model, ModelRc, SharedString};

pub fn setup(ui: &ui::PreviewUi) {
    let api = ui.global::<ui::Api>();

    api.on_filter_palettes(filter_palettes);
    api.on_is_css_color(is_css_color);
}

/// Model for the palette.
///
/// Computing the palette can be quite expensive so the palette will be lazily computed on demand
/// and the result will be cached
struct PaletteModel {
    palette: std::cell::OnceCell<Vec<ui::PaletteEntry>>,
    document_cache: Rc<common::DocumentCache>,
    document_uri: Url,
    window_adapter: Weak<dyn slint::platform::WindowAdapter>,
}

impl PaletteModel {
    fn palette(&self) -> &[ui::PaletteEntry] {
        self.palette.get_or_init(|| {
            collect_palette_from_globals(
                &self.document_cache,
                &self.document_uri,
                collect_colors_palette(),
                self.window_adapter.upgrade().as_ref(),
            )
        })
    }
}

impl Model for PaletteModel {
    type Data = ui::PaletteEntry;
    fn row_count(&self) -> usize {
        self.palette().len()
    }

    fn row_data(&self, index: usize) -> Option<ui::PaletteEntry> {
        self.palette().get(index).cloned()
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        // The content of the model won't change. A new model is set when the palette is changed.
        &()
    }
}

pub fn collect_palette(
    document_cache: &Rc<common::DocumentCache>,
    document_uri: &Url,
    window_adapter: &Rc<dyn slint::platform::WindowAdapter>,
) -> ModelRc<ui::PaletteEntry> {
    let model = PaletteModel {
        palette: Default::default(),
        document_cache: document_cache.clone(),
        document_uri: document_uri.clone(),
        window_adapter: Rc::downgrade(window_adapter),
    };
    ModelRc::new(model)
}

pub fn set_palette(ui: &ui::PreviewUi, values: ModelRc<ui::PaletteEntry>) {
    let api = ui.global::<ui::Api>();
    api.set_palettes(values);
}

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
    use langtype::Type;

    match ty {
        Type::Float32
        | Type::Int32
        | Type::String
        | Type::Color
        | Type::Duration
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Angle
        | Type::Percent
        | Type::Bool
        | Type::Enumeration(_)
        | Type::Brush => {
            let mut value = ui::map_value_and_type_to_property_value(ty, &value, full_accessor);
            if !full_accessor.is_empty() {
                value.display_string = SharedString::from(full_accessor);
                value.code = SharedString::from(full_accessor);
            }
            values.push(ui::PaletteEntry { name: SharedString::from(full_accessor), value });
        }
        Type::Struct(st) => {
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

pub fn evaluate_property(
    element: &object_tree::ElementRc,
    property_name: &str,
    default_value: &Option<expression_tree::Expression>,
    ty: &langtype::Type,
    window_adapter: Option<&Rc<dyn slint::platform::WindowAdapter>>,
) -> ui::PropertyValue {
    let value = find_binding_expression(element, property_name)
        .map(|be| be.expression)
        .or(default_value.clone())
        .as_ref()
        .and_then(|element| {
            crate::preview::eval::fully_eval_expression_tree_expression(element, window_adapter)
        });

    ui::map_value_and_type_to_property_value(ty, &value, "")
}

fn handle_type(
    global_name: &smol_str::SmolStr,
    element: &object_tree::ElementRc,
    property_name: &str,
    ty: &langtype::Type,
    values: &mut Vec<ui::PaletteEntry>,
    window_adapter: Option<&Rc<dyn slint::platform::WindowAdapter>>,
) {
    let full_accessor = format!("{global_name}.{property_name}");

    let value = find_binding_expression(element, property_name)
        .map(|be| be.expression)
        .as_ref()
        .and_then(|element| {
            crate::preview::eval::fully_eval_expression_tree_expression(element, window_adapter)
        });

    handle_type_impl(&full_accessor, value, ty, values);
}

fn collect_palette_from_globals(
    document_cache: &common::DocumentCache,
    document_uri: &Url,
    mut values: Vec<ui::PaletteEntry>,
    window_adapter: Option<&Rc<dyn slint::platform::WindowAdapter>>,
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
            handle_type(name, &global, &property.name, &property.ty, &mut values, window_adapter);
        }
    }

    values.sort_by_key(|p| p.name.clone());

    values
}

fn is_css_color(code: slint::SharedString) -> bool {
    let code = code.to_string();
    let code = code.strip_prefix("Colors.").unwrap_or(&code);
    i_slint_compiler::lookup::named_colors().contains_key(code)
}

fn filter_palettes(
    input: slint::ModelRc<ui::PaletteEntry>,
    pattern: slint::SharedString,
) -> slint::ModelRc<ui::PaletteEntry> {
    let pattern = pattern.to_string();
    std::rc::Rc::new(slint::VecModel::from(common::fuzzy_filter_iter(
        &mut input.iter(),
        |p| {
            format!(
                "{} %kind:{:?} %is_brush:{}",
                p.name,
                p.value.kind,
                if [ui::PropertyValueKind::Color, ui::PropertyValueKind::Brush]
                    .contains(&p.value.kind)
                {
                    "yes"
                } else {
                    "no"
                }
            )
        },
        &pattern,
    )))
    .into()
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
        let color = i_slint_core::Color::from_rgb_u8(r, g, b);
        assert_eq!(entry.name, name);
        assert!(entry.value.value_string.is_empty());
        assert_eq!(entry.value.display_string, name);
        assert_eq!(entry.value.gradient_stops.row_count(), 1);
        assert_eq!(entry.value.value_float, 0.0);
        assert_eq!(entry.value.kind, super::ui::PropertyValueKind::Color);
        assert_eq!(entry.value.value_brush, slint::Brush::SolidColor(color));
    }

    #[track_caller]
    fn compare_brush(entry: &PaletteEntry, name: &str, brush: &slint::Brush) {
        eprintln!("\n\n\n{name}:\n{entry:#?}");
        assert_eq!(entry.name, name);
        assert_eq!(entry.value.display_string, name);
        assert_eq!(entry.value.code, name);
        match &brush {
            slint::Brush::SolidColor(_) => {
                assert!(entry.value.value_string.is_empty());
                assert_eq!(entry.value.gradient_stops.row_count(), 1);
            }
            slint::Brush::LinearGradient(lb) => {
                assert!(entry.value.value_string.is_empty());
                assert_eq!(entry.value.value_float, lb.angle());
                assert_eq!(entry.value.gradient_stops.row_count(), 3);
            }
            slint::Brush::RadialGradient(_) => {
                assert!(entry.value.value_string.is_empty());
                assert_eq!(entry.value.value_float, 0.0);
                assert_eq!(entry.value.gradient_stops.row_count(), 3);
            }
            _ => unreachable!(),
        }
        assert_eq!(entry.value.kind, super::ui::PropertyValueKind::Brush);
        assert_eq!(entry.value.value_brush, brush.clone());
    }

    #[test]
    fn test_globals_color_palettes() {
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
        let result = collect_palette_from_globals(&dc, &url, Vec::new(), None);
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
    fn test_globals_brush_palettes() {
        let (dc, url) = compile(
            r#"
global Other {
    out property <brush> brush1: #1ff;
    out property <brush> brush2: @linear-gradient(90deg, #f00, #0f0, #00f);
    out property <brush> brush3: @radial-gradient(circle, #0ff, #f0f, #ff0);
}

global Test {
    in property <int> index;
    out property <brush> brush1: #e0e;
    out property <brush> brush2: Other.brush2;
    out property <brush> brush3 <=> Other.brush3;
    in property <brush> brush4;
    out property <brush> brush5: index == 0 ? Other.brush1 : Other.brush2;
    out property <brush> brush6: Other.brush1;
}

export component Main { }
            "#,
        );
        let result = collect_palette_from_globals(&dc, &url, Vec::new(), None);
        assert_eq!(result.len(), 8);

        let solid_color = slint::Brush::SolidColor(slint::Color::from_rgb_u8(0x11, 0xff, 0xff));
        let linear_gradient =
            slint::Brush::LinearGradient(i_slint_core::graphics::LinearGradientBrush::new(
                90.0,
                vec![
                    i_slint_core::graphics::GradientStop {
                        color: slint::Color::from_rgb_u8(0xff, 0x00, 0x00),
                        position: 0.0,
                    },
                    i_slint_core::graphics::GradientStop {
                        color: slint::Color::from_rgb_u8(0x00, 0xff, 0x00),
                        position: 0.5,
                    },
                    i_slint_core::graphics::GradientStop {
                        color: slint::Color::from_rgb_u8(0x00, 0x00, 0xff),
                        position: 1.0,
                    },
                ]
                .drain(..),
            ));
        let radial_gradient =
            slint::Brush::RadialGradient(i_slint_core::graphics::RadialGradientBrush::new_circle(
                vec![
                    i_slint_core::graphics::GradientStop {
                        color: slint::Color::from_rgb_u8(0x00, 0xff, 0xff),
                        position: 0.0,
                    },
                    i_slint_core::graphics::GradientStop {
                        color: slint::Color::from_rgb_u8(0xff, 0x00, 0xff),
                        position: 0.5,
                    },
                    i_slint_core::graphics::GradientStop {
                        color: slint::Color::from_rgb_u8(0xff, 0xff, 0x00),
                        position: 1.0,
                    },
                ]
                .drain(..),
            ));

        compare_brush(&result[0], "Other.brush1", &solid_color);
        compare_brush(&result[1], "Other.brush2", &linear_gradient);
        compare_brush(&result[2], "Other.brush3", &radial_gradient);

        compare_brush(
            &result[3],
            "Test.brush1",
            &slint::Brush::SolidColor(slint::Color::from_rgb_u8(0xee, 0x00, 0xee)),
        );
        compare_brush(&result[4], "Test.brush2", &linear_gradient);
        compare_brush(&result[5], "Test.brush3", &radial_gradient);
        compare_brush(&result[6], "Test.brush5", &solid_color);
        compare_brush(&result[7], "Test.brush6", &solid_color);
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
        let result = collect_palette_from_globals(&dc, &url, Vec::new(), None);
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
        let result = collect_palette_from_globals(&dc, &url, Vec::new(), None);
        assert_eq!(result.len(), 9);

        compare(&result[0], "Test._0.color1", 0x11, 0xff, 0xff);
        compare(&result[1], "Test._0.color2", 0x22, 0xff, 0xff);
        compare(&result[2], "Test._0.color3", 0x33, 0xff, 0xff);

        compare(&result[3], "Test._1.color1", 0x11, 0x11, 0x11);
        compare(&result[4], "Test._1.color2", 0x22, 0x22, 0x22);
        compare(&result[5], "Test._1.color3", 0x33, 0x33, 0x33);

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
        let result = collect_palette_from_globals(&dc, &url, Vec::new(), None);
        assert_eq!(result.len(), 6);

        compare(&result[0], "Test.palette.dark.color1", 0xee, 0x00, 0x00);
        compare(&result[1], "Test.palette.dark.color2", 0x00, 0xee, 0x00);
        compare(&result[2], "Test.palette.dark.color3", 0x00, 0x00, 0xee);

        compare(&result[3], "Test.palette.light.color1", 0x11, 0x00, 0x00);
        compare(&result[4], "Test.palette.light.color2", 0x00, 0x11, 0x00);
        compare(&result[5], "Test.palette.light.color3", 0x00, 0x00, 0x11);
    }

    #[test]
    fn test_std_widgets_palette() {
        let cases = [
            ("cosmic-dark", 0xC4C4C433u32),
            ("cosmic-light", 0x29292933u32),
            ("fluent-dark", 0xFFFFFF14u32),
            ("fluent-light", 0x00000073u32),
        ];

        for (style, border) in cases {
            let mut config = crate::common::document_cache::CompilerConfiguration::default();
            config.style = Some(style.to_string());
            let mut dc = common::DocumentCache::new(config);
            let (url, _) = crate::language::test::load(
                None,
                &mut dc,
                &std::env::temp_dir().join("xxx/test.slint"),
                r#"
                    import { Palette } from "std-widgets.slint";
                    export component Main { }
                "#,
            );

            let result = collect_palette_from_globals(&dc, &url, Vec::new(), None);
            let r =
                result.iter().find(|entry| entry.name == "Palette.border").expect("Palette.border");
            let color = i_slint_core::Color::from_argb_u8(
                (border & 0xff) as u8,
                ((border >> 24) & 0xff) as u8,
                ((border >> 16) & 0xff) as u8,
                ((border >> 8) & 0xff) as u8,
            );
            assert_eq!(
                r.value.value_brush,
                slint::Brush::SolidColor(color),
                "border color for {style}"
            );
        }
    }

    #[test]
    fn test_filter_palette() {
        let palette = {
            let mut v = super::collect_colors_palette();
            v.sort_by_key(|p| p.name.clone());
            v
        };

        let model: slint::ModelRc<ui::PaletteEntry> =
            Rc::new(slint::VecModel::from(palette.clone())).into();

        assert_eq!(filter_palettes(model.clone(), "'FOO".into()).row_count(), 0);
        assert_eq!(
            filter_palettes(model.clone(), "'%kind:Color".into()).row_count(),
            palette.len()
        );
        assert_eq!(
            filter_palettes(model.clone(), "'%is_brush:yes".into()).row_count(),
            palette.len()
        );
        assert_eq!(filter_palettes(model.clone(), "'%kind:UNKNOWN".into()).row_count(), 0);
        assert_eq!(filter_palettes(model.clone(), "'Colors.aquamarine".into()).row_count(), 1);
        assert_eq!(filter_palettes(model.clone(), "Colors.aquamarine".into()).row_count(), 2);
        assert_eq!(
            filter_palettes(model.clone(), "Colors.aquamarine '%kind:Color".into()).row_count(),
            2
        );
        assert_eq!(filter_palettes(model.clone(), "aquamarine".into()).row_count(), 2);
        assert_eq!(filter_palettes(model.clone(), "^Colors.".into()).row_count(), palette.len());
        assert_eq!(filter_palettes(model.clone(), "!^Colors.".into()).row_count(), 0);
        assert_eq!(filter_palettes(model.clone(), "^Colors.".into()).row_count(), palette.len());

        let reds = filter_palettes(model, "^Colors. red".into());

        assert!(reds.row_count() >= 6);
        assert!(reds.row_count() <= 12);

        assert_eq!(reds.row_data(0).unwrap().name, "Colors.red");
        assert_eq!(reds.row_data(1).unwrap().name, "Colors.darkred");
        assert_eq!(reds.row_data(2).unwrap().name, "Colors.indianred");
        assert_eq!(reds.row_data(3).unwrap().name, "Colors.mediumvioletred");
        assert_eq!(reds.row_data(4).unwrap().name, "Colors.orangered");
        assert_eq!(reds.row_data(5).unwrap().name, "Colors.palevioletred");
    }

    #[test]
    fn test_is_css_color() {
        assert!(!super::is_css_color(slint::SharedString::from("Colors.foobar")));
        assert!(super::is_css_color(slint::SharedString::from("Colors.blue")));
        assert!(super::is_css_color(slint::SharedString::from("blue")));
        assert!(!super::is_css_color(slint::SharedString::from("Styles.foo")));
        assert!(!super::is_css_color(slint::SharedString::from("my_var")));
    }
}
