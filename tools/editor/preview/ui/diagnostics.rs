// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};
use std::rc::Rc;

use i_slint_core::platform::Clipboard;
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
    global.on_format_location(|diagnostic| format_location(&diagnostic, project_root().as_deref()));
    let global_weak = <ui::Diagnostics as slint::Global<'_, ui::EditorUi>>::as_weak(global);
    global.on_copy_to_clipboard(move || {
        if let Some(global) = global_weak.upgrade() {
            copy_to_clipboard(&global);
        }
    });
}

fn project_root() -> Option<PathBuf> {
    crate::preview::PREVIEW_STATE
        .with_borrow(|state| state.current_project_root.as_ref()?.to_file_path().ok())
}

fn format_location(diagnostic: &ui::Diagnostic, project_root: Option<&Path>) -> SharedString {
    if diagnostic.file.is_empty() {
        return SharedString::default();
    }
    let file = Path::new(diagnostic.file.as_str());
    let file = project_root.and_then(|root| file.strip_prefix(root).ok()).unwrap_or(file);
    let file = file.to_string_lossy();
    if diagnostic.line <= 0 {
        file.as_ref().into()
    } else if diagnostic.column <= 0 {
        format!("{file}:{}", diagnostic.line).into()
    } else {
        format!("{file}:{}:{}", diagnostic.line, diagnostic.column).into()
    }
}

fn copy_to_clipboard(global: &ui::Diagnostics<'_>) {
    let entries = global.get_entries();
    if entries.row_count() == 0 {
        return;
    }
    let project_root = project_root();
    let text = entries
        .iter()
        .map(|diagnostic| {
            let location = format_location(&diagnostic, project_root.as_deref());
            let level = match diagnostic.level {
                ui::DiagnosticLevel::Debug => "debug",
                ui::DiagnosticLevel::Note => "note",
                ui::DiagnosticLevel::Warning => "warning",
                ui::DiagnosticLevel::Error => "error",
            };
            if location.is_empty() {
                format!("{level}: {}", diagnostic.message)
            } else {
                format!("{location}: {level}: {}", diagnostic.message)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if let Err(error) = i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(&text, Clipboard::DefaultClipboard);
        Ok(())
    }) {
        tracing::warn!("Failed to copy diagnostics to clipboard: {error}");
    }
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

pub(in crate::preview) fn set_compilation_diagnostics(
    diagnostics: &mut Vec<slint_interpreter::Diagnostic>,
    no_component: bool,
    has_component: bool,
) {
    if no_component {
        diagnostics.retain(|diagnostic| !is_no_component_diagnostic(diagnostic));
    }
    crate::preview::PREVIEW_STATE.with_borrow_mut(|preview_state| {
        if no_component {
            preview_state.clear_preview();
            preview_state.set_preview_availability(ui::PreviewAvailability::NoComponent);
        } else if !has_component {
            preview_state.set_preview_availability(
                if preview_state.component_instance().is_some() {
                    ui::PreviewAvailability::Stale
                } else {
                    ui::PreviewAvailability::Unavailable
                },
            );
        }
        if let Some(editor_ui) = preview_state.editor_ui.as_ref() {
            clear_diagnostics(&editor_ui.global::<ui::Diagnostics>());
            set_diagnostics(&editor_ui.global::<ui::Diagnostics>(), diagnostics);
        }
    });
}

pub(in crate::preview) fn has_only_no_component_error(
    diagnostics: &[slint_interpreter::Diagnostic],
) -> bool {
    diagnostics.iter().any(is_no_component_diagnostic)
        && diagnostics.iter().all(|diagnostic| {
            diagnostic.level() != DiagnosticLevel::Error || is_no_component_diagnostic(diagnostic)
        })
}

// The interpreter emits this error even for valid files without components.
// The editor distinguishes those files from compilation failures through NoComponent.
fn is_no_component_diagnostic(diagnostic: &slint_interpreter::Diagnostic) -> bool {
    diagnostic.level() == DiagnosticLevel::Error
        && diagnostic.source_file().is_none()
        && diagnostic.message() == "No component found"
}

pub(in crate::preview) fn append_preview_debug_message(
    preview_generation: Option<i32>,
    location: Option<(SharedString, usize, usize)>,
    message: &str,
) {
    crate::preview::PREVIEW_STATE.with_borrow(|state| {
        if let Some(editor_ui) = &state.editor_ui
            && preview_generation == Some(editor_ui.global::<ui::Hover>().get_preview_generation())
        {
            append_diagnostic(
                &editor_ui.global::<ui::Diagnostics>(),
                ui::DiagnosticLevel::Debug,
                location,
                message,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_handle_project_paths_and_partial_positions() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        let relative = Path::new("components").join("Card.slint");
        let inside = root.join(&relative);
        let outside = directory.path().join("project-other").join("Main.slint");
        for (file, project_root, expected) in [
            (inside.clone(), Some(root.as_path()), relative.clone()),
            (outside.clone(), Some(root.as_path()), outside),
            (relative.clone(), Some(root.as_path()), relative),
            (inside.clone(), None, inside),
        ] {
            for (line, column, suffix) in
                [(0, 0, ""), (0, 9, ""), (12, 0, ":12"), (12, 9, ":12:9"), (-1, -1, "")]
            {
                let diagnostic = ui::Diagnostic {
                    file: file.to_string_lossy().as_ref().into(),
                    line,
                    column,
                    ..Default::default()
                };
                assert_eq!(
                    format_location(&diagnostic, project_root),
                    format!("{}{suffix}", expected.display())
                );
            }
        }
        assert_eq!(format_location(&ui::Diagnostic::default(), Some(&root)), "");
    }

    #[test]
    fn clipboard_callback_copies_all_entries_and_preserves_clipboard_when_empty() {
        i_slint_backend_testing::init_no_event_loop();
        let editor = ui::EditorUi::new().unwrap();
        let global = editor.global::<ui::Diagnostics>();
        setup(&global);
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("Main.slint");
        let previous_root = crate::preview::PREVIEW_STATE.with_borrow_mut(|state| {
            state
                .current_project_root
                .replace(lsp_types::Url::from_file_path(directory.path()).unwrap())
        });
        append_diagnostic(
            &global,
            ui::DiagnosticLevel::Error,
            Some((file.to_string_lossy().as_ref().into(), 4, 2)),
            "First error",
        );
        for (level, message) in [
            (ui::DiagnosticLevel::Warning, "Warning"),
            (ui::DiagnosticLevel::Note, "Note"),
            (ui::DiagnosticLevel::Debug, "Debug output\nSecond line"),
        ] {
            append_diagnostic(&global, level, None, message);
        }
        let mut expected =
            "Main.slint:4:2: error: First error\nwarning: Warning\nnote: Note\ndebug: Debug output\nSecond line"
                .to_owned();
        for _ in 0..6 {
            append_diagnostic(&global, ui::DiagnosticLevel::Error, None, "Additional error");
            expected.push_str("\nerror: Additional error");
        }
        assert_eq!(global.get_entries().row_count(), 10);
        for (clear_before_copy, expected) in
            [(false, expected.as_str()), (true, "Unrelated clipboard text")]
        {
            if clear_before_copy {
                clear_diagnostics(&global);
                i_slint_backend_selector::with_platform(|platform| {
                    platform.set_clipboard_text(expected, Clipboard::DefaultClipboard);
                    Ok(())
                })
                .unwrap();
            }
            global.invoke_copy_to_clipboard();
            let clipboard = i_slint_backend_selector::with_platform(|platform| {
                Ok(platform.clipboard_text(Clipboard::DefaultClipboard))
            })
            .unwrap();
            assert_eq!(clipboard.as_deref(), Some(expected));
        }
        crate::preview::PREVIEW_STATE
            .with_borrow_mut(|state| state.current_project_root = previous_root);
    }

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
