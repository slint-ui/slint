// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::Path;

use slint::{Model, ModelRc, VecModel};
use slint_interpreter::{Diagnostic, DiagnosticLevel};

use super::{Api, LogMessageLevel, PreviewAvailability, PreviewDiagnostic};

pub(super) fn setup(api: &Api<'_>, weak: slint::Weak<Api<'static>>) {
    api.on_copy_preview_diagnostics(move || {
        let Some(api) = weak.upgrade() else { return };
        let text = format(api.get_preview_diagnostics().iter());
        if let Err(error) = i_slint_backend_selector::with_platform(|platform| {
            platform.set_clipboard_text(&text, i_slint_core::platform::Clipboard::DefaultClipboard);
            Ok(())
        }) {
            tracing::warn!("Failed to copy preview diagnostics: {error}");
        }
    });
}

pub(in crate::preview) fn display_path(path: &Path, root: Option<&Path>) -> String {
    root.and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

pub(super) fn project(diagnostic: &Diagnostic, root: Option<&Path>) -> PreviewDiagnostic {
    let path = diagnostic.source_file().map(|path| display_path(path, root)).unwrap_or_default();
    let (line, column) = diagnostic.line_column();
    let location = if line == 0 { path.clone() } else { format!("{path}:{line}:{column}") };
    PreviewDiagnostic {
        level: match diagnostic.level() {
            DiagnosticLevel::Error => LogMessageLevel::Error,
            DiagnosticLevel::Warning => LogMessageLevel::Warning,
            DiagnosticLevel::Note => LogMessageLevel::Note,
            _ => LogMessageLevel::Debug,
        },
        path: path.into(),
        location: location.into(),
        message: diagnostic.message().into(),
    }
}

pub(in crate::preview) fn publish(api: &Api<'_>, diagnostics: &[Diagnostic], root: Option<&Path>) {
    let rows: Vec<_> = diagnostics.iter().map(|diagnostic| project(diagnostic, root)).collect();
    let errors: Vec<_> =
        rows.iter().filter(|row| row.level == LogMessageLevel::Error).cloned().collect();
    api.set_preview_error_count(errors.len() as i32);
    api.set_preview_warning_count(
        rows.iter().filter(|row| row.level == LogMessageLevel::Warning).count() as i32,
    );
    api.set_preview_warnings(ModelRc::new(VecModel::from(
        rows.iter()
            .filter(|row| row.level == LogMessageLevel::Warning)
            .take(3)
            .cloned()
            .collect::<Vec<_>>(),
    )));
    api.set_preview_errors(ModelRc::new(VecModel::from(
        errors.into_iter().take(3).collect::<Vec<_>>(),
    )));
    api.set_preview_diagnostics(ModelRc::new(VecModel::from(rows)));
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

pub(super) fn format(rows: impl Iterator<Item = PreviewDiagnostic>) -> String {
    rows.map(|row| {
        let level = match row.level {
            LogMessageLevel::Error => "error",
            LogMessageLevel::Warning => "warning",
            LogMessageLevel::Note => "note",
            _ => "debug",
        };
        if row.location.is_empty() {
            format!("{level}: {}", row.message)
        } else {
            format!("{}: {level}: {}", row.location, row.message)
        }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_limits_visible_errors_copies_all_and_clears_on_repair() {
        use slint::ComponentHandle;
        i_slint_backend_testing::init_no_event_loop();
        let window = super::super::create_ui().unwrap();
        let api = window.global::<Api>();
        setup(&api, <Api as slint::Global<'_, super::super::EditorUi>>::as_weak(&api));
        let path = i_slint_editor_preview::test::main_test_file_name();
        let (diagnostics, _, _, _) = spin_on::spin_on(crate::preview::parse_source(
            Default::default(),
            path.clone(),
            Some(1),
            "export component Broken { unknown-a: 1; unknown-b: 2; unknown-c: 3; unknown-d: 4; }"
                .into(),
            "fluent".into(),
            None,
            |_| Box::pin(async { None }),
        ));
        publish(&api, &diagnostics, path.parent());
        assert_eq!(api.get_preview_error_count(), 4);
        assert_eq!(api.get_preview_errors().row_count(), 3);
        api.invoke_copy_preview_diagnostics();
        let clipboard = i_slint_backend_selector::with_platform(|platform| {
            Ok(platform.clipboard_text(i_slint_core::platform::Clipboard::DefaultClipboard))
        })
        .unwrap()
        .unwrap();
        assert_eq!(clipboard.lines().count(), 4);
        assert!(clipboard.contains("unknown-d"));
        publish(&api, &[], path.parent());
        assert_eq!(api.get_preview_error_count(), 0);
        assert_eq!(api.get_preview_warning_count(), 0);
        assert_eq!(api.get_preview_diagnostics().row_count(), 0);
        assert_eq!(api.get_preview_errors().row_count(), 0);
        assert_eq!(api.get_preview_warnings().row_count(), 0);
    }

    #[test]
    fn projection_preserves_compiler_messages_and_one_based_locations() {
        i_slint_backend_testing::init_no_event_loop();
        let path = i_slint_editor_preview::test::main_test_file_name();
        let (diagnostics, compiled, _, _) = spin_on::spin_on(crate::preview::parse_source(
            Default::default(),
            path.clone(),
            Some(1),
            "export component Broken {\n    unknown-property: 42;\n}".into(),
            "fluent".into(),
            None,
            |_| Box::pin(async { None }),
        ));
        assert!(compiled.is_none());
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.level() == DiagnosticLevel::Error)
            .unwrap();
        let row = project(diagnostic, path.parent());
        assert_eq!(row.message.as_str(), diagnostic.message());
        assert_eq!(row.path.as_str(), path.file_name().unwrap().to_str().unwrap());
        assert_eq!(row.location.as_str(), format!("{}:2:5", row.path));
    }

    #[test]
    fn preview_availability_distinguishes_failures_from_empty_files() {
        assert_eq!(availability(true, false, false), PreviewAvailability::Current);
        assert_eq!(availability(true, false, true), PreviewAvailability::Current);
        assert_eq!(availability(false, true, true), PreviewAvailability::Stale);
        assert_eq!(availability(false, true, false), PreviewAvailability::Unavailable);
        assert_eq!(availability(false, false, true), PreviewAvailability::NoComponent);
        assert_eq!(availability(false, false, false), PreviewAvailability::NoComponent);
    }

    #[test]
    fn diagnostic_paths_and_copy_preserve_external_locations() {
        assert_eq!(
            display_path(Path::new("/project/components/Card.slint"), Some(Path::new("/project"))),
            "components/Card.slint"
        );
        assert_eq!(
            display_path(Path::new("/external/Card.slint"), Some(Path::new("/project"))),
            "/external/Card.slint"
        );
        let rows = [
            PreviewDiagnostic {
                level: LogMessageLevel::Error,
                location: "components/Card.slint:18:21".into(),
                message: "Expected '}'".into(),
                ..Default::default()
            },
            PreviewDiagnostic {
                level: LogMessageLevel::Warning,
                message: "Warning without a location".into(),
                ..Default::default()
            },
        ];
        assert_eq!(
            format(rows.into_iter()),
            "components/Card.slint:18:21: error: Expected '}'\nwarning: Warning without a location"
        );
    }
}
