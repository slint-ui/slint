// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;
use std::{collections::HashMap, rc::Rc};

use i_slint_compiler::literals;
use i_slint_compiler::{
    langtype,
    parser::{syntax_nodes, SyntaxKind, SyntaxNode, TextRange},
};
use lsp_types::Url;
use slint::{SharedString, VecModel};
use slint_interpreter::{DiagnosticLevel, PlatformError};

use crate::common::{self, ComponentInformation};
use crate::preview::properties;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

slint::include_modules!();

pub fn create_ui(experimental: bool) -> Result<PreviewUi, PlatformError> {
    let ui = PreviewUi::new()?;

    let api = ui.global::<Api>();

    api.set_experimental(experimental);

    api.on_add_new_component(super::add_new_component);
    api.on_rename_component(super::rename_component);
    api.on_show_component(super::show_component);
    api.on_show_document(|file, line, column| {
        use lsp_types::{Position, Range};
        let pos = Position::new((line as u32).saturating_sub(1), (column as u32).saturating_sub(1));
        super::ask_editor_to_show_document(&file, Range::new(pos, pos))
    });
    api.on_show_document_offset_range(super::show_document_offset_range);
    api.on_show_preview_for(super::show_preview_for);
    api.on_unselect(super::element_selection::unselect_element);
    api.on_reselect(super::element_selection::reselect_element);
    api.on_select_at(super::element_selection::select_element_at);
    api.on_select_behind(super::element_selection::select_element_behind);
    api.on_can_drop(super::can_drop_component);
    api.on_drop(super::drop_component);
    api.on_selected_element_resize(super::resize_selected_element);
    api.on_selected_element_can_move_to(super::can_move_selected_element);
    api.on_selected_element_move(super::move_selected_element);
    api.on_selected_element_delete(super::delete_selected_element);

    api.on_test_code_binding(super::test_code_binding);
    api.on_test_string_binding(super::test_string_binding);
    api.on_set_code_binding(super::set_code_binding);
    api.on_set_color_binding(super::set_color_binding);
    api.on_set_string_binding(super::set_string_binding);

    Ok(ui)
}

pub fn convert_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) -> Vec<Diagnostics> {
    diagnostics
        .iter()
        .filter(|d| d.level() == DiagnosticLevel::Error)
        .map(|d| {
            let (line, column) = d.line_column();

            Diagnostics {
                level: format!("{:?}", d.level()).into(),
                message: d.message().into(),
                url: d
                    .source_file()
                    .map(|p| p.to_string_lossy().to_string().into())
                    .unwrap_or_default(),
                line: line as i32,
                column: column as i32,
            }
        })
        .collect::<Vec<_>>()
}

fn extract_definition_location(ci: &ComponentInformation) -> (SharedString, SharedString) {
    let Some(url) = ci.defined_at.as_ref().map(|da| &da.url) else {
        return (Default::default(), Default::default());
    };

    let path = url.to_file_path().unwrap_or_default();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    (url.to_string().into(), file_name.into())
}

pub fn ui_set_known_components(
    ui: &PreviewUi,
    known_components: &[crate::common::ComponentInformation],
    current_component_index: usize,
) {
    let mut builtins_map: HashMap<String, Vec<ComponentItem>> = Default::default();
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
            if let Some(library) = position.url.path().strip_prefix("/@") {
                library_map.entry(format!("@{library}")).or_default().push(item);
            } else {
                let path = i_slint_compiler::pathutils::clean_path(
                    &(position.url.to_file_path().unwrap_or_default()),
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
        } else {
            builtins_map.entry(ci.category.clone()).or_default().push(item);
        }
    }

    let mut builtin_components = builtins_map
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
    builtin_components.sort_by_key(|k| k.category.clone());

    let mut library_components = library_map
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
    library_components.sort_by_key(|k| k.category.clone());

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
) -> Option<PropertyDeclaration> {
    let da = declared_at.as_ref()?;
    let source_version = document_cache.document_version_by_path(&da.path).unwrap_or(-1);
    let pos = TextRange::new(da.start_position, da.start_position);

    Some(PropertyDeclaration {
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
        value.tr_context = context.into();
        value.tr_plural = plural.into();
        value.tr_plural_expression = plural_expression.unwrap_or_default().into();
        value.value_string = text.into();
    }
}

fn convert_number_literal(
    node: &SyntaxNode,
) -> Option<(f64, i_slint_compiler::expression_tree::Unit)> {
    let literal = node.child_text(SyntaxKind::NumberLiteral)?;
    let expr = literals::parse_number_literal(literal).ok()?;

    match expr {
        i_slint_compiler::expression_tree::Expression::NumberLiteral(value, unit) => {
            return Some((value, unit))
        }
        _ => None,
    }
}

fn extract_value_with_unit_impl(
    expression: &Option<syntax_nodes::Expression>,
    code: &str,
    units: &[i_slint_compiler::expression_tree::Unit],
) -> Option<(PropertyValueKind, f32, i32)> {
    if let Some(expression) = expression {
        if let Some((value, unit)) = convert_number_literal(&expression) {
            let index = units.iter().position(|u| u == &unit).or_else(|| {
                (units.is_empty() && unit == i_slint_compiler::expression_tree::Unit::None)
                    .then_some(0_usize)
            })?;

            return Some((PropertyValueKind::Float, value as f32, index as i32));
        }
    } else if code.is_empty() {
        return Some((PropertyValueKind::Float, 0.0, 0));
    }

    None
}

fn extract_value_with_unit(
    expression: &Option<syntax_nodes::Expression>,
    units: &[i_slint_compiler::expression_tree::Unit],
    value: &mut PropertyValue,
) {
    let Some((kind, v, index)) = extract_value_with_unit_impl(expression, &value.code, units)
    else {
        return;
    };

    let model = Rc::new(VecModel::from(
        units.iter().map(|u| u.to_string().into()).collect::<Vec<slint::SharedString>>(),
    ));

    value.kind = kind;
    value.value_float = v;
    value.visual_items = model.into();
    value.value_int = index
}

fn extract_color(
    expression: &syntax_nodes::Expression,
    kind: PropertyValueKind,
    value: &mut PropertyValue,
) -> bool {
    if let Some(text) = expression.child_text(SyntaxKind::ColorLiteral) {
        if let Some(color) = literals::parse_color_literal(&text) {
            value.kind = kind;
            value.value_brush = slint::Brush::SolidColor(slint::Color::from_argb_encoded(color));
            return true;
        }
    }
    false
}

fn simplify_value(
    property_type: &langtype::Type,
    code_block_or_expression: &Option<properties::CodeBlockOrExpression>,
) -> PropertyValue {
    use i_slint_compiler::expression_tree::Unit;
    use langtype::Type;

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

    match property_type {
        Type::Float32 => extract_value_with_unit(&expression, &[], &mut value),
        Type::Duration => extract_value_with_unit(&expression, &[Unit::S, Unit::Ms], &mut value),
        Type::PhysicalLength | Type::LogicalLength | Type::Rem => extract_value_with_unit(
            &expression,
            &[Unit::Px, Unit::Cm, Unit::Mm, Unit::In, Unit::Pt, Unit::Phx, Unit::Rem],
            &mut value,
        ),
        Type::Angle => extract_value_with_unit(
            &expression,
            &[Unit::Deg, Unit::Grad, Unit::Turn, Unit::Rad],
            &mut value,
        ),
        Type::Percent => extract_value_with_unit(&expression, &[Unit::Percent], &mut value),
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
                value.kind = PropertyValueKind::Color;
            }
        }
        Type::Brush => {
            if let Some(expression) = expression {
                extract_color(&expression, PropertyValueKind::Brush, &mut value);
                // TODO: Handle gradients...
            } else if value.code.is_empty() {
                value.kind = PropertyValueKind::Brush;
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
                    value.value_string = text.into();
                } else if let Some(tr_node) = &expression.AtTr() {
                    extract_tr_data(tr_node, &mut value)
                }
            } else if value.code.is_empty() {
                value.kind = PropertyValueKind::String;
            }
        }
        Type::Enumeration(enumeration) => {
            value.kind = PropertyValueKind::Enum;
            value.value_string = enumeration.name.clone().into();
            value.default_selection = i32::try_from(enumeration.default_value).unwrap_or_default();
            value.visual_items = Rc::new(VecModel::from(
                enumeration.values.iter().map(SharedString::from).collect::<Vec<_>>(),
            ))
            .into();

            if let Some(expression) = expression {
                if let Some(text) = expression
                    .child_node(SyntaxKind::QualifiedName)
                    .map(|n| i_slint_compiler::object_tree::QualifiedTypeName::from_node(n.into()))
                    .and_then(|n| {
                        n.to_string()
                            .strip_prefix(&format!("{}.", enumeration.name))
                            .map(|s| s.to_string())
                    })
                {
                    value.value_int = enumeration
                        .values
                        .iter()
                        .position(|v| v == &text)
                        .and_then(|v| i32::try_from(v).ok())
                        .unwrap_or_default();
                }
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
) -> Option<ElementInformation> {
    let properties = &properties?;
    let element = properties.element.as_ref()?;

    let raw_source_uri = Url::parse(&properties.source_uri).ok()?;
    let source_uri: SharedString = raw_source_uri.to_string().into();
    let source_version = properties.source_version;

    let mut property_groups: Vec<PropertyGroup> = vec![];
    let mut current_group_properties = vec![];
    let mut current_group = String::new();

    fn property_group_from(name: &str, properties: Vec<PropertyInformation>) -> PropertyGroup {
        PropertyGroup {
            group_name: name.into(),
            properties: Rc::new(VecModel::from(properties)).into(),
        }
    }

    for pi in &properties.properties {
        let declared_at = map_property_declaration(document_cache, &pi.declared_at).unwrap_or(
            PropertyDeclaration {
                source_path: String::new().into(),
                source_version: -1,
                range: Range { start: 0, end: 0 },
            },
        );
        let defined_at = map_property_definition(&pi.defined_at).unwrap_or(PropertyDefinition {
            definition_range: Range { start: 0, end: 0 },
            selection_range: Range { start: 0, end: 0 },
            expression_range: Range { start: 0, end: 0 },
            expression_value: String::new().into(),
        });

        let value = {
            let code_block_or_expression =
                pi.defined_at.as_ref().map(|da| da.code_block_or_expression.clone());
            simplify_value(&pi.ty, &code_block_or_expression)
        };

        if pi.group != current_group {
            if !current_group_properties.is_empty() {
                property_groups.push(property_group_from(&current_group, current_group_properties));
            }
            current_group_properties = vec![];
            current_group = pi.group.clone();
        }

        current_group_properties.push(PropertyInformation {
            name: pi.name.clone().into(),
            type_name: pi.ty.to_string().into(),
            declared_at,
            defined_at,
            value,
        });
    }

    if !current_group_properties.is_empty() {
        property_groups.push(property_group_from(&current_group, current_group_properties));
    }

    Some(ElementInformation {
        id: element.id.clone().into(),
        type_name: element.type_name.clone().into(),
        source_uri,
        source_version,
        range: to_ui_range(element.range)?,

        properties: Rc::new(VecModel::from(property_groups)).into(),
    })
}

pub fn ui_set_properties(
    ui: &PreviewUi,
    document_cache: &common::DocumentCache,
    properties: Option<properties::QueryPropertyResponse>,
) {
    let element = map_properties_to_ui(document_cache, properties).unwrap_or(ElementInformation {
        id: "".into(),
        type_name: "".into(),
        source_uri: "".into(),
        source_version: 0,
        range: Range { start: 0, end: 0 },

        properties: Rc::new(VecModel::from(Vec::<PropertyGroup>::new())).into(),
    });

    let api = ui.global::<Api>();
    api.set_current_element(element);
}

#[cfg(test)]
mod tests {
    use crate::language::test::loaded_document_cache;

    use crate::common;
    use crate::preview::properties;

    use i_slint_core::model::Model;

    use super::{PropertyValue, PropertyValueKind};

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

        super::simplify_value(
            &test1.ty,
            &test1.defined_at.as_ref().map(|da| da.code_block_or_expression.clone()),
        )
    }

    #[test]
    fn test_property_bool() {
        let result =
            property_conversion_test(r#"export component Test { in property <bool> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert_eq!(result.value_bool, false);
        assert!(result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: true; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert_eq!(result.value_bool, true);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: false; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Boolean);
        assert_eq!(result.value_bool, false);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: 1.1.round() == 1.1.floor(); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert_eq!(result.value_bool, false);
        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_property_string() {
        let result =
            property_conversion_test(r#"export component Test { in property <string> test1; }"#, 0);
        assert_eq!(result.kind, PropertyValueKind::String);
        assert_eq!(result.is_translatable, false);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_bool, false);
        assert!(result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: ""; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::String);
        assert_eq!(result.is_translatable, false);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_bool, false);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: "string"; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::String);
        assert_eq!(result.is_translatable, false);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_bool, false);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: "" + "test"); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert_eq!(result.is_translatable, false);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_bool, false);
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
        assert_eq!(result.is_translatable, true);
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
        assert_eq!(result.is_translatable, true);
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
        assert_eq!(result.is_translatable, true);
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
        assert_eq!(result.is_translatable, false);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "");
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("" + "test"); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert_eq!(result.is_translatable, false);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "");
        assert!(!result.code.is_empty());
        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("width {}", self.width()); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Code);
        assert_eq!(result.is_translatable, false);
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
        assert_eq!(result.is_translatable, false);

        assert_eq!(result.visual_items.row_count(), 4);

        let result = property_conversion_test(
            r#"export component Test { in property <ImageFit> test1: ImageFit   .    /* abc */ preserve; }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::Enum);
        assert_eq!(result.value_string, "ImageFit");
        assert_eq!(result.value_int, 3);
        assert_eq!(result.default_selection, 0);
        assert_eq!(result.is_translatable, false);

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
        assert_eq!(result.is_translatable, false);

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
        assert_eq!(result.is_translatable, false);

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
}
