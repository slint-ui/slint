// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{FilterModel, Global, Model, ModelRc, VecModel};

use super::{
    Diagnostic, DiagnosticLevel, Diagnostics, EditorUi, FileDiagnosticSummary, PreviewAvailability,
};

pub(in crate::preview) fn setup(global: &Diagnostics<'_>) {
    let entries = Rc::new(VecModel::<Diagnostic>::default());
    global.set_entries(entries.clone().into());
    global.set_errors(ModelRc::new(FilterModel::new(entries.clone(), |entry: &Diagnostic| {
        entry.level == DiagnosticLevel::Error
    })));
    global.set_warnings(ModelRc::new(FilterModel::new(entries, |entry: &Diagnostic| {
        entry.level == DiagnosticLevel::Warning
    })));
    let weak = <Diagnostics as Global<'_, EditorUi>>::as_weak(global);
    global.on_file_diagnostics(move |path| {
        let Some(global) = weak.upgrade() else { return Default::default() };
        file_summary(&global.get_entries(), Path::new(path.as_str()))
    });
    global.on_format_location(|diagnostic| {
        let root = crate::preview::PREVIEW_STATE.with_borrow(|state| {
            state.current_project_root.as_ref().and_then(|root| root.to_file_path().ok())
        });
        format_location(&diagnostic, root.as_deref()).into()
    });
    let weak = <Diagnostics as Global<'_, EditorUi>>::as_weak(global);
    global.on_copy_to_clipboard(move || {
        let Some(global) = weak.upgrade() else { return };
        let text = global
            .get_entries()
            .iter()
            .map(|entry| {
                let location = global.invoke_format_location(entry.clone());
                let level = match entry.level {
                    DiagnosticLevel::Error => "error",
                    DiagnosticLevel::Warning => "warning",
                    DiagnosticLevel::Note => "note",
                    DiagnosticLevel::Debug => "debug",
                };
                if location.is_empty() {
                    format!("{level}: {}", entry.message)
                } else {
                    format!("{location}: {level}: {}", entry.message)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        if let Err(error) = i_slint_backend_selector::with_platform(|platform| {
            platform.set_clipboard_text(&text, i_slint_core::platform::Clipboard::DefaultClipboard);
            Ok(())
        }) {
            tracing::warn!("Failed to copy diagnostics: {error}");
        }
    });
}

fn normalized_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| i_slint_compiler::pathutils::clean_path(path))
}

fn file_summary(entries: &ModelRc<Diagnostic>, path: &Path) -> FileDiagnosticSummary {
    entries.model_tracker().track_any_change(entries.row_count(), i_slint_core::InternalToken);
    let path = normalized_path(path);
    let mut summary = FileDiagnosticSummary::default();
    for entry in entries.iter() {
        if entry.file.is_empty()
            || !normalized_path(Path::new(entry.file.as_str())).starts_with(&path)
        {
            continue;
        }
        match entry.level {
            DiagnosticLevel::Error => summary.error_count += 1,
            DiagnosticLevel::Warning => summary.warning_count += 1,
            _ => {}
        }
    }
    summary
}

pub(super) fn format_location(diagnostic: &Diagnostic, root: Option<&Path>) -> String {
    if diagnostic.file.is_empty() {
        return String::new();
    }
    let path = Path::new(diagnostic.file.as_str());
    let file = root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    if diagnostic.line > 0 {
        format!("{file}:{}:{}", diagnostic.line, diagnostic.column)
    } else {
        file
    }
}

pub(in crate::preview) fn clear(global: &Diagnostics<'_>) {
    if let Some(entries) = global.get_entries().as_any().downcast_ref::<VecModel<Diagnostic>>() {
        entries.set_vec(Vec::new());
    }
}

pub(in crate::preview) fn publish(
    global: &Diagnostics<'_>,
    diagnostics: &[slint_interpreter::Diagnostic],
) {
    for diagnostic in diagnostics {
        let (line, column) = diagnostic.line_column();
        append(
            global,
            Diagnostic {
                file: diagnostic
                    .source_file()
                    .map(|file| file.to_string_lossy().into_owned().into())
                    .unwrap_or_default(),
                line: line as i32,
                column: column as i32,
                message: diagnostic.message().into(),
                level: match diagnostic.level() {
                    slint_interpreter::DiagnosticLevel::Error => DiagnosticLevel::Error,
                    slint_interpreter::DiagnosticLevel::Warning => DiagnosticLevel::Warning,
                    slint_interpreter::DiagnosticLevel::Note => DiagnosticLevel::Note,
                    _ => DiagnosticLevel::Debug,
                },
            },
        );
    }
}

pub(in crate::preview) fn append(global: &Diagnostics<'_>, diagnostic: Diagnostic) {
    if let Some(entries) = global.get_entries().as_any().downcast_ref::<VecModel<Diagnostic>>() {
        entries.push(diagnostic);
    }
}

pub(in crate::preview) fn availability(
    compiled: bool,
    has_errors: bool,
    same_preview: bool,
) -> PreviewAvailability {
    if compiled {
        PreviewAvailability::Current
    } else if !has_errors {
        PreviewAvailability::NoComponent
    } else if same_preview {
        PreviewAvailability::Stale
    } else {
        PreviewAvailability::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::ComponentHandle;

    fn error(file: &str) -> Diagnostic {
        Diagnostic {
            file: file.into(),
            line: 2,
            column: 5,
            message: "Expected '}'".into(),
            level: DiagnosticLevel::Error,
        }
    }

    #[test]
    fn severity_views_follow_in_place_changes_and_reset() {
        i_slint_backend_testing::init_no_event_loop();
        let window = super::super::create_ui().unwrap();
        let global = window.global::<Diagnostics>();
        setup(&global);
        let model = global.get_entries();
        let entries = model.as_any().downcast_ref::<VecModel<Diagnostic>>().unwrap();
        append(&global, error("/project/Main.slint"));
        assert_eq!(global.get_errors().row_count(), 1);
        assert_eq!(global.get_warnings().row_count(), 0);
        entries.set_row_data(
            0,
            Diagnostic { level: DiagnosticLevel::Warning, ..error("/project/Main.slint") },
        );
        assert_eq!(global.get_errors().row_count(), 0);
        assert_eq!(global.get_warnings().row_count(), 1);
        clear(&global);
        assert_eq!(global.get_entries().row_count(), 0);
        assert_eq!(global.get_warnings().row_count(), 0);
        append(&global, error("/project/Main.slint"));
        assert_eq!(global.get_errors().row_count(), 1);
    }

    #[test]
    fn file_query_tracks_replacement_and_every_model_mutation() {
        i_slint_backend_testing::init_no_event_loop();
        let window = super::super::create_ui().unwrap();
        let global = window.global::<Diagnostics>();
        setup(&global);
        let tracker = Box::pin(i_slint_core::properties::PropertyTracker::<false>::default());
        let read = || global.invoke_file_diagnostics("/project".into());
        assert_eq!(tracker.as_ref().evaluate(read).error_count, 0);
        let model = global.get_entries();
        let entries = model.as_any().downcast_ref::<VecModel<Diagnostic>>().unwrap();
        entries.push(error("/project/components/Card.slint"));
        assert!(tracker.is_dirty());
        assert_eq!(tracker.as_ref().evaluate(read).error_count, 1);
        entries.set_row_data(0, error("/project-other/Card.slint"));
        assert!(tracker.is_dirty());
        assert_eq!(tracker.as_ref().evaluate(read).error_count, 0);
        entries.remove(0);
        assert!(tracker.is_dirty());
        tracker.as_ref().evaluate(read);
        global.set_entries(ModelRc::new(VecModel::from(vec![error("/project/Main.slint")])));
        assert!(tracker.is_dirty());
        assert_eq!(tracker.as_ref().evaluate(read).error_count, 1);
        assert_eq!(global.invoke_file_diagnostics("/project/components".into()).error_count, 0);
        assert_eq!(global.invoke_file_diagnostics("/project/Main.slint".into()).error_count, 1);
    }

    #[test]
    fn structured_locations_handle_imports_external_files_and_missing_spans() {
        let mut diagnostic = error("/project/components/Card.slint");
        assert_eq!(
            format_location(&diagnostic, Some(Path::new("/project"))),
            "components/Card.slint:2:5"
        );
        assert_eq!(
            format_location(&diagnostic, Some(Path::new("/elsewhere"))),
            "/project/components/Card.slint:2:5"
        );
        diagnostic.line = 0;
        diagnostic.column = 0;
        assert_eq!(
            format_location(&diagnostic, Some(Path::new("/project"))),
            "components/Card.slint"
        );
        diagnostic.file = Default::default();
        assert_eq!(format_location(&diagnostic, None), "");
    }

    #[test]
    fn copy_includes_all_entries_and_compilation_does_not_disable_current_preview() {
        i_slint_backend_testing::init_no_event_loop();
        let window = super::super::create_ui().unwrap();
        let global = window.global::<Diagnostics>();
        setup(&global);
        for _ in 0..5 {
            append(&global, error("Main.slint"));
        }
        global.invoke_copy_to_clipboard();
        let clipboard = i_slint_backend_selector::with_platform(|platform| {
            Ok(platform.clipboard_text(i_slint_core::platform::Clipboard::DefaultClipboard))
        })
        .unwrap()
        .unwrap();
        assert_eq!(clipboard.lines().count(), 5);
        assert!(clipboard.contains("Main.slint:2:5: error: Expected '}'"));
        global.set_preview_availability(PreviewAvailability::Current);
        global.set_compiling(true);
        assert!(global.get_preview_editable());
        global.set_preview_availability(PreviewAvailability::Stale);
        assert!(!global.get_preview_editable());
        assert_eq!(availability(false, true, false), PreviewAvailability::Unavailable);
        assert_eq!(availability(false, false, true), PreviewAvailability::NoComponent);
    }

    #[test]
    fn preview_mocks_supply_models_queries_and_actions_without_host_setup() {
        i_slint_backend_testing::init_no_event_loop();
        for preview in [false, true] {
            let mut compiler = slint_interpreter::Compiler::default();
            compiler.set_style("fluent".into());
            compiler.compiler_configuration(i_slint_core::InternalToken).is_preview = preview;
            let source = r#"
                import { Diagnostics, DiagnosticsMock, DiagnosticMockScenario } from "diagnostics.slint";
                export component MockHarness inherits Window {
                    out property <int> errors: Diagnostics.errors.length;
                    out property <int> folder-errors: Diagnostics.file-diagnostics("/mock/project/components").error-count;
                    out property <string> location: Diagnostics.format-location(Diagnostics.entries[0]);
                    out property <int> copies: DiagnosticsMock.copy-count;
                    callback show-imports();
                    show-imports => { DiagnosticsMock.scenario = DiagnosticMockScenario.Imports; }
                    callback copy();
                    copy => { Diagnostics.copy-to-clipboard(); }
                }
            "#;
            let result = spin_on::spin_on(compiler.build_from_source(
                source.into(),
                Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/mock-contract.slint"),
            ));
            assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
            let instance = result.components().next().unwrap().create().unwrap();
            instance.invoke("show-imports", &[]).unwrap();
            assert_eq!(
                instance.get_property("errors").unwrap(),
                slint_interpreter::Value::Number(if preview { 1.0 } else { 0.0 })
            );
            assert_eq!(
                instance.get_property("folder-errors").unwrap(),
                slint_interpreter::Value::Number(if preview { 1.0 } else { 0.0 })
            );
            assert_eq!(
                instance.get_property("location").unwrap(),
                slint_interpreter::Value::String(if preview {
                    "components/Card.slint:12:9".into()
                } else {
                    "".into()
                })
            );
            instance.invoke("copy", &[]).unwrap();
            assert_eq!(
                instance.get_property("copies").unwrap(),
                slint_interpreter::Value::Number(if preview { 1.0 } else { 0.0 })
            );
        }
    }
}
