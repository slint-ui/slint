// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;
use std::{collections::HashMap, iter::once, rc::Rc};

use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, TextRange};
use i_slint_compiler::{expression_tree, langtype, literals};
use itertools::Itertools;
use lsp_types::Url;
use slint::{Model, SharedString, VecModel};
use slint_interpreter::{DiagnosticLevel, PlatformError};
use smol_str::SmolStr;

use crate::common::{self, ComponentInformation};
use crate::preview::{self, preview_data, properties, SelectionNotification};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

slint::include_modules!();

pub type PropertyDeclarations = HashMap<SmolStr, PropertyDeclaration>;

pub fn create_ui(style: String, experimental: bool) -> Result<PreviewUi, PlatformError> {
    let ui = PreviewUi::new()?;

    // styles:
    let known_styles = once(&"native")
        .chain(i_slint_compiler::fileaccess::styles().iter())
        .filter(|s| s != &&"qt" || i_slint_backend_selector::HAS_NATIVE_STYLE)
        .cloned()
        .sorted()
        .collect::<Vec<_>>();
    let style = if known_styles.contains(&style.as_str()) {
        style
    } else {
        known_styles
            .iter()
            .find(|x| **x == "native")
            .or_else(|| known_styles.first())
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    let style_model = Rc::new({
        let model = VecModel::default();
        model.extend(known_styles.iter().map(|s| SharedString::from(*s)));
        assert!(model.row_count() > 1);
        model
    });

    let api = ui.global::<Api>();

    api.set_current_style(style.clone().into());
    api.set_experimental(experimental);
    api.set_known_styles(style_model.into());

    api.on_add_new_component(super::add_new_component);
    api.on_rename_component(super::rename_component);
    api.on_style_changed(super::change_style);
    api.on_show_component(super::show_component);
    api.on_show_document(|file, line, column| {
        use lsp_types::{Position, Range};
        let pos = Position::new((line as u32).saturating_sub(1), (column as u32).saturating_sub(1));
        super::ask_editor_to_show_document(&file, Range::new(pos, pos), false)
    });
    api.on_show_document_offset_range(super::show_document_offset_range);
    api.on_show_preview_for(super::show_preview_for);
    api.on_reload_preview(super::reload_preview);
    api.on_unselect(super::element_selection::unselect_element);
    api.on_reselect(super::element_selection::reselect_element);
    api.on_select_at(super::element_selection::select_element_at);
    api.on_selection_stack_at(super::element_selection::selection_stack_at);
    api.on_filter_sort_selection_stack(super::element_selection::filter_sort_selection_stack);
    api.on_find_selected_selection_stack_frame(|stack| {
        stack.iter().find(|frame| frame.is_selected).unwrap_or_default()
    });
    api.on_select_element(|path, offset, x, y| {
        super::element_selection::select_element_at_source_code_position(
            PathBuf::from(path.to_string()),
            preview::TextSize::from(offset as u32),
            Some(i_slint_core::lengths::LogicalPoint::new(x, y)),
            SelectionNotification::Now,
        );
    });
    api.on_select_behind(super::element_selection::select_element_behind);
    api.on_can_drop(super::can_drop_component);
    api.on_drop(super::drop_component);
    api.on_selected_element_resize(super::resize_selected_element);
    api.on_selected_element_can_move_to(super::can_move_selected_element);
    api.on_selected_element_move(super::move_selected_element);
    api.on_selected_element_delete(super::delete_selected_element);

    api.on_test_code_binding(super::test_code_binding);
    api.on_set_code_binding(super::set_code_binding);
    api.on_set_color_binding(super::set_color_binding);
    api.on_property_declaration_ranges(super::property_declaration_ranges);

    api.on_get_property_value(get_property_value);
    api.on_set_json_preview_data(set_json_preview_data);

    api.on_string_to_code(string_to_code);
    api.on_string_to_color(|s| string_to_color(s.as_ref()).unwrap_or_default());
    api.on_string_is_color(|s| string_to_color(s.as_ref()).is_some());
    api.on_color_to_data(|c| {
        let encoded = c.as_argb_encoded();

        let a = ((encoded & 0xff000000) >> 24) as u8;
        let r = ((encoded & 0x00ff0000) >> 16) as u8;
        let g = ((encoded & 0x0000ff00) >> 8) as u8;
        let b = (encoded & 0x000000ff) as u8;

        ColorData {
            a: a as i32,
            r: r as i32,
            g: g as i32,
            b: b as i32,
            text: format!(
                "#{:08x}",
                ((r as u32) << 24) + ((g as u32) << 16) + ((b as u32) << 8) + (a as u32)
            )
            .into(),
        }
    });
    api.on_rgba_to_color(|r, g, b, a| {
        if (0..256).contains(&r)
            && (0..256).contains(&g)
            && (0..256).contains(&b)
            && (0..256).contains(&a)
        {
            slint::Color::from_argb_u8(a as u8, r as u8, g as u8, b as u8)
        } else {
            slint::Color::default()
        }
    });

    #[cfg(target_vendor = "apple")]
    api.set_control_key_name("command".into());

    #[cfg(target_family = "wasm")]
    if web_sys::window()
        .and_then(|window| window.navigator().platform().ok())
        .map_or(false, |platform| platform.to_ascii_lowercase().contains("mac"))
    {
        api.set_control_key_name("command".into());
    }

    Ok(ui)
}

fn extract_definition_location(ci: &ComponentInformation) -> (SharedString, SharedString) {
    let Some(url) = ci.defined_at.as_ref().map(|da| da.url()) else {
        return (Default::default(), Default::default());
    };

    let path = url.to_file_path().unwrap_or_default();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    (url.to_string().into(), file_name.into())
}

pub fn ui_set_uses_widgets(ui: &PreviewUi, uses_widgets: bool) {
    let api = ui.global::<Api>();
    api.set_uses_widgets(uses_widgets);
}

pub fn set_diagnostics(ui: &PreviewUi, diagnostics: &[slint_interpreter::Diagnostic]) {
    let summary = diagnostics.iter().fold(DiagnosticSummary::NothingDetected, |acc, d| {
        match (acc, d.level()) {
            (_, DiagnosticLevel::Error) => DiagnosticSummary::Errors,
            (DiagnosticSummary::Errors, DiagnosticLevel::Warning) => DiagnosticSummary::Errors,
            (_, DiagnosticLevel::Warning) => DiagnosticSummary::Warnings,
            // DiagnosticLevel is non-exhaustive:
            (acc, _) => acc,
        }
    });

    let api = ui.global::<Api>();
    api.set_diagnostic_summary(summary);
}

pub fn ui_set_known_components(
    ui: &PreviewUi,
    known_components: &[crate::common::ComponentInformation],
    current_component_index: usize,
) {
    let mut builtins_map: HashMap<String, Vec<ComponentItem>> = Default::default();
    let mut std_widgets_map: HashMap<String, Vec<ComponentItem>> = Default::default();
    let mut path_map: HashMap<PathBuf, (SharedString, Vec<ComponentItem>)> = Default::default();
    let mut library_map: HashMap<String, Vec<ComponentItem>> = Default::default();
    let mut longest_path_prefix = PathBuf::new();

    for (idx, ci) in known_components.iter().enumerate() {
        if ci.is_global {
            continue;
        }
        let (url, pretty_location) = extract_definition_location(ci);
        let item = ComponentItem {
            name: ci.name.clone().into(),
            index: idx.try_into().unwrap(),
            defined_at: url.clone(),
            pretty_location,
            is_user_defined: !(ci.is_builtin || ci.is_std_widget),
            is_currently_shown: idx == current_component_index,
            is_exported: ci.is_exported,
        };

        if let Some(position) = &ci.defined_at {
            if let Some(library) = position.url().path().strip_prefix("/@") {
                library_map.entry(format!("@{library}")).or_default().push(item);
            } else {
                let path = i_slint_compiler::pathutils::clean_path(
                    &(position.url().to_file_path().unwrap_or_default()),
                );
                if path != PathBuf::new() {
                    if longest_path_prefix == PathBuf::new() {
                        longest_path_prefix = path.clone();
                    } else {
                        longest_path_prefix =
                            std::iter::zip(longest_path_prefix.components(), path.components())
                                .take_while(|(l, p)| l == p)
                                .map(|(l, _)| l)
                                .collect();
                    }
                }
                path_map.entry(path).or_insert((url, Vec::new())).1.push(item);
            }
        } else if ci.is_builtin {
            builtins_map.entry(ci.category.clone()).or_default().push(item);
        } else {
            std_widgets_map.entry(ci.category.clone()).or_default().push(item);
        }
    }

    fn sort_subset(mut input: HashMap<String, Vec<ComponentItem>>) -> Vec<ComponentListItem> {
        let mut output = input
            .drain()
            .map(|(k, mut v)| {
                v.sort_by_key(|i| i.name.clone());
                let model = Rc::new(VecModel::from(v));
                ComponentListItem {
                    category: k.into(),
                    file_url: SharedString::new(),
                    components: model.into(),
                }
            })
            .collect::<Vec<_>>();
        output.sort_by_key(|k| k.category.clone());
        output
    }

    let builtin_components = sort_subset(builtins_map);
    let std_widgets_components = sort_subset(std_widgets_map);
    let library_components = sort_subset(library_map);
    let mut file_components = path_map
        .drain()
        .map(|(p, (file_url, mut v))| {
            v.sort_by_key(|i| i.name.clone());
            let model = Rc::new(VecModel::from(v));
            let name = if p == longest_path_prefix {
                p.file_name().unwrap_or_default().to_string_lossy().to_string()
            } else {
                p.strip_prefix(&longest_path_prefix).unwrap_or(&p).to_string_lossy().to_string()
            };
            ComponentListItem { category: name.into(), file_url, components: model.into() }
        })
        .collect::<Vec<_>>();
    file_components.sort_by_key(|k| PathBuf::from(k.category.to_string()));

    let mut all_components = Vec::with_capacity(
        builtin_components.len() + library_components.len() + file_components.len(),
    );
    all_components.extend_from_slice(&builtin_components);
    all_components.extend_from_slice(&std_widgets_components);
    all_components.extend_from_slice(&library_components);
    all_components.extend_from_slice(&file_components);

    let result = Rc::new(VecModel::from(all_components));
    let api = ui.global::<Api>();
    api.set_known_components(result.into());
}

fn to_ui_range(r: TextRange) -> Option<Range> {
    Some(Range {
        start: i32::try_from(u32::from(r.start())).ok()?,
        end: i32::try_from(u32::from(r.end())).ok()?,
    })
}

fn map_property_declaration(
    document_cache: &common::DocumentCache,
    declared_at: &Option<properties::DeclarationInformation>,
    defined_at: PropertyDefinition,
) -> Option<PropertyDeclaration> {
    let da = declared_at.as_ref()?;
    let source_version = document_cache.document_version_by_path(&da.path).unwrap_or(-1);
    let pos = TextRange::new(da.start_position, da.start_position);

    Some(PropertyDeclaration {
        defined_at,
        source_path: da.path.to_string_lossy().to_string().into(),
        source_version,
        range: to_ui_range(pos)?,
    })
}

fn extract_tr_data(tr_node: &syntax_nodes::AtTr, value: &mut PropertyValue) {
    let Some(text) = tr_node
        .child_text(SyntaxKind::StringLiteral)
        .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
    else {
        return;
    };

    let context = tr_node
        .TrContext()
        .and_then(|n| n.child_text(SyntaxKind::StringLiteral))
        .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
        .unwrap_or_default();
    let plural = tr_node
        .TrPlural()
        .and_then(|n| n.child_text(SyntaxKind::StringLiteral))
        .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
        .unwrap_or_default();
    let plural_expression = tr_node
        .TrPlural()
        .and_then(|n| n.child_node(SyntaxKind::Expression))
        .and_then(|e| e.child_node(SyntaxKind::QualifiedName))
        .map(|n| i_slint_compiler::object_tree::QualifiedTypeName::from_node(n.into()))
        .map(|qtn| qtn.to_string());

    // We have expressions -> Edit as code
    if tr_node.Expression().next().is_none() && (plural.is_empty() || plural_expression.is_some()) {
        value.kind = PropertyValueKind::String;
        value.is_translatable = true;
        value.tr_context = context.as_str().into();
        value.tr_plural = plural.as_str().into();
        value.tr_plural_expression = plural_expression.unwrap_or_default().into();
        value.value_string = text.as_str().into();
    }
}

fn convert_number_literal(
    node: &syntax_nodes::Expression,
) -> Option<(f64, i_slint_compiler::expression_tree::Unit)> {
    if let Some(unary) = &node.UnaryOpExpression() {
        let factor = match unary.first_token().unwrap().text() {
            "-" => -1.0,
            "+" => 1.0,
            _ => return None,
        };
        convert_number_literal(&unary.Expression()).map(|(v, u)| (factor * v, u))
    } else {
        let literal = node.child_text(SyntaxKind::NumberLiteral)?;
        let expr = literals::parse_number_literal(literal).ok()?;

        match expr {
            i_slint_compiler::expression_tree::Expression::NumberLiteral(value, unit) => {
                Some((value, unit))
            }
            _ => None,
        }
    }
}

fn extract_value_with_unit_impl(
    expression: &Option<syntax_nodes::Expression>,
    def_val: Option<&expression_tree::Expression>,
    code: &str,
    units: &[i_slint_compiler::expression_tree::Unit],
) -> Option<(PropertyValueKind, f32, i32)> {
    if let Some(expression) = expression {
        if let Some((value, unit)) = convert_number_literal(expression) {
            let index = units.iter().position(|u| u == &unit).or_else(|| {
                (units.is_empty() && unit == i_slint_compiler::expression_tree::Unit::None)
                    .then_some(0_usize)
            })?;

            return Some((PropertyValueKind::Float, value as f32, index as i32));
        }
    } else if code.is_empty() {
        if let Some(expression_tree::Expression::NumberLiteral(value, unit)) = def_val {
            let index = units.iter().position(|u| u == unit).unwrap_or(0);
            return Some((PropertyValueKind::Float, *value as f32, index as i32));
        } else {
            // FIXME: if def_vale is Some but not a NumberLiteral, we should not show "0"
            return Some((PropertyValueKind::Float, 0.0, 0));
        }
    }

    None
}

fn convert_simple_string(input: slint::SharedString) -> String {
    format!("\"{}\"", str::escape_debug(input.as_ref()))
}

fn string_to_code(
    input: slint::SharedString,
    is_translatable: bool,
    tr_context: slint::SharedString,
    tr_plural: slint::SharedString,
    tr_plural_expression: slint::SharedString,
) -> slint::SharedString {
    let input = convert_simple_string(input);
    if !is_translatable {
        input
    } else {
        let context = if tr_context.is_empty() {
            String::new()
        } else {
            format!("{} => ", convert_simple_string(tr_context))
        };
        let plural = if tr_plural.is_empty() {
            String::new()
        } else {
            format!(" | {} % {}", convert_simple_string(tr_plural), tr_plural_expression)
        };
        format!("@tr({context}{input}{plural})")
    }
    .into()
}

fn string_to_color(text: &str) -> Option<slint::Color> {
    literals::parse_color_literal(text).map(slint::Color::from_argb_encoded)
}

fn unit_model(units: &[expression_tree::Unit]) -> slint::ModelRc<slint::SharedString> {
    Rc::new(VecModel::from(
        units.iter().map(|u| u.to_string().into()).collect::<Vec<slint::SharedString>>(),
    ))
    .into()
}

fn extract_value_with_unit(
    expression: &Option<syntax_nodes::Expression>,
    def_val: Option<&expression_tree::Expression>,
    units: &[expression_tree::Unit],
    value: &mut PropertyValue,
) {
    let Some((kind, v, index)) =
        extract_value_with_unit_impl(expression, def_val, &value.code, units)
    else {
        return;
    };

    value.kind = kind;
    value.value_float = v;
    value.visual_items = unit_model(units);
    value.value_int = index
}

fn extract_color(
    expression: &syntax_nodes::Expression,
    kind: PropertyValueKind,
    value: &mut PropertyValue,
) -> bool {
    if let Some(text) = expression.child_text(SyntaxKind::ColorLiteral) {
        if let Some(color) = string_to_color(&text) {
            value.kind = kind;
            value.value_brush = slint::Brush::SolidColor(color);
            value.value_string = text.as_str().into();
            return true;
        }
    }
    false
}

fn set_default_brush(
    kind: PropertyValueKind,
    def_val: Option<&expression_tree::Expression>,
    value: &mut PropertyValue,
) {
    use expression_tree::Expression;
    value.kind = kind;
    if let Some(mut def_val) = def_val {
        if let Expression::Cast { from, .. } = def_val {
            def_val = from;
        }
        if let Expression::NumberLiteral(v, _) = def_val {
            value.value_brush = slint::Brush::SolidColor(slint::Color::from_argb_encoded(*v as _));
            return;
        }
    }
    let text = "#00000000";
    let color = literals::parse_color_literal(text).unwrap();
    value.value_string = text.into();
    value.value_brush = slint::Brush::SolidColor(slint::Color::from_argb_encoded(color));
}

fn simplify_value(prop_info: &super::properties::PropertyInformation) -> PropertyValue {
    use i_slint_compiler::expression_tree::Unit;
    use langtype::Type;

    let code_block_or_expression =
        prop_info.defined_at.as_ref().map(|da| da.code_block_or_expression.clone());
    let expression = code_block_or_expression.as_ref().and_then(|cbe| cbe.expression());

    let mut value = PropertyValue {
        code: code_block_or_expression
            .as_ref()
            .map(|cbe| cbe.text().to_string())
            .unwrap_or_default()
            .into(),
        kind: PropertyValueKind::Code,
        ..Default::default()
    };

    let def_val = prop_info.default_value.as_ref();

    match &prop_info.ty {
        Type::Float32 => extract_value_with_unit(&expression, def_val, &[], &mut value),
        Type::Duration => {
            extract_value_with_unit(&expression, def_val, &[Unit::S, Unit::Ms], &mut value)
        }
        Type::PhysicalLength | Type::LogicalLength | Type::Rem => extract_value_with_unit(
            &expression,
            def_val,
            &[Unit::Px, Unit::Cm, Unit::Mm, Unit::In, Unit::Pt, Unit::Phx, Unit::Rem],
            &mut value,
        ),
        Type::Angle => extract_value_with_unit(
            &expression,
            def_val,
            &[Unit::Deg, Unit::Grad, Unit::Turn, Unit::Rad],
            &mut value,
        ),
        Type::Percent => {
            extract_value_with_unit(&expression, def_val, &[Unit::Percent], &mut value)
        }
        Type::Int32 => {
            if let Some(expression) = expression {
                if let Some((v, unit)) = convert_number_literal(&expression) {
                    if unit == i_slint_compiler::expression_tree::Unit::None {
                        value.kind = PropertyValueKind::Integer;
                        value.value_int = v as i32;
                    }
                }
            } else if value.code.is_empty() {
                value.kind = PropertyValueKind::Integer;
            }
        }
        Type::Color => {
            if let Some(expression) = expression {
                extract_color(&expression, PropertyValueKind::Color, &mut value);
                // TODO: Extract `Foo.bar` as Palette `Foo`, entry `bar`.
                // This makes no sense right now, as we have no way to get any
                // information on the palettes.
            } else if value.code.is_empty() {
                set_default_brush(PropertyValueKind::Color, def_val, &mut value);
            }
        }
        Type::Brush => {
            if let Some(expression) = expression {
                extract_color(&expression, PropertyValueKind::Brush, &mut value);
                // TODO: Handle gradients...
            } else if value.code.is_empty() {
                set_default_brush(PropertyValueKind::Brush, def_val, &mut value);
            }
        }
        Type::Bool => {
            if let Some(expression) = expression {
                let qualified_name =
                    expression.QualifiedName().map(|qn| qn.text().to_string()).unwrap_or_default();
                if ["true", "false"].contains(&qualified_name.as_str()) {
                    value.kind = PropertyValueKind::Boolean;
                    value.value_bool = &qualified_name == "true";
                }
            } else if value.code.is_empty() {
                if let Some(expression_tree::Expression::BoolLiteral(v)) = def_val {
                    value.value_bool = *v;
                }
                value.kind = PropertyValueKind::Boolean;
            }
        }
        Type::String => {
            if let Some(expression) = &expression {
                if let Some(text) = expression
                    .child_text(SyntaxKind::StringLiteral)
                    .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
                {
                    value.kind = PropertyValueKind::String;
                    value.value_string = text.as_str().into();
                } else if let Some(tr_node) = &expression.AtTr() {
                    extract_tr_data(tr_node, &mut value)
                }
            } else if value.code.is_empty() {
                if let Some(expression_tree::Expression::StringLiteral(v)) = def_val {
                    value.value_string = v.as_str().into();
                }
                value.kind = PropertyValueKind::String;
            }
        }
        Type::Enumeration(enumeration) => {
            value.kind = PropertyValueKind::Enum;
            value.value_string = enumeration.name.as_str().into();
            value.default_selection = i32::try_from(enumeration.default_value).unwrap_or_default();
            value.visual_items = Rc::new(VecModel::from(
                enumeration
                    .values
                    .iter()
                    .map(|s| SharedString::from(s.as_str()))
                    .collect::<Vec<_>>(),
            ))
            .into();

            if let Some(expression) = expression {
                if let Some(text) = expression
                    .child_node(SyntaxKind::QualifiedName)
                    .map(|n| i_slint_compiler::object_tree::QualifiedTypeName::from_node(n.into()))
                    .map(|n| {
                        let n_str = n.to_string();
                        n_str
                            .strip_prefix(&format!("{}.", enumeration.name))
                            .map(|s| s.to_string())
                            .unwrap_or(n_str)
                    })
                    .map(|s| s.to_string())
                {
                    value.value_int = enumeration
                        .values
                        .iter()
                        .position(|v| v == &text)
                        .and_then(|v| i32::try_from(v).ok())
                        .unwrap_or_default();
                }
            } else if let Some(expression_tree::Expression::EnumerationValue(v)) = def_val {
                value.value_int = v.value as i32
            }
        }
        _ => {}
    }

    value
}

fn map_property_definition(
    defined_at: &Option<properties::DefinitionInformation>,
) -> Option<PropertyDefinition> {
    let da = defined_at.as_ref()?;

    Some(PropertyDefinition {
        definition_range: to_ui_range(da.property_definition_range)?,
        selection_range: to_ui_range(da.selection_range)?,
        expression_range: to_ui_range(da.code_block_or_expression.text_range())?,
        expression_value: da.code_block_or_expression.text().to_string().into(),
    })
}

fn map_properties_to_ui(
    document_cache: &common::DocumentCache,
    properties: Option<properties::QueryPropertyResponse>,
) -> Option<(ElementInformation, HashMap<SmolStr, PropertyDeclaration>, PropertyGroupModel)> {
    use std::cmp::Ordering;

    let properties = &properties?;
    let element = properties.element.as_ref()?;

    let raw_source_uri = Url::parse(&properties.source_uri).ok()?;
    let source_uri: SharedString = raw_source_uri.to_string().into();
    let source_version = properties.source_version;

    let mut property_groups: HashMap<(SmolStr, u32), Vec<PropertyInformation>> = HashMap::new();

    let mut declarations = HashMap::new();

    fn property_group_from(
        groups: &mut HashMap<(SmolStr, u32), Vec<PropertyInformation>>,
        name: SmolStr,
        group_priority: u32,
        property: PropertyInformation,
    ) {
        let entry = groups.entry((name.clone(), group_priority));
        entry.and_modify(|e| e.push(property.clone())).or_insert(vec![property]);
    }

    for pi in &properties.properties {
        let defined_at = map_property_definition(&pi.defined_at).unwrap_or(PropertyDefinition {
            definition_range: Range { start: 0, end: 0 },
            selection_range: Range { start: 0, end: 0 },
            expression_range: Range { start: 0, end: 0 },
            expression_value: String::new().into(),
        });
        let declared_at =
            map_property_declaration(document_cache, &pi.declared_at, defined_at.clone())
                .unwrap_or(PropertyDeclaration {
                    defined_at,
                    source_path: String::new().into(),
                    source_version: -1,
                    range: Range { start: 0, end: 0 },
                });

        declarations.insert(pi.name.clone(), declared_at);

        let value = simplify_value(pi);

        property_group_from(
            &mut property_groups,
            pi.group.clone(),
            pi.group_priority,
            PropertyInformation {
                name: pi.name.as_str().into(),
                type_name: pi.ty.to_string().into(),
                value,
                display_priority: i32::try_from(pi.priority).unwrap(),
            },
        );
    }

    let keys = property_groups
        .keys()
        .sorted_by(|a, b| match a.1.cmp(&b.1) {
            Ordering::Less => Ordering::Less,
            Ordering::Equal => a.0.cmp(&b.0),
            Ordering::Greater => Ordering::Greater,
        })
        .cloned()
        .collect::<Vec<_>>();

    Some((
        ElementInformation {
            id: element.id.as_str().into(),
            type_name: element.type_name.as_str().into(),
            source_uri,
            source_version,
            range: to_ui_range(element.range)?,
        },
        declarations,
        Rc::new(VecModel::from(
            keys.iter()
                .map(|k| PropertyGroup {
                    group_name: k.0.as_str().into(),
                    properties: Rc::new(VecModel::from({
                        let mut v = property_groups.remove(k).unwrap();
                        v.sort_by(|a, b| match a.display_priority.cmp(&b.display_priority) {
                            Ordering::Less => Ordering::Less,
                            Ordering::Equal => a.name.cmp(&b.name),
                            Ordering::Greater => Ordering::Greater,
                        });
                        v
                    }))
                    .into(),
                })
                .collect::<Vec<_>>(),
        ))
        .into(),
    ))
}

fn is_equal_value(c: &PropertyValue, n: &PropertyValue) -> bool {
    c.code == n.code
}

fn is_equal_property(c: &PropertyInformation, n: &PropertyInformation) -> bool {
    c.name == n.name && c.type_name == n.type_name && is_equal_value(&c.value, &n.value)
}

fn is_equal_element(c: &ElementInformation, n: &ElementInformation) -> bool {
    c.id == n.id
        && c.type_name == n.type_name
        && c.source_uri == n.source_uri
        && c.range.start == n.range.start
}

pub type PropertyGroupModel = slint::ModelRc<PropertyGroup>;

fn update_grouped_properties(
    cvg: &VecModel<PropertyInformation>,
    nvg: &VecModel<PropertyInformation>,
) {
    enum Op {
        Insert((usize, usize)),
        Copy((usize, usize)),
        PushBack(usize),
        Remove(usize),
    }

    let mut to_do = Vec::new();

    let mut c_it = cvg.iter();
    let mut n_it = nvg.iter();

    let mut cp = c_it.next();
    let mut np = n_it.next();

    let mut c_index = 0_usize;
    let mut n_index = 0_usize;

    loop {
        match (cp.as_ref(), np.as_ref()) {
            (None, None) => break,
            (Some(_), None) => {
                to_do.push(Op::Remove(c_index));
                cp = c_it.next();
            }
            (Some(c), Some(n)) => match c.name.cmp(&n.name) {
                std::cmp::Ordering::Less => {
                    to_do.push(Op::Remove(c_index));
                    cp = c_it.next();
                }
                std::cmp::Ordering::Equal => {
                    if !is_equal_property(c, n) {
                        to_do.push(Op::Copy((c_index, n_index)));
                    }
                    c_index += 1;
                    n_index += 1;
                    cp = c_it.next();
                    np = n_it.next();
                }
                std::cmp::Ordering::Greater => {
                    to_do.push(Op::Insert((c_index, n_index)));
                    c_index += 1;
                    n_index += 1;
                    np = n_it.next();
                }
            },
            (None, Some(_)) => {
                to_do.push(Op::PushBack(n_index));
                n_index += 1;
                np = n_it.next();
            }
        }
    }

    for op in &to_do {
        match op {
            Op::Insert((c, n)) => {
                cvg.insert(*c, nvg.row_data(*n).unwrap());
            }
            Op::Copy((c, n)) => {
                cvg.set_row_data(*c, nvg.row_data(*n).unwrap());
            }
            Op::PushBack(n) => {
                cvg.push(nvg.row_data(*n).unwrap());
            }
            Op::Remove(c) => {
                cvg.remove(*c);
            }
        }
    }
}

fn get_value<T: Sized + std::convert::TryFrom<slint_interpreter::Value> + std::default::Default>(
    v: &Option<slint_interpreter::Value>,
) -> T {
    v.clone().and_then(|v| v.try_into().ok()).unwrap_or_default()
}

fn get_code(v: &Option<slint_interpreter::Value>) -> SharedString {
    v.as_ref()
        .and_then(|v| slint_interpreter::json::value_to_json(v).ok())
        .and_then(|j| serde_json::to_string_pretty(&j).ok())
        .unwrap_or_default()
        .into()
}

#[derive(Default, Debug)]
struct ValueMapping {
    name_prefix: String,
    is_too_complex: bool,
    is_array: bool,
    header: Vec<String>,
    current_values: Vec<PropertyValue>,
    array_values: Vec<Vec<PropertyValue>>,
}

fn map_value_and_type(
    ty: &langtype::Type,
    value: &Option<slint_interpreter::Value>,
    mapping: &mut ValueMapping,
) {
    use i_slint_compiler::expression_tree::Unit;
    use langtype::Type;

    match ty {
        Type::Float32 => {
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                value_string: get_value::<f32>(value).to_string().into(),
                code: get_code(value),
                ..Default::default()
            });
        }

        Type::Int32 => {
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Integer,
                value_int: get_value::<i32>(value),
                value_string: get_value::<i32>(value).to_string().into(),
                code: get_code(value),
                ..Default::default()
            });
        }
        Type::Duration => {
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                value_string: format!("{}{}", get_value::<f32>(value), Unit::Ms).into(),
                visual_items: unit_model(&[Unit::S, Unit::Ms]),
                value_int: 0,
                code: get_code(value),
                default_selection: 1,
                ..Default::default()
            });
        }
        Type::PhysicalLength | Type::LogicalLength | Type::Rem => {
            // TODO: Is this correct? That unit is the Value anyway?!
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                value_string: format!("{}{}", get_value::<f32>(value), Unit::Px).into(),
                visual_items: unit_model(&[
                    Unit::Px,
                    Unit::Cm,
                    Unit::Mm,
                    Unit::In,
                    Unit::Pt,
                    Unit::Phx,
                    Unit::Rem,
                ]),
                value_int: 0,
                code: get_code(value),
                default_selection: 0,
                ..Default::default()
            });
        }
        Type::Angle => {
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                value_string: format!("{}{}", get_value::<f32>(value), Unit::Deg).into(),
                visual_items: unit_model(&[Unit::Deg, Unit::Grad, Unit::Turn, Unit::Rad]),
                value_int: 0,
                code: get_code(value),
                default_selection: 0,
                ..Default::default()
            });
        }
        Type::Percent => {
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                value_string: format!("{}{}", get_value::<f32>(value), Unit::Percent).into(),
                visual_items: unit_model(&[Unit::Percent]),
                value_int: 0,
                code: get_code(value),
                default_selection: 0,
                ..Default::default()
            });
        }
        Type::String => {
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::String,
                value_string: get_value::<slint::SharedString>(value),
                code: get_code(value),
                ..Default::default()
            });
        }
        Type::Color => {
            let color = get_value::<slint::Color>(value);
            let color_string = {
                let a = color.alpha();
                let r = color.red();
                let g = color.green();
                let b = color.blue();

                format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
            };
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Color,
                value_brush: slint::Brush::SolidColor(color),
                value_string: color_string.into(),
                code: get_code(value),
                ..Default::default()
            });
        }
        Type::Bool => {
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Boolean,
                value_bool: get_value::<bool>(value),
                value_string: if get_value::<bool>(value) { "true".into() } else { "false".into() },
                code: get_code(value),
                ..Default::default()
            });
        }
        Type::Array(_)
        | Type::Image
        | Type::Model
        | Type::PathData
        | Type::Easing
        | Type::Brush
        | Type::Struct(_)
        | Type::Enumeration(_)
        | Type::UnitProduct(_) => {
            mapping.is_too_complex = true;
        }
        _ => {
            mapping.header.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                kind: PropertyValueKind::Code,
                value_string: "???".into(),
                code: get_code(value),
                ..Default::default()
            });
        }
    }

    if mapping.array_values.is_empty() {
        mapping.array_values = vec![std::mem::take(&mut mapping.current_values)];
    }

    // Back out when this got too complex and just put the JSON value in:
    if mapping.is_too_complex {
        mapping.is_array = false;
        mapping.header = vec![String::new()];
        mapping.array_values = vec![vec![PropertyValue {
            kind: PropertyValueKind::Code,
            code: get_code(value),
            ..Default::default()
        }]]
    }
}

fn map_preview_data_to_property_value(
    preview_data: &preview_data::PreviewData,
) -> Option<PropertyValue> {
    let mut mapping = ValueMapping::default();
    map_value_and_type(&preview_data.ty, &preview_data.value, &mut mapping);
    mapping.array_values.first().and_then(|av| av.first()).cloned()
}

fn map_preview_data_property(preview_data: &preview_data::PreviewData) -> Option<PreviewData> {
    if !preview_data.is_property() {
        return None;
    };

    let has_getter = preview_data.has_getter();
    let has_setter = preview_data.has_setter();

    let mut mapping = ValueMapping::default();
    map_value_and_type(&preview_data.ty, &preview_data.value, &mut mapping);

    Some(PreviewData {
        name: preview_data.name.clone().into(),
        has_getter,
        has_setter,
        kind: if mapping.is_too_complex { PreviewDataKind::Json } else { PreviewDataKind::Value },
    })
}

pub fn ui_set_preview_data(
    ui: &PreviewUi,
    preview_data: HashMap<preview_data::PropertyContainer, Vec<preview_data::PreviewData>>,
) {
    fn fill_container(
        container_name: String,
        container_id: String,
        properties: &[preview_data::PreviewData],
    ) -> Option<PropertyContainer> {
        let properties =
            properties.iter().filter_map(map_preview_data_property).collect::<Vec<_>>();

        (!properties.is_empty()).then(|| PropertyContainer {
            container_name: container_name.into(),
            container_id: container_id.into(),
            properties: Rc::new(slint::VecModel::from(properties)).into(),
        })
    }

    let mut result: Vec<PropertyContainer> = vec![];

    if let Some(main) = preview_data.get(&preview_data::PropertyContainer::Main) {
        if let Some(c) = fill_container("<MAIN>".to_string(), String::new(), main) {
            result.push(c)
        }
    }

    for component_key in
        preview_data.keys().filter(|k| **k != preview_data::PropertyContainer::Main)
    {
        if let Some(component) = preview_data.get(component_key) {
            let component_key = component_key.to_string();
            if let Some(c) = fill_container(component_key.clone(), component_key, component) {
                result.push(c);
            }
        }
    }

    let api = ui.global::<Api>();

    let old_model = api.get_preview_data();

    fn update_model(
        old_model: &slint::ModelRc<PropertyContainer>,
        new_model: &[PropertyContainer],
    ) -> bool {
        if old_model.row_count() != new_model.len() {
            return true;
        }

        for (oc, nc) in old_model.iter().zip(new_model.iter()) {
            if oc.container_name != nc.container_name
                || oc.container_id != nc.container_id
                || oc.properties.row_count() != nc.properties.row_count()
                || oc.properties.iter().zip(nc.properties.iter()).any(|(o, n)| o != n)
            {
                return true;
            }
        }

        false
    }

    if update_model(&old_model, &result) {
        api.set_preview_data(Rc::new(VecModel::from(result)).into());
    }
}

fn to_property_container(container: slint::SharedString) -> preview_data::PropertyContainer {
    if container.is_empty() {
        preview_data::PropertyContainer::Main
    } else {
        preview_data::PropertyContainer::Global(container.to_string())
    }
}

fn get_property_value(container: SharedString, property_name: SharedString) -> PropertyValue {
    preview::component_instance()
        .and_then(|component_instance| {
            preview_data::get_preview_data(
                &component_instance,
                to_property_container(container),
                property_name.to_string(),
            )
        })
        .and_then(|pd| map_preview_data_to_property_value(&pd))
        .unwrap_or_else(Default::default)
}

fn set_json_preview_data(
    container: SharedString,
    property_name: SharedString,
    json_string: SharedString,
) -> bool {
    let property_name = (!property_name.is_empty()).then_some(property_name.to_string());

    let Ok(json) = serde_json::from_str::<serde_json::Value>(json_string.as_ref()) else {
        return false;
    };

    if property_name.is_none() && !json.is_object() {
        return false;
    }

    preview::component_instance()
        .and_then(|component_instance| {
            preview_data::set_json_preview_data(
                &component_instance,
                to_property_container(container),
                property_name,
                json,
            )
            .ok()
        })
        .is_some()
}

fn update_properties(
    current_model: PropertyGroupModel,
    next_model: PropertyGroupModel,
) -> PropertyGroupModel {
    debug_assert_eq!(current_model.row_count(), next_model.row_count());

    for (c, n) in std::iter::zip(current_model.iter(), next_model.iter()) {
        debug_assert_eq!(c.group_name, n.group_name);

        let cvg = c.properties.as_any().downcast_ref::<VecModel<PropertyInformation>>().unwrap();
        let nvg = n.properties.as_any().downcast_ref::<VecModel<PropertyInformation>>().unwrap();

        update_grouped_properties(cvg, nvg);
    }

    current_model
}

pub fn ui_set_properties(
    ui: &PreviewUi,
    document_cache: &common::DocumentCache,
    properties: Option<properties::QueryPropertyResponse>,
) -> PropertyDeclarations {
    let (next_element, declarations, next_model) = map_properties_to_ui(document_cache, properties)
        .unwrap_or((
            ElementInformation {
                id: "".into(),
                type_name: "".into(),
                source_uri: "".into(),
                source_version: 0,
                range: Range { start: 0, end: 0 },
            },
            HashMap::new(),
            Rc::new(VecModel::from(Vec::<PropertyGroup>::new())).into(),
        ));

    let api = ui.global::<Api>();
    let current_model = api.get_properties();

    let element = api.get_current_element();
    if !is_equal_element(&element, &next_element) {
        api.set_properties(next_model);
    } else if current_model.row_count() > 0 {
        update_properties(current_model, next_model);
    } else {
        api.set_properties(next_model);
    }

    api.set_current_element(next_element);

    declarations
}

#[cfg(test)]
mod tests {
    use crate::{language::test::loaded_document_cache, preview::preview_data};

    use crate::common;
    use crate::preview::properties;

    use i_slint_core::model::Model;

    use super::{PropertyInformation, PropertyValue, PropertyValueKind};

    fn properties_at_position(
        source: &str,
        line: u32,
        character: u32,
    ) -> Option<(
        common::ElementRcNode,
        Vec<properties::PropertyInformation>,
        common::DocumentCache,
        lsp_types::Url,
    )> {
        let (dc, url, _) = loaded_document_cache(source.to_string());
        if let Some((e, p)) =
            properties::tests::properties_at_position_in_cache(line, character, &dc, &url)
        {
            Some((e, p, dc, url))
        } else {
            None
        }
    }

    fn property_conversion_test(contents: &str, property_line: u32) -> PropertyValue {
        let (_, pi, _, _) = properties_at_position(contents, property_line, 30).unwrap();
        let test1 = pi.iter().find(|pi| pi.name == "test1").unwrap();
        super::simplify_value(test1)
    }

    #[test]
    fn test_property_bool() {
        let result =
            property_conversion_test(r#"export component Test { in property <bool> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert!(!result.value_bool);
        assert!(result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: true; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert!(result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: false; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: 1.1.round() == 1.1.floor(); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_property_string() {
        let result =
            property_conversion_test(r#"export component Test { in property <string> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::String);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: ""; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::String);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: "string"; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::String);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: "" + "test"); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_property_tr_string() {
        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("Context" => "test"); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::String);
        assert_eq!(result.value_string, "test");
        assert!(result.is_translatable);
        assert_eq!(result.tr_context, "Context");
        assert_eq!(result.tr_plural, "");
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test {
    property <int> test: 42;
    in property <string> test1: @tr("{n} string" | "{n} strings" % test);
}"#,
            2,
        );
        assert_eq!(result.kind, PropertyValueKind::String);
        assert!(result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "{n} strings");
        assert_eq!(result.tr_plural_expression, "test");
        assert_eq!(result.value_string, "{n} string");
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test {
    property <int> test: 42;
    in property <string> test1: @tr("{n} string" | "{n} strings" % self.test);
}"#,
            2,
        );
        assert_eq!(result.kind, PropertyValueKind::String);
        assert!(result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "{n} strings");
        assert_eq!(result.tr_plural_expression, "self.test");
        assert_eq!(result.value_string, "{n} string");
        assert!(!result.code.is_empty());

        // `15` is not a qualified name
        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("{n} string" | "{n} strings" % 15); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "");
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("" + "test"); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "");
        assert!(!result.code.is_empty());
        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("width {}", self.width()); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "");
        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_property_enum() {
        let result = property_conversion_test(
            r#"export component Test { in property <ImageFit> test1: ImageFit.preserve; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Enum);
        assert_eq!(result.value_string, "ImageFit");
        assert_eq!(result.value_int, 3);
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 4);

        let result = property_conversion_test(
            r#"export component Test { in property <ImageFit> test1: ImageFit   .    /* abc */ preserve; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Enum);
        assert_eq!(result.value_string, "ImageFit");
        assert_eq!(result.value_int, 3);
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 4);

        let result = property_conversion_test(
            r#"export component Test { in property <ImageFit> test1: /* abc */ preserve; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Enum);
        assert_eq!(result.value_string, "ImageFit");
        assert_eq!(result.value_int, 3);
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 4);

        let result = property_conversion_test(
            r#"enum Foobar { foo, bar }
export component Test { in property <Foobar> test1: Foobar.bar; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Enum);
        assert_eq!(result.value_string, "Foobar");
        assert_eq!(result.value_int, 1);
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 2);
        assert_eq!(result.visual_items.row_data(0), Some(slint::SharedString::from("foo")));
        assert_eq!(result.visual_items.row_data(1), Some(slint::SharedString::from("bar")));

        let result = property_conversion_test(
            r#"enum Foobar { foo, bar }
export component Test { in property <Foobar> test1; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Enum);
        assert_eq!(result.value_string, "Foobar");
        assert_eq!(result.value_int, 0); // default
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 2);
        assert_eq!(result.visual_items.row_data(0), Some(slint::SharedString::from("foo")));
        assert_eq!(result.visual_items.row_data(1), Some(slint::SharedString::from("bar")));
    }

    #[test]
    fn test_property_float() {
        let result =
            property_conversion_test(r#"export component Test { in property <float> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::Float);
        assert_eq!(result.value_float, 0.0);

        let result = property_conversion_test(
            r#"export component Test { in property <float> test1: 42.0; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Float);
        assert_eq!(result.value_float, 42.0);

        let result = property_conversion_test(
            r#"export component Test { in property <float> test1: +42.0; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Float);
        assert_eq!(result.value_float, 42.0);

        let result = property_conversion_test(
            r#"export component Test { in property <float> test1: -42.0; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Float);
        assert_eq!(result.value_float, -42.0);

        let result = property_conversion_test(
            r#"export component Test { in property <float> test1: 42.0 * 23.0; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert_eq!(result.value_float, 0.0);
    }

    #[test]
    fn test_property_integer() {
        let result =
            property_conversion_test(r#"export component Test { in property <int> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::Integer);
        assert_eq!(result.value_int, 0);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: 42; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Integer);
        assert_eq!(result.value_int, 42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: +42; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Integer);
        assert_eq!(result.value_int, 42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: -42; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Integer);
        assert_eq!(result.value_int, -42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: 42 * 23; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert_eq!(result.value_int, 0);
    }

    #[test]
    fn test_property_color() {
        let result =
            property_conversion_test(r#"export component Test { in property <color> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_brush.color().red(), 0);
        assert_eq!(result.value_brush.color().green(), 0);
        assert_eq!(result.value_brush.color().blue(), 0);
        assert_eq!(result.value_brush.color().alpha(), 0);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: #10203040; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_brush.color().red(), 0x10);
        assert_eq!(result.value_brush.color().green(), 0x20);
        assert_eq!(result.value_brush.color().blue(), 0x30);
        assert_eq!(result.value_brush.color().alpha(), 0x40);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: #10203040.darker(0.5); }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: Colors.red; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
    }

    #[test]
    fn test_property_brush() {
        let result =
            property_conversion_test(r#"export component Test { in property <brush> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::Brush);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_brush.color().red(), 0);
        assert_eq!(result.value_brush.color().green(), 0);
        assert_eq!(result.value_brush.color().blue(), 0);
        assert_eq!(result.value_brush.color().alpha(), 0);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: #10203040; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Brush);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_brush.color().red(), 0x10);
        assert_eq!(result.value_brush.color().green(), 0x20);
        assert_eq!(result.value_brush.color().blue(), 0x30);
        assert_eq!(result.value_brush.color().alpha(), 0x40);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: #10203040.darker(0.5); }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: Colors.red; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%); }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @radial-gradient(circle, #f00 0%, #0f0 50%, #00f 100%)
            @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%); }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
    }

    #[test]
    fn test_property_units() {
        let result =
            property_conversion_test(r#"export component Test { in property <length> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::Float);
        assert_eq!(result.default_selection, 0);
        assert_eq!(result.value_int, 0);
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("px".into()));
        let length_row_count = result.visual_items.row_count();
        assert!(length_row_count > 2);

        let result = property_conversion_test(
            r#"export component Test { in property <duration> test1: 25s; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Float);
        assert_eq!(result.value_float, 25.0);
        assert_eq!(result.default_selection, 0);
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("s".into()));
        assert_eq!(result.visual_items.row_count(), 2); // ms, s

        let result = property_conversion_test(
            r#"export component Test { in property <physical-length> test1: 1.5phx; }"#,
            1,
        );
        assert_eq!(result.kind, PropertyValueKind::Float);
        assert_eq!(result.value_float, 1.5);
        assert_eq!(result.default_selection, 0);
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("phx".into()));
        assert!(result.visual_items.row_count() > 1); // More than just physical length

        let result = property_conversion_test(
            r#"export component Test { in property <angle> test1: 1.5turns + 1.3deg; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
    }

    #[test]
    fn test_property_with_default_values() {
        let source = r#"
import { Button } from "std-widgets.slint";
component MyButton inherits Button {
    text: "Ok";
    in property <color> color: red;
    in property alias <=> self.xxx;
    property <length> xxx: 45cm;
}
export component X {
    MyButton {
        /*CURSOR*/
    }
}
        "#;

        let (dc, url, _diag) = loaded_document_cache(source.to_string());
        let element = dc
            .element_at_offset(&url, (source.find("/*CURSOR*/").expect("cursor") as u32).into())
            .unwrap();
        let pi = super::properties::get_properties(&element, super::properties::LayoutKind::None);

        let prop = pi.iter().find(|pi| pi.name == "visible").unwrap();
        let result = super::simplify_value(prop);
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert!(result.value_bool);

        let prop = pi.iter().find(|pi| pi.name == "enabled").unwrap();
        let result = super::simplify_value(prop);
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert!(result.value_bool);

        let prop = pi.iter().find(|pi| pi.name == "text").unwrap();
        let result = super::simplify_value(prop);
        assert_eq!(result.kind, PropertyValueKind::String);
        assert_eq!(result.value_string, "Ok");

        let prop = pi.iter().find(|pi| pi.name == "alias").unwrap();
        let result = super::simplify_value(prop);
        assert_eq!(result.kind, PropertyValueKind::Float);
        assert_eq!(result.value_float, 45.);
        assert_eq!(result.visual_items.row_data(result.value_int as usize).unwrap(), "cm");

        let prop = pi.iter().find(|pi| pi.name == "color").unwrap();
        let result = super::simplify_value(prop);
        assert_eq!(result.kind, PropertyValueKind::Color);
        assert_eq!(
            result.value_brush,
            slint::Brush::SolidColor(slint::Color::from_rgb_u8(255, 0, 0))
        );
    }

    #[test]
    fn test_property_with_default_values_loop() {
        let source = r#"
component Abc {
        // This should be an error, not a infinite loop/hang
        in property <length> some_loop <=> r.border-width;
        r:= Rectangle {
            property <length> some_loop <=> root.some_loop;
            border-width <=> some_loop;
        }
}
export component X {
    Abc {
        /*CURSOR*/
    }
}
        "#;

        let (dc, url, _diag) = loaded_document_cache(source.to_string());

        let element = dc
            .element_at_offset(&url, (source.find("/*CURSOR*/").expect("cursor") as u32).into())
            .unwrap();
        let pi = super::properties::get_properties(&element, super::properties::LayoutKind::None);

        let prop = pi.iter().find(|pi| pi.name == "visible").unwrap();
        let result = super::simplify_value(prop);
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert!(result.value_bool);
    }

    fn create_test_property(name: &str, value: &str) -> PropertyInformation {
        PropertyInformation {
            name: name.into(),
            display_priority: 1000,
            type_name: "Sometype".into(),
            value: PropertyValue {
                kind: PropertyValueKind::String,
                value_string: value.into(),
                code: value.into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_property_date_update() {
        let current = slint::VecModel::from(vec![
            create_test_property("aaa", "AAA"),
            create_test_property("bbb", "BBB"),
            create_test_property("ccc", "CCC"),
            create_test_property("ddd", "DDD"),
            create_test_property("eee", "EEE"),
        ]);
        let next = slint::VecModel::from(vec![
            create_test_property("aaa", "AAA"),
            create_test_property("aab", "AAB"),
            create_test_property("abb", "ABB"),
            create_test_property("bbb", "BBBX"),
            create_test_property("ddd", "DDD"),
        ]);

        super::update_grouped_properties(&current, &next);

        let mut it = current.iter();

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "aaa");
        assert_eq!(t.value.code.as_str(), "AAA");

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "aab");
        assert_eq!(t.value.code.as_str(), "AAB");

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "abb");
        assert_eq!(t.value.code.as_str(), "ABB");

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "bbb");
        assert_eq!(t.value.code.as_str(), "BBBX");

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "ddd");
        assert_eq!(t.value.code.as_str(), "DDD");

        assert!(it.next().is_none());
    }

    fn generate_preview_data(
        visibility: &str,
        type_def: &str,
        type_name: &str,
        code: &str,
    ) -> crate::preview::preview_data::PreviewData {
        let component_instance = crate::preview::test::interpret_test(
            "fluent",
            &format!(
                r#"
{type_def}
export component Tester {{
    {visibility} property <{type_name}> test: {code};
}}
            "#
            ),
        );
        let preview_data =
            preview_data::query_preview_data_properties_and_callbacks(&component_instance);
        return preview_data.get(&preview_data::PropertyContainer::Main).unwrap()[0].clone();
    }

    fn compare_pv(r: &super::PropertyValue, e: &PropertyValue) {
        eprintln!("Received: {r:?}");
        eprintln!("Expected: {e:?}");

        assert_eq!(r.value_bool, e.value_bool);
        assert_eq!(r.is_translatable, e.is_translatable);
        assert_eq!(r.value_brush, e.value_brush);
        assert_eq!(r.value_float, e.value_float);
        assert_eq!(r.value_int, e.value_int);
        assert_eq!(r.default_selection, e.default_selection);
        assert_eq!(r.value_string, e.value_string);
        assert_eq!(r.tr_context, e.tr_context);
        assert_eq!(r.tr_plural, e.tr_plural);
        assert_eq!(r.tr_plural_expression, e.tr_plural_expression);
        assert_eq!(r.code, e.code);

        assert_eq!(r.visual_items.row_count(), e.visual_items.row_count());
        for (r, e) in r.visual_items.iter().zip(e.visual_items.iter()) {
            assert_eq!(r, e);
        }
    }

    fn validate_rp(
        visibility: &str,
        type_def: &str,
        type_name: &str,
        code: &str,
        expected_data: super::PreviewData,
        expected_value: super::PropertyValue,
    ) {
        let raw_data = generate_preview_data(visibility, type_def, type_name, code);

        let rp = super::map_preview_data_property(&raw_data).unwrap();

        eprintln!("*** Validating PreviewData: Received: {rp:?}");
        eprintln!("*** Validating PreviewData: Expected: {expected_data:?}");

        assert_eq!(rp.name, expected_data.name);
        assert_eq!(rp.has_getter, expected_data.has_getter);
        assert_eq!(rp.has_setter, expected_data.has_setter);
        assert_eq!(rp.kind, expected_data.kind);

        let pv = super::map_preview_data_to_property_value(&raw_data).unwrap();
        compare_pv(&pv, &expected_value);
    }

    #[test]
    fn test_map_preview_data_string() {
        validate_rp(
            "in",
            "",
            "string",
            "\"Test\"",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "\"Test\"".into(),
                kind: super::PropertyValueKind::String,
                value_string: "Test".into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_length() {
        validate_rp(
            "in",
            "",
            "length",
            "100px",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "100".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 100.0,
                value_string: "100px".into(),
                visual_items: std::rc::Rc::new(slint::VecModel::from(vec![
                    "px".into(),
                    "cm".into(),
                    "mm".into(),
                    "in".into(),
                    "pt".into(),
                    "phx".into(),
                    "rem".into(),
                ]))
                .into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_duration() {
        validate_rp(
            "in",
            "",
            "duration",
            "100s",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "100000".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 100000.0,
                value_string: "100000ms".into(),
                visual_items: std::rc::Rc::new(slint::VecModel::from(vec![
                    "s".into(),
                    "ms".into(),
                ]))
                .into(),
                default_selection: 1,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_angle() {
        validate_rp(
            "in",
            "",
            "angle",
            "100turn",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "36000".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 36000.0,
                value_string: "36000deg".into(),
                visual_items: std::rc::Rc::new(slint::VecModel::from(vec![
                    "deg".into(),
                    "grad".into(),
                    "turn".into(),
                    "rad".into(),
                ]))
                .into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_percent() {
        validate_rp(
            "in",
            "",
            "percent",
            "10%",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "10".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 10.0,
                value_string: "10%".into(),
                visual_items: std::rc::Rc::new(slint::VecModel::from(vec!["%".into()])).into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_color() {
        validate_rp(
            "in",
            "",
            "color",
            "#aabbcc",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "\"#aabbccff\"".into(),
                kind: super::PropertyValueKind::Color,
                value_string: "#aabbccff".into(),
                value_brush: slint::Brush::SolidColor(slint::Color::from_argb_u8(
                    0xff, 0xaa, 0xbb, 0xcc,
                )),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_int() {
        validate_rp(
            "in",
            "",
            "int",
            "12",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "12".into(),
                kind: super::PropertyValueKind::Integer,
                value_string: "12".into(),
                value_int: 12,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_bool_true() {
        validate_rp(
            "out",
            "",
            "bool",
            "true",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "true".into(),
                kind: super::PropertyValueKind::Boolean,
                value_string: "true".into(),
                value_bool: true,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_bool_false() {
        validate_rp(
            "in-out",
            "",
            "bool",
            "false",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                code: "false".into(),
                kind: super::PropertyValueKind::Boolean,
                value_string: "false".into(),
                value_bool: false,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_struct_with_array() {
        validate_rp(
            "in-out",
            r#"
            struct FooStruct { first: [string] }
            "#,
            "FooStruct",
            "{ first: [ \"first of a kind\", \"second of a kind\"] }",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                has_setter: true,
                kind: super::PreviewDataKind::Json,
                ..Default::default()
            },
            super::PropertyValue {
                kind: super::PropertyValueKind::Code,
                code:
                    "{\n  \"first\": [\n    \"first of a kind\",\n    \"second of a kind\"\n  ]\n}"
                        .into(),
                ..Default::default()
            },
        );
    }
}
