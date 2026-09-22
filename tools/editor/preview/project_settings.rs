// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use slint::ComponentHandle;

use super::{PreviewState, ui, undo_redo};

const FILE_NAME: &str = "slint.project.json";

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub(super) struct CanvasSize {
    pub width: f32,
    pub height: f32,
}

impl Default for CanvasSize {
    fn default() -> Self {
        Self { width: 390., height: 720. }
    }
}

impl CanvasSize {
    fn validate(self) -> Result<Self, String> {
        if [self.width, self.height].into_iter().all(|v| v.is_finite() && v > 0.) {
            Ok(self)
        } else {
            Err("Canvas width and height must be positive, finite numbers.".into())
        }
    }
}

pub(super) struct ProjectSettings {
    path: PathBuf,
    contents: Option<Vec<u8>>,
    document: Value,
    pub size: CanvasSize,
    load_error: Option<String>,
    pub error: String,
}

impl ProjectSettings {
    fn load(root: &Path) -> Self {
        let mut settings = Self {
            path: root.join(FILE_NAME),
            contents: None,
            document: json!({}),
            size: CanvasSize::default(),
            load_error: None,
            error: String::new(),
        };
        let result = (|| {
            settings.contents = read_optional(&settings.path)?;
            if let Some(contents) = &settings.contents {
                let document: Value =
                    serde_json::from_slice(contents).map_err(|e| e.to_string())?;
                if !document.is_object() {
                    return Err("Project settings must be a JSON object.".into());
                }
                if let Some(editor) = document.get("visual-editor") {
                    if !editor.is_object() {
                        return Err("Visual editor settings must be a JSON object.".into());
                    }
                    if let Some(canvas) = editor.get("canvas") {
                        let size: CanvasSize =
                            serde_json::from_value(canvas.clone()).map_err(|e| e.to_string())?;
                        settings.size = size.validate()?;
                    }
                }
                settings.document = document;
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            settings.error = format!("Cannot load {FILE_NAME}: {error}");
            settings.load_error = Some(settings.error.clone());
        }
        settings
    }

    fn save(&mut self, size: CanvasSize) -> Result<(), String> {
        let size = size.validate()?;
        if let Some(error) = &self.load_error {
            return Err(error.clone());
        }
        if read_optional(&self.path)? != self.contents {
            return Err(format!("{FILE_NAME} changed outside the editor. Reopen the project."));
        }
        let mut document = self.document.clone();
        document["visual-editor"]["canvas"]["width"] = json!(size.width);
        document["visual-editor"]["canvas"]["height"] = json!(size.height);
        let mut contents = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
        contents.push(b'\n');
        let mut temporary = tempfile::NamedTempFile::new_in(self.path.parent().unwrap())
            .map_err(|e| e.to_string())?;
        temporary.write_all(&contents).map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary.persist(&self.path).map_err(|e| e.to_string())?;
        self.contents = Some(contents);
        self.document = document;
        self.size = size;
        self.error.clear();
        Ok(())
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn publish(state: &PreviewState) {
    let Some(editor) = &state.editor_ui else { return };
    let project = editor.global::<ui::Project>();
    let settings = state.project_settings.as_ref();
    let size = settings.map_or_else(CanvasSize::default, |s| s.size);
    project.set_canvas_width(size.width);
    project.set_canvas_height(size.height);
    project.set_settings_key(state.project_settings_generation.to_string().into());
    project.set_settings_available(settings.is_some());
    project.set_settings_editable(
        settings.is_some_and(|s| s.load_error.is_none()) && !undo_redo::edit_pending(state),
    );
    project.set_settings_error(settings.map_or_else(String::new, |s| s.error.clone()).into());
}

pub(super) fn open(state: &mut PreviewState, root: &lsp_types::Url) {
    if state.current_project_root.as_ref() == Some(root) && state.project_settings.is_some() {
        return;
    }
    state.project_settings = root.to_file_path().ok().map(|root| ProjectSettings::load(&root));
    state.project_settings_generation += 1;
    state.undo_redo_stack.clear();
    state.pending_history.clear();
    undo_redo::publish_edit_state(state);
}

pub(super) fn apply(state: &mut PreviewState, size: CanvasSize) -> bool {
    let Some(settings) = &mut state.project_settings else { return false };
    let result = settings.save(size);
    if let Err(error) = &result {
        settings.error = format!("Cannot save {FILE_NAME}: {error}");
    }
    result.is_ok()
}

pub(super) fn setup(editor: &ui::EditorUi) {
    editor.global::<ui::Project>().on_commit_canvas_dimension(|key, dimension, value| {
        super::PREVIEW_STATE.with_borrow_mut(|state| commit(state, &key, &dimension, &value))
    });
}

fn commit(state: &mut PreviewState, key: &str, dimension: &str, value: &str) -> bool {
    if key != state.project_settings_generation.to_string() || undo_redo::edit_pending(state) {
        return false;
    }
    let Some(settings) = &state.project_settings else { return false };
    let before = settings.size;
    let Ok(value) = value.trim().parse::<f32>() else { return false };
    let size = match dimension {
        "width" => CanvasSize { width: value, ..before },
        "height" => CanvasSize { height: value, ..before },
        _ => return false,
    };
    if size.validate().is_err() {
        return false;
    }
    if size == before {
        return true;
    }
    let applied = apply(state, size);
    if applied {
        state.undo_redo_stack.push_canvas(before);
    }
    undo_redo::publish_edit_state(state);
    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_canvas_tracks_project_dimensions_at_each_scale_factor() {
        let component = super::super::test::interpret_test(
            "fluent",
            r#"
            export component Main inherits Window {
                width: 100px;
                height: 80px;
                Rectangle {
                    width: parent.width;
                    height: parent.height;
                    accessible-role: region;
                    accessible-label: "Canvas size probe";
                }
            }
        "#,
        );
        let editor = ui::EditorUi::new().unwrap();
        let api = editor.global::<ui::Api>();
        super::super::set_preview_factory(
            &editor,
            &api,
            component.definition(),
            Box::new(|_| {}),
            super::super::LoadBehavior::Reload,
        );
        editor.show().unwrap();
        for factor in [1., 2.] {
            i_slint_backend_testing::set_window_scale_factor(&editor, factor);
            assert_eq!(editor.window().scale_factor(), factor);
            for (width, height) in [(390., 720.), (640., 360.)] {
                editor.global::<ui::Project>().set_canvas_width(width);
                editor.global::<ui::Project>().set_canvas_height(height);
                for label in ["Artboard", "Canvas size probe"] {
                    let item = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                        &editor, label,
                    )
                    .next()
                    .unwrap();
                    assert_eq!(item.size(), slint::LogicalSize::new(width, height));
                }
            }
        }
    }

    #[test]
    fn project_switches_reset_dimensions_and_reject_stale_or_pending_edits() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let first_url = lsp_types::Url::from_directory_path(first.path()).unwrap();
        let second_url = lsp_types::Url::from_directory_path(second.path()).unwrap();
        let mut state = PreviewState::default();
        open(&mut state, &first_url);
        state.current_project_root = Some(first_url.clone());
        let old_key = state.project_settings_generation.to_string();
        assert!(commit(&mut state, &old_key, "width", "500"));
        state.workspace_edit_sent = true;
        assert!(!commit(&mut state, &old_key, "height", "500"));
        state.workspace_edit_sent = false;
        open(&mut state, &second_url);
        state.current_project_root = Some(second_url);
        assert!(!commit(&mut state, &old_key, "width", "600"));
        assert_eq!(state.project_settings.as_ref().unwrap().size, CanvasSize::default());
        assert!(!second.path().join(FILE_NAME).exists());
        open(&mut state, &first_url);
        assert_eq!(
            state.project_settings.as_ref().unwrap().size,
            CanvasSize { width: 500., height: 720. }
        );
    }

    #[test]
    fn defaults_save_reopen_and_preserve_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = ProjectSettings::load(dir.path());
        assert_eq!(settings.size, CanvasSize::default());
        assert!(!settings.path.exists());
        settings.document["custom"] = json!(true);
        settings.document["visual-editor"]["custom"] = json!("preserved");
        settings.document["visual-editor"]["canvas"]["custom"] = json!(42);
        let size = CanvasSize { width: 1200., height: 800. };
        settings.save(size).unwrap();
        let loaded = ProjectSettings::load(dir.path());
        assert_eq!(loaded.size, size);
        assert_eq!(loaded.document["custom"], true);
        assert_eq!(loaded.document["visual-editor"]["custom"], "preserved");
        assert!(loaded.document.get("canvas").is_none());
        assert!(loaded.document.get("version").is_none());
        assert_eq!(loaded.document["visual-editor"]["canvas"]["custom"], 42);
    }

    #[test]
    fn existing_project_without_canvas_settings_preserves_other_fields() {
        for document in [
            json!({"version": 42, "name": "Example"}),
            json!({"version": 42, "visual-editor": {"custom": true}}),
        ] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join(FILE_NAME), document.to_string()).unwrap();
            let mut settings = ProjectSettings::load(dir.path());
            assert_eq!(settings.size, CanvasSize::default());
            assert!(settings.load_error.is_none());
            let size = CanvasSize { width: 800., height: 600. };
            settings.save(size).unwrap();
            let loaded = ProjectSettings::load(dir.path());
            assert_eq!(loaded.size, size);
            assert_eq!(loaded.document["version"], 42);
            assert_eq!(loaded.document["name"], document["name"]);
            assert_eq!(
                loaded.document["visual-editor"]["custom"],
                document["visual-editor"]["custom"]
            );
        }
    }

    #[test]
    fn invalid_settings_and_external_changes_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        for contents in [
            "not json",
            "[]",
            r#"{"visual-editor":false}"#,
            r#"{"visual-editor":{"version":1,"canvas":{"width":0,"height":10}}}"#,
        ] {
            std::fs::write(&path, contents).unwrap();
            let mut settings = ProjectSettings::load(dir.path());
            assert!(settings.save(CanvasSize::default()).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
        }
        std::fs::remove_file(&path).unwrap();
        let mut settings = ProjectSettings::load(dir.path());
        std::fs::write(&path, "external edit").unwrap();
        assert!(settings.save(CanvasSize::default()).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "external edit");
    }

    #[test]
    fn failed_writes_and_invalid_sizes_leave_state_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = ProjectSettings::load(dir.path());
        for width in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(settings.save(CanvasSize { width, height: 100. }).is_err());
        }
        std::fs::remove_dir(dir.path()).unwrap();
        assert!(settings.save(CanvasSize { width: 800., height: 600. }).is_err());
        assert_eq!(settings.size, CanvasSize::default());
        assert!(settings.contents.is_none());
    }
}
