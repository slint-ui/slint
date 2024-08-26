// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;
use std::{collections::HashMap, iter::once, rc::Rc};

use i_slint_compiler::{
    langtype,
    parser::{syntax_nodes, SyntaxKind, TextRange},
};
use lsp_types::Url;
use slint::{Model, SharedString, VecModel};
use slint_interpreter::{DiagnosticLevel, PlatformError};

use crate::common::{self, ComponentInformation};
use crate::preview::properties;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

slint::include_modules!();

pub fn create_ui(style: String, experimental: bool) -> Result<PreviewUi, PlatformError> {
    let ui = PreviewUi::new()?;

    // styles:
    let known_styles = once(&"native")
        .chain(i_slint_compiler::fileaccess::styles().iter())
        .filter(|s| s != &&"qt" || i_slint_backend_selector::HAS_NATIVE_STYLE)
        .cloned()
        .collect::<Vec<_>>();
    let style = if known_styles.contains(&style.as_str()) {
        style
    } else {
        known_styles.first().map(|s| s.to_string()).unwrap_or_default()
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
    api.on_set_bool_binding(super::set_bool_binding);
    api.on_set_enum_binding(super::set_enum_binding);
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

fn extract_tr_data(tr_node: &syntax_nodes::AtTr) -> Option<(String, String, String)> {
    let text = tr_node
        .child_text(SyntaxKind::StringLiteral)
        .and_then(|s| i_slint_compiler::literals::unescape_string(&s))?;
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
    // We have expressions -> Edit as code
    tr_node.Expression().next().is_none().then_some((context, plural, text))
}

fn simplify_value(
    property_type: &langtype::Type,
    code_block_or_expression: &Option<properties::CodeBlockOrExpression>,
) -> PropertyValue {
    let expression = code_block_or_expression.as_ref().and_then(|cbe| cbe.expression());

    let mut kind = PropertyValueKind::Code;
    let mut value_bool = false;
    let mut value_int = 0;
    let mut value_string = String::new();
    let mut visual_items = Rc::new(VecModel::default()).into();
    let mut is_translatable = false;
    let mut tr_context = String::new();
    let mut tr_plural = String::new();
    let mut default_selection = 0;
    let code =
        code_block_or_expression.as_ref().map(|cbe| cbe.text().to_string()).unwrap_or_default();

    match property_type {
        langtype::Type::Bool => {
            if let Some(expression) = expression {
                let qualified_name =
                    expression.QualifiedName().map(|qn| qn.text().to_string()).unwrap_or_default();
                if ["true", "false"].contains(&qualified_name.as_str()) {
                    kind = PropertyValueKind::Boolean;
                    value_bool = &qualified_name == "true";
                }
            } else if code.is_empty() {
                kind = PropertyValueKind::Boolean;
            }
        }
        langtype::Type::String => {
            if let Some(expression) = &expression {
                if let Some(text) = expression
                    .child_text(SyntaxKind::StringLiteral)
                    .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
                {
                    kind = PropertyValueKind::String;
                    value_string = text;
                } else if let Some(tr_node) = &expression.AtTr() {
                    if let Some((context, plural, text)) = extract_tr_data(tr_node) {
                        is_translatable = true;
                        tr_context = context;
                        tr_plural = plural;
                        kind = PropertyValueKind::String;
                        value_string = text;
                    }
                }
            } else if code.is_empty() {
                kind = PropertyValueKind::String;
            }
        }
        langtype::Type::Enumeration(enumeration) => {
            kind = PropertyValueKind::Enum;
            value_string = enumeration.name.clone();
            default_selection = i32::try_from(enumeration.default_value).unwrap_or_default();
            visual_items = Rc::new(VecModel::from(
                enumeration.values.iter().map(SharedString::from).collect::<Vec<_>>(),
            ));

            if let Some(expression) = expression {
                if let Some(text) = expression
                    .child_node(SyntaxKind::QualifiedName)
                    .map(|n| i_slint_compiler::object_tree::QualifiedTypeName::from_node(n.into()))
                    .and_then(|n| {
                        n.to_string()
                            .strip_prefix(&format!("{value_string}."))
                            .map(|s| s.to_string())
                    })
                {
                    value_int = enumeration
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

    PropertyValue {
        kind,
        value_bool,
        value_string: value_string.into(),
        value_int,
        value_float: 0.0,
        visual_items: visual_items.into(),
        is_translatable,
        tr_context: tr_context.into(),
        tr_plural: tr_plural.into(),
        code: code.into(),
        default_selection,
    }
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

        super::simplify_value(
            &pi[0].ty,
            &pi[0].defined_at.as_ref().map(|da| da.code_block_or_expression.clone()),
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
            r#"export component Test { in property <string> test1: @tr("{n} string" | "{n} strings" % 15); }"#,
            0,
        );
        assert_eq!(result.kind, PropertyValueKind::String);
        assert_eq!(result.is_translatable, true);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "{n} strings");
        assert_eq!(result.value_string, "{n} string");
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
}
