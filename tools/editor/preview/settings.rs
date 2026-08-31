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
}

#[derive(Default, Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "VisualEditorSettingsSerde", into = "VisualEditorSettingsSerde")]
pub(crate) struct VisualEditorSettings {
    recent_projects: Vec<Project>,
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

    pub(crate) fn add_recent_project(&mut self, project: Project) -> bool {
        if self.recent_projects.first() == Some(&project) {
            return false;
        }
        self.recent_projects.retain(|recent| recent.root != project.root);
        self.recent_projects.insert(0, project);
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        true
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
    pub(crate) fn from_file(
        path: impl AsRef<Path>,
        component: Option<String>,
    ) -> i_slint_editor_preview::Result<Self> {
        let path = std::fs::canonicalize(path.as_ref())?;
        let root = path
            .parent()
            .ok_or_else(|| format!("Failed to determine project root for {}", path.display()))?;
        Self::from_root(root, &path, component)
    }

    pub(crate) fn from_root(
        root: &Path,
        path: &Path,
        component: Option<String>,
    ) -> i_slint_editor_preview::Result<Self> {
        let root = std::fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(format!("{} is not a directory", root.display()).into());
        }
        let path = std::fs::canonicalize(path)?;
        if !path.is_file()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("slint"))
        {
            return Err(format!("{} is not a Slint file", path.display()).into());
        }
        let url = Url::from_file_path(&path)
            .map_err(|_| format!("Failed to convert {} to URL", path.display()))?;
        Ok(Self { root, preview: PreviewComponent { url, component } })
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
        Ok(Self { recent_projects })
    }
}

impl From<VisualEditorSettings> for VisualEditorSettingsSerde {
    fn from(value: VisualEditorSettings) -> Self {
        let VisualEditorSettings { recent_projects } = value;
        Self { version: VisualEditorSettings::CURRENT_VERSION, recent_projects }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn recent_project(root: impl Into<PathBuf>, url: Url, component: &str) -> Project {
        Project {
            root: root.into(),
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

        let project = Project::from_file(&path, Some("MainWindow".into())).unwrap();

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
}
