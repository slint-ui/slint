// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::ComponentHandle;

use crate::preview;
use crate::preview::settings::{Project, SETTINGS_FILE, TOOL_NAME, VisualEditorSettings};
use crate::preview::ui::{Api, EditorUi, Project as ProjectGlobal};

#[cfg(not(target_arch = "wasm32"))]
const NEW_PROJECT_NAME: &str = "Slint UI Project";
const NEW_PROJECT_MAIN_FILE: &str = "main.slint";
const NEW_PROJECT_MAIN_FILE_CONTENTS: &str = r#"export component MainWindow inherits Window {
    width: 400px;
    height: 300px;

    Text {
        text: "Hello from Slint";
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}
"#;

pub fn load_settings() -> VisualEditorSettings {
    i_slint_editor_preview::settings_store::load(TOOL_NAME, SETTINGS_FILE)
        .and_then(|contents| VisualEditorSettings::deserialize(&contents))
        .unwrap_or_default()
}

pub fn setup(
    editor_ui: &EditorUi,
    settings: &VisualEditorSettings,
    start_project: Rc<dyn Fn(Project) -> bool>,
) {
    let api = editor_ui.global::<Api>();
    api.set_startup_wizard_visible(true);

    let project = editor_ui.global::<ProjectGlobal>();
    project.set_file_tree(Default::default());
    project.set_selected_project_file(Default::default());
    preview::apply_visible_recent_projects(editor_ui, settings);

    let editor_ui_weak = editor_ui.as_weak();
    let start_existing_project = start_project.clone();
    project.on_open_existing_project(move || {
        let window = editor_ui_weak.upgrade().map(|editor_ui| editor_ui.window().window_handle());
        let Some(path) = choose_project_file(window) else {
            return false;
        };
        match Project::from_file(path, None) {
            Ok(project) => start_existing_project(project),
            Err(error) => {
                tracing::warn!("Failed to open project: {error}");
                false
            }
        }
    });

    let editor_ui_weak = editor_ui.as_weak();
    let start_new_project = start_project.clone();
    project.on_create_new_project(move || {
        let window = editor_ui_weak.upgrade().map(|editor_ui| editor_ui.window().window_handle());
        let Some(root) = choose_new_project_path(window) else {
            return false;
        };
        if let Err(error) = std::fs::create_dir_all(&root) {
            tracing::warn!("Failed to create project directory {}: {error}", root.display());
            return false;
        }
        let path = root.join(NEW_PROJECT_MAIN_FILE);
        if let Err(error) = std::fs::write(&path, NEW_PROJECT_MAIN_FILE_CONTENTS) {
            tracing::warn!("Failed to create project file {}: {error}", path.display());
            return false;
        }
        match Project::from_root(&root, &path, None) {
            Ok(project) => start_new_project(project),
            Err(error) => {
                tracing::warn!("Failed to open new project: {error}");
                false
            }
        }
    });

    let editor_ui_weak = editor_ui.as_weak();
    let settings = settings.clone();
    project.on_open_recent_project(move |recent_project| {
        let root = PathBuf::from(recent_project.root_path.as_str());
        let path = PathBuf::from(recent_project.path.as_str());
        let component =
            (!recent_project.component.is_empty()).then(|| recent_project.component.to_string());
        match Project::from_root(&root, &path, component) {
            Ok(project) => start_project(project),
            Err(error) => {
                tracing::warn!("Failed to open recent project: {error}");
                if let Some(editor_ui) = editor_ui_weak.upgrade() {
                    preview::apply_visible_recent_projects(&editor_ui, &settings);
                }
                false
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn choose_project_file(window: Option<slint::WindowHandle>) -> Option<PathBuf> {
    let dialog =
        rfd::FileDialog::new().set_title("Open Slint File").add_filter("Slint files", &["slint"]);
    with_parent(dialog, window).pick_file()
}

#[cfg(target_arch = "wasm32")]
fn choose_project_file(_window: Option<slint::WindowHandle>) -> Option<PathBuf> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn with_parent(dialog: rfd::FileDialog, window: Option<slint::WindowHandle>) -> rfd::FileDialog {
    match window {
        Some(window) => dialog.set_parent(&window),
        None => dialog,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn unique_new_project_path(parent: &Path) -> PathBuf {
    let path = parent.join(NEW_PROJECT_NAME);
    if !path.exists() {
        return path;
    }

    for index in 2.. {
        let path = parent.join(format!("{NEW_PROJECT_NAME} {index}"));
        if !path.exists() {
            return path;
        }
    }

    unreachable!("unbounded project-name search must find a free path")
}

#[cfg(not(target_arch = "wasm32"))]
fn choose_new_project_path(window: Option<slint::WindowHandle>) -> Option<PathBuf> {
    let parent = default_new_project_parent();
    let path = unique_new_project_path(&parent);
    let file_name = path.file_name()?.to_string_lossy();

    let dialog = rfd::FileDialog::new()
        .set_title("New Slint UI Project")
        .set_directory(parent)
        .set_file_name(file_name.as_ref());
    with_parent(dialog, window).save_file()
}

#[cfg(target_arch = "wasm32")]
fn choose_new_project_path(_window: Option<slint::WindowHandle>) -> Option<PathBuf> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn default_new_project_parent() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(documents) =
        home.as_ref().map(|home| home.join("Documents")).filter(|path| path.is_dir())
    {
        return documents;
    }
    if let Some(home) = home.filter(|path| path.is_dir()) {
        return home;
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn unique_new_project_path_uses_base_name_when_free() {
        let directory = tempfile::tempdir().unwrap();

        assert_eq!(
            unique_new_project_path(directory.path()),
            directory.path().join(NEW_PROJECT_NAME)
        );
    }

    #[test]
    fn unique_new_project_path_skips_existing_names() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join(NEW_PROJECT_NAME)).unwrap();
        fs::create_dir(directory.path().join("Slint UI Project 2")).unwrap();
        fs::write(directory.path().join("Slint UI Project 3"), "").unwrap();

        assert_eq!(
            unique_new_project_path(directory.path()),
            directory.path().join("Slint UI Project 4")
        );
    }

    #[test]
    fn new_project_main_file_is_a_window_component() {
        assert!(NEW_PROJECT_MAIN_FILE_CONTENTS.contains("export component MainWindow"));
        assert!(NEW_PROJECT_MAIN_FILE_CONTENTS.contains("inherits Window"));
    }
}
