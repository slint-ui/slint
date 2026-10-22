// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};

use i_slint_live_preview::protocol::PreviewComponent;
use lsp_types::Url;

use super::ui;

pub(crate) const SETTINGS_FILE: &str = "visual-editor-user-settings.json";
pub(crate) const TOOL_NAME: &str = "visual-editor";
const MAX_RECENT_PROJECTS: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Project {
    pub root: PathBuf,
    pub preview: PreviewComponent,
    #[serde(default)]
    pub selected_canvas_preset: i32,
}

#[derive(Default, Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "VisualEditorSettingsSerde", into = "VisualEditorSettingsSerde")]
pub(crate) struct VisualEditorSettings {
    recent_projects: Vec<Project>,
    pub(crate) elements_pane_height: Option<i32>,
    pub(crate) outline_pane_height: Option<i32>,
    pub(crate) left_pane_width: Option<i32>,
    pub(crate) inspector_pane_width: Option<i32>,
}

impl VisualEditorSettings {
    const CURRENT_VERSION: u32 = 1;

    pub(crate) fn serialize(&self) -> String {
        let mut json =
            serde_json::to_string_pretty(self).expect("serializing visual editor settings");
        json.push('\n');
        json
    }

    pub(crate) fn deserialize(contents: &str) -> Option<Self> {
        serde_json::from_str::<VisualEditorSettings>(contents)
            .map_err(|error| tracing::warn!("Ignoring malformed visual editor settings: {error}"))
            .ok()
    }

    pub(crate) fn add_recent_project(&mut self, mut project: Project) -> bool {
        if let Some(existing) =
            self.recent_projects.iter().find(|recent| recent.root == project.root)
        {
            project.selected_canvas_preset = existing.selected_canvas_preset;
        }
        if self.recent_projects.first() == Some(&project) {
            return false;
        }
        self.recent_projects.retain(|recent| recent.root != project.root);
        self.recent_projects.insert(0, project);
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        true
    }

    pub(crate) fn selected_canvas_preset(&self, root: &Path) -> i32 {
        self.recent_projects
            .iter()
            .find(|recent| recent.root == root)
            .map_or(0, |project| project.selected_canvas_preset.max(0))
    }

    pub(crate) fn set_selected_canvas_preset(&mut self, root: &Path, index: i32) -> bool {
        let Some(project) = self.recent_projects.iter_mut().find(|recent| recent.root == root)
        else {
            return false;
        };
        if project.selected_canvas_preset == index {
            return false;
        }
        project.selected_canvas_preset = index;
        true
    }

    pub(crate) fn clear_recent_projects(&mut self) -> bool {
        let changed = !self.recent_projects.is_empty();
        self.recent_projects.clear();
        changed
    }

    pub(crate) fn visible_recent_projects(&self) -> Vec<ui::RecentProject> {
        self.recent_projects
            .iter()
            .filter_map(|project| {
                let root_path = project.root.as_path();
                let path = project.preview.url.to_file_path().ok()?;
                let component = project.preview.component.as_deref()?;
                // TODO: This should use the name configured in the project file.
                let project_name = root_path.file_name()?;
                if !root_path.is_dir() || !path.is_file() {
                    return None;
                }
                Some(ui::RecentProject {
                    name: slint::format!("{}", project_name.display()),
                    root_path: project.root.to_string_lossy().as_ref().into(),
                    component: component.into(),
                    path: path.to_string_lossy().as_ref().into(),
                })
            })
            .collect()
    }
}

impl Project {
    pub(crate) fn from_selected_file(
        path: impl AsRef<Path>,
        component: Option<String>,
    ) -> i_slint_editor_preview::Result<Self> {
        let path = path.as_ref();
        let input = if i_slint_compiler::project_file::is_project_file(path) {
            path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."))
        } else {
            path
        };
        Self::from_input(input, component, None)
    }

    pub(crate) fn from_input(
        path: impl AsRef<Path>,
        component: Option<String>,
        project_root: Option<&Path>,
    ) -> i_slint_editor_preview::Result<Self> {
        let path =
            std::fs::canonicalize(path.as_ref()).unwrap_or(std::path::absolute(path.as_ref())?);
        if path.is_dir() {
            if project_root.is_some() {
                return Err("A project directory cannot be combined with --project-root".into());
            }
            let (main, _) = i_slint_compiler::project_file::resolve_input(&path)?;
            return Self::from_root(&path, &main, component);
        }
        let root =
            project_root.unwrap_or(path.parent().ok_or("Project input has no parent directory")?);
        Self::from_root(root, &path, component)
    }

    pub(crate) fn from_root(
        root: &Path,
        path: &Path,
        component: Option<String>,
    ) -> i_slint_editor_preview::Result<Self> {
        let root = std::fs::canonicalize(root)
            .unwrap_or(i_slint_compiler::source_path::clean_path(&std::path::absolute(root)?));
        let path = std::fs::canonicalize(path)
            .unwrap_or(i_slint_compiler::source_path::clean_path(&std::path::absolute(path)?));
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("slint"))
        {
            return Err(format!("{} is not a Slint file", path.display()).into());
        }
        let url = Url::from_file_path(&path)
            .map_err(|_| format!("Failed to convert {} to URL", path.display()))?;
        Ok(Self { root, preview: PreviewComponent { url, component }, selected_canvas_preset: 0 })
    }
}

/// Helper struct, should be able to load older formats and convert it into the current
/// [`VisualEditorSettings`].
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
struct VisualEditorSettingsSerde {
    version: u32,
    #[serde(default)]
    recent_projects: Vec<Project>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    elements_pane_height: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    outline_pane_height: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    left_pane_width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    inspector_pane_width: Option<i32>,
}

impl TryFrom<VisualEditorSettingsSerde> for VisualEditorSettings {
    type Error = serde_json::Error;

    fn try_from(settings: VisualEditorSettingsSerde) -> Result<Self, Self::Error> {
        if settings.version != Self::CURRENT_VERSION {
            return Err(serde::de::Error::custom(format!(
                "unsupported VisualEditorSettings version {}, expected {}",
                settings.version,
                Self::CURRENT_VERSION
            )));
        }
        let mut recent_projects = settings.recent_projects;
        recent_projects.truncate(MAX_RECENT_PROJECTS);
        Ok(Self {
            recent_projects,
            elements_pane_height: positive_size(settings.elements_pane_height),
            outline_pane_height: positive_size(settings.outline_pane_height),
            left_pane_width: positive_size(settings.left_pane_width),
            inspector_pane_width: positive_size(settings.inspector_pane_width),
        })
    }
}

impl From<VisualEditorSettings> for VisualEditorSettingsSerde {
    fn from(value: VisualEditorSettings) -> Self {
        let VisualEditorSettings {
            recent_projects,
            elements_pane_height,
            outline_pane_height,
            left_pane_width,
            inspector_pane_width,
        } = value;
        Self {
            version: VisualEditorSettings::CURRENT_VERSION,
            recent_projects,
            elements_pane_height: positive_size(elements_pane_height),
            outline_pane_height: positive_size(outline_pane_height),
            left_pane_width: positive_size(left_pane_width),
            inspector_pane_width: positive_size(inspector_pane_width),
        }
    }
}

fn positive_size(size: Option<i32>) -> Option<i32> {
    size.filter(|size| *size > 0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn recent_project(root: impl Into<PathBuf>, url: Url, component: &str) -> Project {
        Project {
            root: root.into(),
            selected_canvas_preset: 0,
            preview: PreviewComponent { url, component: Some(component.into()) },
        }
    }

    #[test]
    fn settings_serialize_and_deserialize() {
        let settings = VisualEditorSettings {
            recent_projects: vec![recent_project(
                "/project",
                Url::parse("file:///project/main.slint").unwrap(),
                "MainWindow",
            )],
            elements_pane_height: None,
            outline_pane_height: None,
            left_pane_width: None,
            inspector_pane_width: None,
        };
        let serialized = settings.serialize();
        let json: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(json["recent_projects"][0]["preview"]["component"], "MainWindow");
        assert!(json["recent_projects"][0].get("path").is_none());
        assert_eq!(VisualEditorSettings::deserialize(&serialized), Some(settings));
    }

    #[test]
    fn settings_reject_malformed_and_mismatched_versions() {
        assert!(VisualEditorSettings::deserialize("not json").is_none());
        assert!(
            VisualEditorSettings::deserialize(r#"{"version":2,"recent_projects":[]}"#).is_none()
        );
    }

    #[test]
    fn pane_heights_round_trip_and_non_positive_values_are_unset() {
        let settings = VisualEditorSettings {
            recent_projects: Vec::new(),
            elements_pane_height: Some(320),
            outline_pane_height: Some(240),
            left_pane_width: Some(400),
            inspector_pane_width: Some(360),
        };
        let json = settings.serialize();
        assert_eq!(VisualEditorSettings::deserialize(&json), Some(settings));
        let unset = VisualEditorSettings::deserialize(
            r#"{"version":1,"recent_projects":[],"elements_pane_height":0,"outline_pane_height":-1,"left_pane_width":0,"inspector_pane_width":-1}"#,
        )
        .unwrap();
        assert_eq!(unset.elements_pane_height, None);
        assert_eq!(unset.outline_pane_height, None);
        assert_eq!(unset.left_pane_width, None);
        assert_eq!(unset.inspector_pane_width, None);
    }

    #[test]
    fn older_settings_default_pane_heights() {
        let settings =
            VisualEditorSettings::deserialize(r#"{"version":1,"recent_projects":[]}"#).unwrap();
        assert_eq!(settings.elements_pane_height, None);
        assert_eq!(settings.outline_pane_height, None);
        assert_eq!(settings.left_pane_width, None);
        assert_eq!(settings.inspector_pane_width, None);
    }

    #[test]
    fn recent_projects_are_deduplicated_reordered_and_truncated() {
        let mut settings = VisualEditorSettings::default();
        for index in 0..5 {
            assert!(settings.add_recent_project(recent_project(
                format!("/project/{index}"),
                Url::parse(&format!("file:///project/{index}.slint")).unwrap(),
                &format!("Component{index}"),
            )));
        }
        assert_eq!(settings.recent_projects.len(), MAX_RECENT_PROJECTS);
        assert_eq!(settings.recent_projects[0].preview.component.as_deref(), Some("Component4"));
        assert_eq!(settings.recent_projects[3].preview.component.as_deref(), Some("Component1"));

        let existing = settings.recent_projects[2].clone();
        assert!(settings.add_recent_project(existing.clone()));
        assert_eq!(settings.recent_projects[0], existing);
        assert!(!settings.add_recent_project(settings.recent_projects[0].clone()));

        assert!(settings.add_recent_project(recent_project(
            "/project/2",
            Url::parse("file:///project/2/other.slint").unwrap(),
            "Other",
        )));
        assert_eq!(settings.recent_projects[0].root, Path::new("/project/2"));
        assert_eq!(settings.recent_projects[0].preview.component.as_deref(), Some("Other"));
    }

    #[test]
    fn clear_recent_projects_reports_whether_anything_changed() {
        let mut settings = VisualEditorSettings::default();
        assert!(!settings.clear_recent_projects());
        assert!(settings.add_recent_project(recent_project(
            "/project",
            Url::parse("file:///project/main.slint").unwrap(),
            "Main",
        )));
        assert!(settings.clear_recent_projects());
        assert!(settings.recent_projects.is_empty());
        assert!(!settings.clear_recent_projects());
    }

    #[test]
    fn selected_canvas_preset_survives_preview_updates_and_older_settings() {
        let root = Path::new("/project");
        let mut settings = VisualEditorSettings::default();
        settings.add_recent_project(recent_project(
            root,
            Url::parse("file:///project/main.slint").unwrap(),
            "Main",
        ));
        assert!(settings.set_selected_canvas_preset(root, 1));
        settings.add_recent_project(recent_project(
            root,
            Url::parse("file:///project/other.slint").unwrap(),
            "Other",
        ));
        assert_eq!(settings.selected_canvas_preset(root), 1);
        let serialized = settings.serialize();
        assert_eq!(
            VisualEditorSettings::deserialize(&serialized).unwrap().selected_canvas_preset(root),
            1
        );
        let mut old: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        old["recent_projects"][0].as_object_mut().unwrap().remove("selected_canvas_preset");
        assert_eq!(
            VisualEditorSettings::deserialize(&old.to_string())
                .unwrap()
                .selected_canvas_preset(root),
            0
        );
        assert!(settings.clear_recent_projects());
        assert_eq!(settings.selected_canvas_preset(root), 0);
    }

    #[test]
    fn missing_recent_projects_stay_stored_but_are_not_visible() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("visible.slint");
        fs::write(&path, "").unwrap();
        let settings = VisualEditorSettings {
            recent_projects: vec![
                recent_project(directory.path(), Url::from_file_path(&path).unwrap(), "Visible"),
                recent_project(
                    "/missing",
                    Url::parse("file:///missing/project.slint").unwrap(),
                    "Missing",
                ),
            ],
            elements_pane_height: None,
            outline_pane_height: None,
            left_pane_width: None,
            inspector_pane_width: None,
        };

        let visible = settings.visible_recent_projects();
        assert_eq!(settings.recent_projects.len(), 2);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].component, "Visible");
    }

    fn project_file() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("ui")).unwrap();
        let path = directory.path().join("ui/main.slint");
        fs::write(&path, "export component MainWindow inherits Window {}").unwrap();
        (directory, path)
    }

    #[test]
    fn project_from_file_uses_parent_as_root() {
        let (directory, path) = project_file();
        let expected_url = Url::from_file_path(std::fs::canonicalize(&path).unwrap()).unwrap();

        let project = Project::from_input(&path, Some("MainWindow".into()), None).unwrap();

        assert_eq!(project.root, std::fs::canonicalize(directory.path().join("ui")).unwrap());
        assert_eq!(project.preview.url, expected_url);
        assert_eq!(project.preview.component.as_deref(), Some("MainWindow"));
    }

    #[test]
    fn project_from_root_keeps_explicit_root() {
        let (directory, path) = project_file();
        let expected_url = Url::from_file_path(std::fs::canonicalize(&path).unwrap()).unwrap();

        let project = Project::from_root(directory.path(), &path, None).unwrap();

        assert_eq!(project.root, std::fs::canonicalize(directory.path()).unwrap());
        assert_eq!(project.preview.url, expected_url);
    }
    #[test]
    fn project_inputs_use_only_the_selected_root() {
        let directory = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(directory.path()).unwrap();
        let nested = root.join("ui");
        std::fs::create_dir(&nested).unwrap();
        let manifest = root.join(i_slint_compiler::project_file::FILE_NAME);
        std::fs::write(&manifest, r#"{"slint-edition":"1.19","main":"ui/missing.slint"}"#).unwrap();
        let source = nested.join("missing.slint");
        let standalone = Project::from_input(&source, None, None).unwrap();
        assert_eq!(standalone.root, nested);
        assert_eq!(standalone.preview.url, Url::from_file_path(&source).unwrap());
        let explicit = Project::from_input(&source, None, Some(&root)).unwrap();
        assert_eq!(explicit.root, root);
        let directory = Project::from_input(&root, None, None).unwrap();
        assert_eq!(directory.preview.url, Url::from_file_path(&source).unwrap());
        assert!(Project::from_input(&root, None, Some(&nested)).is_err());
        assert!(Project::from_input(&manifest, None, None).is_err());
        let missing_root = root.join("unavailable");
        let path = missing_root.join("main.slint");
        assert_eq!(Project::from_root(&missing_root, &path, None).unwrap().root, missing_root);
        assert!(Project::from_input(root.join("other.json"), None, None).is_err());
    }

    #[test]
    fn selecting_a_project_file_opens_its_directory_and_main_file() {
        let directory = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(directory.path()).unwrap();
        let source = root.join("ui/main.slint");
        std::fs::create_dir(source.parent().unwrap()).unwrap();
        std::fs::write(&source, "export component Main inherits Window {}").unwrap();
        let manifest = root.join(i_slint_compiler::project_file::FILE_NAME);
        std::fs::write(&manifest, r#"{"slint-edition":"1.19","main":"ui/main.slint"}"#).unwrap();

        let project = Project::from_selected_file(&manifest, None).unwrap();
        assert_eq!(project.root, root);
        assert_eq!(project.preview.url, Url::from_file_path(&source).unwrap());
        assert!(Project::from_input(&manifest, None, None).is_err());

        std::fs::write(&manifest, r#"{"slint-edition":"1.19"}"#).unwrap();
        assert!(Project::from_selected_file(&manifest, None).is_err());
        assert!(Project::from_selected_file(root.join("other.json"), None).is_err());
    }
}
