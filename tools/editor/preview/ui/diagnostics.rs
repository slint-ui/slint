// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use slint::{ComponentHandle, Model, SharedString, VecModel};

use crate::preview::ui;
use slint_interpreter::DiagnosticLevel;

pub fn setup(global: &ui::Diagnostics<'_>) {
    clear_diagnostics(global);
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
    global.set_entries(Rc::new(VecModel::default()).into());
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
