// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::{collections::HashMap, iter::once, rc::Rc};

use slint::{Model, SharedString, VecModel};
use slint_interpreter::{DiagnosticLevel, PlatformError};

slint::include_modules!();

pub fn create_ui(style: String) -> Result<PreviewUi, PlatformError> {
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

    ui.set_known_styles(style_model.into());
    ui.set_current_style(style.clone().into());
    #[cfg(feature = "experimental")]
    ui.set_experimental(true);

    ui.on_style_changed(super::change_style);
    ui.on_show_document(|file, line, column| {
        use lsp_types::{Position, Range};
        let pos = Position::new((line as u32).saturating_sub(1), (column as u32).saturating_sub(1));
        super::ask_editor_to_show_document(&file, Range::new(pos, pos))
    });
    ui.on_unselect(super::element_selection::unselect_element);
    ui.on_select_at(super::element_selection::select_element_at);
    ui.on_select_behind(super::element_selection::select_element_behind);
    ui.on_can_drop(super::can_drop_component);
    ui.on_drop(super::drop_component);

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

pub fn ui_set_known_components(
    ui: &PreviewUi,
    known_components: &[crate::common::ComponentInformation],
) {
    let mut map: HashMap<String, Vec<ComponentListSubItem>> = Default::default();
    for ci in known_components {
        if ci.is_global {
            continue;
        }
        map.entry(ci.category.clone()).or_default().push(ComponentListSubItem {
            name: ci.name.clone().into(),
            is_builtin: ci.is_builtin,
            is_std_widget: ci.is_std_widget,
            is_exported: ci.is_exported,
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
