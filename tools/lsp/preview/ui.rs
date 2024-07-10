// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;
use std::{collections::HashMap, iter::once, rc::Rc};

use lsp_types::Url;
use slint::{Model, SharedString, VecModel};
use slint_interpreter::{DiagnosticLevel, PlatformError};

use crate::common::{self, properties, ComponentInformation};
use crate::util;

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

    api.on_navigate(super::navigate);

    api.on_test_binding(super::test_binding);
    api.on_set_binding(super::set_binding);
    api.on_test_simple_binding(super::test_simple_binding);
    api.on_set_simple_binding(super::set_simple_binding);

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

fn extract_definition_location(
    ci: &ComponentInformation,
) -> (slint::SharedString, slint::SharedString) {
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
    let mut longest_path_prefix = PathBuf::new();

    for (idx, ci) in known_components.iter().enumerate() {
        if ci.is_global {
            continue;
        }
        let (url, pretty_location) = extract_definition_location(ci);
        let item = ComponentItem {
            name: ci.name.clone().into(),
            defined_at: url.clone(),
            pretty_location,
            is_user_defined: !(ci.is_builtin || ci.is_std_widget),
            is_currently_shown: idx == current_component_index,
        };

        if let Some(position) = &ci.defined_at {
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

    let mut all_components = builtin_components;
    all_components.extend_from_slice(&file_components);

    let result = Rc::new(VecModel::from(all_components));
    let api = ui.global::<Api>();
    api.set_known_components(result.into());
}

fn to_range(r: (usize, usize)) -> Option<Range> {
    Some(Range { start: i32::try_from(r.0).ok()?, end: i32::try_from(r.1).ok()? })
}

fn map_property_declaration(
    document_cache: &common::DocumentCache,
    declared_at: &Option<properties::DeclarationInformation>,
) -> Option<PropertyDeclaration> {
    let da = declared_at.as_ref()?;

    let doc = document_cache.get_document(&da.uri)?;
    let doc_node = doc.node.as_ref()?;
    let source_version = doc_node.source_file.version().unwrap_or(-1);

    let pos = util::map_to_offset(&doc_node.source_file, da.start_position);

    Some(PropertyDeclaration {
        source_uri: da.uri.to_string().into(),
        source_version,
        range: to_range((pos, pos))?,
    })
}

fn simplify_string(value: &str) -> Option<String> {
    let mut had_initial_quote = false;
    let mut is_escaped = false;
    let mut last_was_quote = false;
    let mut is_first = true;
    let mut about_to_unicode_escape = false;
    let mut in_unicode_escape = false;
    let mut in_expression_escape = false;
    let mut opening_braces = 0;

    let mut result = String::new();

    for c in value.chars() {
        if last_was_quote || (!is_first && !had_initial_quote) {
            return None;
        }

        match c {
            '"' => {
                if is_first {
                    had_initial_quote = true;
                } else if is_escaped {
                    result.push(c);
                    is_escaped = false;
                } else {
                    last_was_quote = true;
                }
            }
            '\\' => {
                result.push(c);
                is_escaped = !is_escaped;
            }
            'n' => {
                result.push(c);
                is_escaped = false;
            }
            'u' => {
                result.push(c);
                if is_escaped {
                    about_to_unicode_escape = true;
                    is_escaped = false;
                }
            }
            '{' => {
                result.push(c);
                if in_expression_escape {
                    opening_braces += 1;
                } else if about_to_unicode_escape {
                    about_to_unicode_escape = false;
                    in_unicode_escape = true;
                } else if is_escaped {
                    in_expression_escape = true;
                    is_escaped = false;
                }
            }
            '}' => {
                result.push(c);
                if in_expression_escape {
                    if opening_braces == 1 {
                        in_expression_escape = false;
                    }
                    opening_braces -= 1;
                }
                if in_unicode_escape {
                    in_unicode_escape = false;
                }
            }
            'a'..='f' | 'A'..='F' | '0'..='9' => {
                result.push(c);
            }
            _ => {
                result.push(c);
                if in_unicode_escape {
                    return None;
                }
            }
        };
        is_first = false;
    }

    last_was_quote.then_some(result)
}

fn simplify_value(
    document_cache: &common::DocumentCache,
    property_type: &str,
    property_value: &str,
) -> SimpleValueData {
    if property_type == "bool"
        && (property_value == "true" || property_value == "false" || property_value.is_empty())
    {
        let value: slint::SharedString = if property_value == "true" {
            "true".to_string().into()
        } else {
            "false".to_string().into()
        };
        return SimpleValueData {
            widget: "bool".to_string().into(),
            meta_data: Rc::new(VecModel::from(vec![value])).into(),
            visual_items: Rc::new(VecModel::default()).into(),
        };
    } else if property_type == "string" {
        if let Some(simple) = simplify_string(property_value) {
            return SimpleValueData {
                widget: "string".to_string().into(),
                meta_data: Rc::new(VecModel::from(vec![simple.into()])).into(),
                visual_items: Rc::new(VecModel::default()).into(),
            };
        }
    } else if property_type.starts_with("enum ") {
        let property_type = &property_type["enum ".len()..];
        if let i_slint_compiler::langtype::Type::Enumeration(enumeration) =
            &document_cache.global_type_registry().lookup(property_type)
        {
            let short_property_value =
                property_value.strip_prefix(&format!("{property_type}.")).unwrap_or(property_value);
            let type_name: slint::SharedString = property_type.to_string().into();
            let default_value: slint::SharedString = enumeration.default_value.to_string().into();
            let current_value = enumeration
                .values
                .iter()
                .position(|v| v == short_property_value)
                .map(|p| slint::SharedString::from(p.to_string()))
                .unwrap_or_else(|| default_value.clone());
            let visual_values: Vec<_> =
                enumeration.values.iter().map(|v| slint::SharedString::from(v)).collect();
            return SimpleValueData {
                widget: "enum".to_string().into(),
                meta_data: Rc::new(VecModel::from(vec![type_name, default_value, current_value]))
                    .into(),
                visual_items: Rc::new(VecModel::from(visual_values)).into(),
            };
        }
    }

    SimpleValueData {
        widget: slint::SharedString::new(),
        meta_data: Rc::new(VecModel::default()).into(),
        visual_items: Rc::new(VecModel::default()).into(),
    }
}

fn map_property_definition(
    document: &i_slint_compiler::parser::syntax_nodes::Document,
    defined_at: &Option<properties::DefinitionInformation>,
) -> Option<PropertyDefinition> {
    let da = defined_at.as_ref()?;

    Some(PropertyDefinition {
        definition_range: to_range(util::map_to_offsets(
            &document.source_file,
            da.property_definition_range,
        ))?,
        selection_range: to_range(util::map_to_offsets(&document.source_file, da.selection_range))?,
        expression_range: to_range(util::map_to_offsets(
            &document.source_file,
            da.expression_range,
        ))?,
        expression_value: da.expression_value.clone().into(),
    })
}

fn map_properties_to_ui(
    document_cache: &common::DocumentCache,
    properties: Option<properties::QueryPropertyResponse>,
) -> Option<ElementInformation> {
    let properties = &properties?;
    let element = properties.element.as_ref()?;

    let raw_source_uri = Url::parse(&properties.source_uri).ok()?;
    let source_uri: slint::SharedString = raw_source_uri.to_string().into();
    let source_version = properties.source_version;

    let doc = document_cache.get_document(&raw_source_uri)?;
    let doc_node = doc.node.as_ref()?;

    let mut property_groups: Vec<PropertyGroup> = vec![];
    let mut current_group_properties = vec![];
    let mut current_group = String::new();

    fn property_group_from(name: &str, properties: Vec<PropertyInformation>) -> PropertyGroup {
        PropertyGroup {
            group_name: name.to_string().into(),
            properties: Rc::new(VecModel::from(properties)).into(),
        }
    }

    for pi in &properties.properties {
        let declared_at = map_property_declaration(document_cache, &pi.declared_at).unwrap_or(
            PropertyDeclaration {
                source_uri: String::new().into(),
                source_version: -1,
                range: Range { start: 0, end: 0 },
            },
        );
        let defined_at =
            map_property_definition(doc_node, &pi.defined_at).unwrap_or(PropertyDefinition {
                definition_range: Range { start: 0, end: 0 },
                selection_range: Range { start: 0, end: 0 },
                expression_range: Range { start: 0, end: 0 },
                expression_value: String::new().into(),
            });

        let simple_value = {
            let value = if let Some(da) = &pi.defined_at {
                da.expression_value.clone()
            } else {
                String::new()
            };
            simplify_value(document_cache, &pi.type_name, &value)
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
            type_name: pi.type_name.clone().into(),
            declared_at,
            defined_at,
            simple_value,
        });
    }

    if !current_group_properties.is_empty() {
        property_groups.push(property_group_from(&current_group, current_group_properties));
    }

    let element_range = util::map_to_offsets(
        &doc_node.source_file,
        element.range.unwrap_or(lsp_types::Range {
            start: lsp_types::Position::new(1, 1),
            end: lsp_types::Position::new(1, 1),
        }),
    );

    Some(ElementInformation {
        id: element.id.clone().into(),
        type_name: element.type_name.clone().into(),
        source_uri,
        source_version,
        range: to_range(element_range)?,

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
