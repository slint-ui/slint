// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use slint::{ComponentHandle, FilterModel, Model, SharedString, VecModel};

use crate::preview::ui;
use slint_interpreter::DiagnosticLevel;

pub fn setup(global: &ui::Diagnostics<'_>) {
    global.set_entries(Rc::new(VecModel::<ui::Diagnostic>::default()).into());
    global.set_errors(
        Rc::new(FilterModel::new(global.get_entries(), |diagnostic| {
            diagnostic.level == ui::DiagnosticLevel::Error
        }))
        .into(),
    );
    global.set_warnings(
        Rc::new(FilterModel::new(global.get_entries(), |diagnostic| {
            diagnostic.level == ui::DiagnosticLevel::Warning
        }))
        .into(),
    );
}

pub fn append_diagnostic(
    global: &ui::Diagnostics<'_>,
    level: ui::DiagnosticLevel,
    location: Option<(SharedString, usize, usize)>,
    message: &str,
) {
    let entries = global.get_entries();
    let Some(model) = entries.as_any().downcast_ref::<VecModel<ui::Diagnostic>>() else {
        return;
    };

    let location = location.unwrap_or_default();

    model.push(ui::Diagnostic {
        file: location.0,
        line: location.1 as i32,
        column: location.2 as i32,
        message: message.into(),
        level,
    });
}

pub fn clear_diagnostics(global: &ui::Diagnostics<'_>) {
    let entries = global.get_entries();
    if let Some(model) = entries.as_any().downcast_ref::<VecModel<ui::Diagnostic>>() {
        model.clear();
    }
}

pub fn set_diagnostics(
    global: &ui::Diagnostics<'_>,
    diagnostics: &[slint_interpreter::Diagnostic],
) {
    diagnostics.iter().for_each(|diagnostic| {
        let location = diagnostic.source_file().map(|path| {
            let (line, column) = diagnostic.line_column();
            (path.to_string_lossy().to_string().into(), line, column)
        });

        let level = match diagnostic.level() {
            DiagnosticLevel::Error => ui::DiagnosticLevel::Error,
            DiagnosticLevel::Warning => ui::DiagnosticLevel::Warning,
            DiagnosticLevel::Note => ui::DiagnosticLevel::Note,
            _ => ui::DiagnosticLevel::Debug,
        };

        append_diagnostic(global, level, location, diagnostic.message());
    });
}

pub fn set_compiling(compiling: bool) {
    i_slint_core::api::invoke_from_event_loop(move || {
        crate::preview::PREVIEW_STATE.with_borrow(|preview_state| {
            if let Some(editor_ui) = preview_state.editor_ui.as_ref() {
                editor_ui.global::<ui::Diagnostics>().set_compiling(compiling);
            }
        });
    })
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_severity_views(global: &ui::Diagnostics<'_>) {
        let entries = global.get_entries();
        for (level, view) in [
            (ui::DiagnosticLevel::Error, global.get_errors()),
            (ui::DiagnosticLevel::Warning, global.get_warnings()),
        ] {
            let expected =
                entries.iter().filter(|diagnostic| diagnostic.level == level).collect::<Vec<_>>();
            assert_eq!(view.iter().collect::<Vec<_>>(), expected);
            assert_eq!(view.row_count(), expected.len());
        }
    }

    #[test]
    fn severity_views_follow_appends_and_clearing() {
        i_slint_backend_testing::init_no_event_loop();
        let editor = ui::EditorUi::new().unwrap();
        let global = editor.global::<ui::Diagnostics>();
        setup(&global);
        let entries = global.get_entries();

        assert_severity_views(&global);
        for index in 0..24 {
            let level = [
                ui::DiagnosticLevel::Debug,
                ui::DiagnosticLevel::Note,
                ui::DiagnosticLevel::Warning,
                ui::DiagnosticLevel::Error,
            ][index % 4];
            append_diagnostic(&global, level, None, &format!("Message {index}"));
            assert_severity_views(&global);
        }
        assert_eq!(global.get_errors().row_count(), 6);
        assert_eq!(global.get_warnings().row_count(), 6);

        clear_diagnostics(&global);
        assert_eq!(entries, global.get_entries());
        assert_eq!(entries.row_count(), 0);
        assert_severity_views(&global);

        append_diagnostic(&global, ui::DiagnosticLevel::Error, None, "After clearing");
        assert_severity_views(&global);
        assert_eq!(global.get_errors().row_count(), 1);
    }
}
