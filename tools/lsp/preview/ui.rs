// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{collections::HashMap, iter::once, rc::Rc};

use slint::{Model, SharedString, VecModel};
use slint_interpreter::{DiagnosticLevel, PlatformError};

use crate::common::ComponentInformation;

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

    ui.set_current_style(style.clone().into());
    ui.set_experimental(experimental);
    ui.set_known_styles(style_model.into());

    ui.on_add_new_component(super::add_new_component);
    ui.on_rename_component(super::rename_component);
    ui.on_style_changed(super::change_style);
    ui.on_show_component(super::show_component);
    ui.on_show_document(|file, line, column| {
        use lsp_types::{Position, Range};
        let pos = Position::new((line as u32).saturating_sub(1), (column as u32).saturating_sub(1));
        super::ask_editor_to_show_document(&file, Range::new(pos, pos))
    });
    ui.on_show_preview_for(super::show_preview_for);
    ui.on_unselect(super::element_selection::unselect_element);
    ui.on_reselect(super::element_selection::reselect_element);
    ui.on_select_at(super::element_selection::select_element_at);
    ui.on_select_behind(super::element_selection::select_element_behind);
    ui.on_can_drop(super::can_drop_component);
    ui.on_drop(super::drop_component);
    ui.on_selected_element_resize(super::resize_selected_element);
    ui.on_selected_element_can_move_to(super::can_move_selected_element);
    ui.on_selected_element_move(super::move_selected_element);
    ui.on_selected_element_delete(super::delete_selected_element);

    ui.on_navigate(super::navigate);

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
    ui.set_known_components(result.into());
}
