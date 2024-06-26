// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
    let mut map: HashMap<String, Vec<ComponentItem>> = Default::default();
    for (idx, ci) in known_components.iter().enumerate() {
        if ci.is_global {
            continue;
        }
        let (url, pretty_location) = extract_definition_location(ci);
        map.entry(ci.category.clone()).or_default().push(ComponentItem {
            name: ci.name.clone().into(),
            defined_at: url,
            pretty_location,
            is_user_defined: !(ci.is_builtin || ci.is_std_widget),
            is_currently_shown: idx == current_component_index,
        });
    }
    let mut result = map
        .into_iter()
        .map(|(k, v)| {
            let model = Rc::new(VecModel::from(v));
            ComponentListItem { category: k.into(), components: model.into() }
        })
        .collect::<Vec<_>>();
    result.sort_by_key(|k| k.category.clone());

    let result = Rc::new(VecModel::from(result));
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
