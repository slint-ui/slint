// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::Path;

use super::ui;

pub(crate) const VISUAL_EDITOR_SETTINGS_FILE: &str = "visual-editor-user-settings.json";
const MAX_RECENT_PROJECTS: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct StoredRecentProject {
    pub root: String,
    pub path: String,
    pub component: String,
}

#[derive(Default, Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "VisualEditorSettingsSerde", into = "VisualEditorSettingsSerde")]
pub(crate) struct VisualEditorSettings {
    recent_projects: Vec<StoredRecentProject>,
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

    pub(crate) fn add_recent_project(&mut self, project: StoredRecentProject) -> bool {
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
                let root_path = Path::new(&project.root);
                // TODO: This should use the name configured in the project file.
                let project_name = root_path.file_name();
                if let Some(project_name) = project_name
                    && root_path.is_dir()
                    && Path::new(&project.path).is_file()
                {
                    Some((slint::format!("{}", project_name.display()), project))
                } else {
                    None
                }
            })
            .map(|(name, project)| ui::RecentProject {
                name,
                root_path: project.root.clone().into(),
                component: project.component.clone().into(),
                path: project.path.clone().into(),
            })
            .collect()
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
    recent_projects: Vec<StoredRecentProject>,
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

    fn recent_project(root: &str, path: &str, component: &str) -> StoredRecentProject {
        StoredRecentProject { root: root.into(), path: path.into(), component: component.into() }
    }

    #[test]
    fn settings_serialize_and_deserialize() {
        let settings = VisualEditorSettings {
            recent_projects: vec![recent_project("/project", "/project/main.slint", "MainWindow")],
        };

        assert_eq!(VisualEditorSettings::deserialize(&settings.serialize()), Some(settings));
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
                &format!("/project/{index}"),
                &format!("/project/{index}.slint"),
                &format!("Component{index}"),
            )));
        }
        assert_eq!(settings.recent_projects.len(), MAX_RECENT_PROJECTS);
        assert_eq!(settings.recent_projects[0].component, "Component4");
        assert_eq!(settings.recent_projects[3].component, "Component1");

        let existing = settings.recent_projects[2].clone();
        assert!(settings.add_recent_project(existing.clone()));
        assert_eq!(settings.recent_projects[0], existing);
        assert!(!settings.add_recent_project(settings.recent_projects[0].clone()));

        assert!(settings.add_recent_project(recent_project(
            "/project/2",
            "/project/2/other.slint",
            "Other",
        )));
        assert_eq!(settings.recent_projects[0].root, "/project/2");
        assert_eq!(settings.recent_projects[0].component, "Other");
    }

    #[test]
    fn missing_recent_projects_stay_stored_but_are_not_visible() {
        let root = std::env::temp_dir()
            .join(format!("slint-visual-editor-settings-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("visible.slint");
        fs::write(&path, "").unwrap();
        let settings = VisualEditorSettings {
            recent_projects: vec![
                recent_project(&root.to_string_lossy(), &path.to_string_lossy(), "Visible"),
                recent_project("/missing", "/missing/project.slint", "Missing"),
            ],
        };

        let visible = settings.visible_recent_projects();
        assert_eq!(settings.recent_projects.len(), 2);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].component, "Visible");

        let _ = fs::remove_dir_all(root);
    }
}
