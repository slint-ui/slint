// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

/// The file name of a Slint project manifest.
pub const PROJECT_MANIFEST_FILE: &str = "slint.toml";

/// The project entry component used by the Visual Editor's Run action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRunTarget {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub entry_file: PathBuf,
    pub component: String,
}

/// An error while reading or creating a Slint project manifest.
#[derive(Debug)]
pub struct ProjectManifestError {
    message: String,
}

impl ProjectManifestError {
    fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl std::fmt::Display for ProjectManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for ProjectManifestError {}

/// Find the nearest project manifest at or above `path`.
pub fn find_project_manifest(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() { path } else { path.parent()? };
    start
        .ancestors()
        .map(|directory| directory.join(PROJECT_MANIFEST_FILE))
        .find(|manifest| manifest.is_file())
}

/// Return the manifest directory nearest to `path`, or the containing directory when none exists.
pub fn project_root_for_path(path: &Path) -> Option<PathBuf> {
    find_project_manifest(path)
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
        .or_else(|| {
            if path.is_dir() {
                Some(path.to_path_buf())
            } else {
                path.parent().map(Path::to_path_buf)
            }
        })
}

/// Load the run target from `project_root/slint.toml`.
///
/// Returns `Ok(None)` when the manifest doesn't exist.
pub fn load_project_run_target(
    project_root: &Path,
) -> Result<Option<ProjectRunTarget>, ProjectManifestError> {
    let project_root = canonicalize(project_root, "project directory")?;
    let manifest_path = project_root.join(PROJECT_MANIFEST_FILE);
    if !manifest_path.exists() {
        return Ok(None);
    }
    if !manifest_path.is_file() {
        return Err(ProjectManifestError::new(format!(
            "The project manifest {} is not a file",
            manifest_path.display()
        )));
    }

    let contents = std::fs::read_to_string(&manifest_path).map_err(|error| {
        ProjectManifestError::new(format!(
            "Failed to read project manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let document = contents.parse::<toml_edit::DocumentMut>().map_err(|error| {
        ProjectManifestError::new(format!(
            "Failed to parse project manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let entry = required_string(&document, "entry", &manifest_path)?;
    let component = required_string(&document, "component", &manifest_path)?;
    if component.trim().is_empty() {
        return Err(ProjectManifestError::new(format!(
            "Project manifest {} has an empty `component`",
            manifest_path.display()
        )));
    }

    let entry_path = Path::new(entry);
    validate_relative_entry(entry_path, &manifest_path)?;
    let entry_file = canonicalize(&project_root.join(entry_path), "project entry file")?;
    validate_entry_file(&project_root, &entry_file, &manifest_path)?;

    Ok(Some(ProjectRunTarget {
        project_root,
        manifest_path,
        entry_file,
        component: component.to_string(),
    }))
}

/// Create `slint.toml` for an entry file contained by `project_root`.
pub fn create_project_manifest(
    project_root: &Path,
    entry_file: &Path,
    component: &str,
) -> Result<ProjectRunTarget, ProjectManifestError> {
    let project_root = canonicalize(project_root, "project directory")?;
    let entry_file = canonicalize(entry_file, "project entry file")?;
    let manifest_path = project_root.join(PROJECT_MANIFEST_FILE);
    validate_entry_file(&project_root, &entry_file, &manifest_path)?;
    if component.trim().is_empty() {
        return Err(ProjectManifestError::new("The project component name is empty"));
    }

    let relative_entry = entry_file.strip_prefix(&project_root).map_err(|_| {
        ProjectManifestError::new(format!(
            "Project entry file {} is outside {}",
            entry_file.display(),
            project_root.display()
        ))
    })?;
    let relative_entry = relative_entry.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");

    let mut document = toml_edit::DocumentMut::new();
    document["entry"] = toml_edit::value(relative_entry);
    document["component"] = toml_edit::value(component);

    let mut file =
        std::fs::OpenOptions::new().write(true).create_new(true).open(&manifest_path).map_err(
            |error| {
                ProjectManifestError::new(format!(
                    "Failed to create project manifest {}: {error}",
                    manifest_path.display()
                ))
            },
        )?;
    file.write_all(document.to_string().as_bytes()).map_err(|error| {
        ProjectManifestError::new(format!(
            "Failed to write project manifest {}: {error}",
            manifest_path.display()
        ))
    })?;

    Ok(ProjectRunTarget {
        project_root,
        manifest_path,
        entry_file,
        component: component.to_string(),
    })
}

fn required_string<'a>(
    document: &'a toml_edit::DocumentMut,
    key: &str,
    manifest_path: &Path,
) -> Result<&'a str, ProjectManifestError> {
    document.get(key).and_then(toml_edit::Item::as_str).ok_or_else(|| {
        ProjectManifestError::new(format!(
            "Project manifest {} requires a string `{key}`",
            manifest_path.display()
        ))
    })
}

fn canonicalize(path: &Path, description: &str) -> Result<PathBuf, ProjectManifestError> {
    std::fs::canonicalize(path).map_err(|error| {
        ProjectManifestError::new(format!(
            "Failed to resolve {description} {}: {error}",
            path.display()
        ))
    })
}

fn validate_relative_entry(entry: &Path, manifest_path: &Path) -> Result<(), ProjectManifestError> {
    if entry.is_absolute()
        || entry.components().any(|component| {
            matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))
        })
    {
        return Err(ProjectManifestError::new(format!(
            "Project manifest {} requires `entry` to stay within the project directory",
            manifest_path.display()
        )));
    }
    Ok(())
}

fn validate_entry_file(
    project_root: &Path,
    entry_file: &Path,
    manifest_path: &Path,
) -> Result<(), ProjectManifestError> {
    if !entry_file.starts_with(project_root) {
        return Err(ProjectManifestError::new(format!(
            "Project manifest {} points outside the project directory",
            manifest_path.display()
        )));
    }
    if !entry_file.is_file() {
        return Err(ProjectManifestError::new(format!(
            "Project entry {} is not a file",
            entry_file.display()
        )));
    }
    if !entry_file
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("slint"))
    {
        return Err(ProjectManifestError::new(format!(
            "Project entry {} is not a .slint file",
            entry_file.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn discovers_and_loads_nearest_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("ui/components/main window.slint");
        write(&entry, "export component App inherits Window {}\n");
        write(
            &directory.path().join(PROJECT_MANIFEST_FILE),
            "entry = \"ui/components/main window.slint\"\ncomponent = \"App\"\n",
        );

        assert_eq!(
            find_project_manifest(&entry),
            Some(directory.path().join(PROJECT_MANIFEST_FILE))
        );
        let target = load_project_run_target(directory.path()).unwrap().unwrap();
        assert_eq!(target.entry_file, std::fs::canonicalize(entry).unwrap());
        assert_eq!(target.component, "App");
    }

    #[test]
    fn missing_manifest_has_no_run_target() {
        let directory = tempfile::tempdir().unwrap();

        assert_eq!(load_project_run_target(directory.path()).unwrap(), None);
    }

    #[test]
    fn rejects_malformed_and_incomplete_manifests() {
        for (contents, expected) in [
            ("not toml", "Failed to parse project manifest"),
            ("entry = \"main.slint\"\n", "requires a string `component`"),
            ("component = \"App\"\n", "requires a string `entry`"),
        ] {
            let directory = tempfile::tempdir().unwrap();
            write(&directory.path().join(PROJECT_MANIFEST_FILE), contents);

            let error = load_project_run_target(directory.path()).unwrap_err().to_string();
            assert!(error.contains(expected), "unexpected error: {error}");
        }
    }

    #[test]
    fn rejects_entries_outside_the_project() {
        let parent = tempfile::tempdir().unwrap();
        let project = parent.path().join("project");
        std::fs::create_dir(&project).unwrap();
        write(&parent.path().join("outside.slint"), "export component App {}\n");
        write(
            &project.join(PROJECT_MANIFEST_FILE),
            "entry = \"../outside.slint\"\ncomponent = \"App\"\n",
        );

        let error = load_project_run_target(&project).unwrap_err().to_string();
        assert!(error.contains("stay within the project directory"));
    }

    #[test]
    fn rejects_absolute_missing_and_non_slint_entries() {
        let outside = tempfile::NamedTempFile::new().unwrap();
        for (entry, expected) in [
            (outside.path().to_string_lossy().to_string(), "stay within the project directory"),
            ("missing.slint".into(), "Failed to resolve project entry file"),
            ("main.txt".into(), "is not a .slint file"),
        ] {
            let directory = tempfile::tempdir().unwrap();
            if entry == "main.txt" {
                write(&directory.path().join(&entry), "not Slint\n");
            }
            write(
                &directory.path().join(PROJECT_MANIFEST_FILE),
                &format!("entry = {entry:?}\ncomponent = \"App\"\n"),
            );

            let error = load_project_run_target(directory.path()).unwrap_err().to_string();
            assert!(error.contains(expected), "unexpected error: {error}");
        }
    }

    #[test]
    fn rejects_an_empty_component() {
        let directory = tempfile::tempdir().unwrap();
        write(&directory.path().join("main.slint"), "export component App {}\n");
        write(
            &directory.path().join(PROJECT_MANIFEST_FILE),
            "entry = \"main.slint\"\ncomponent = \"  \"\n",
        );

        let error = load_project_run_target(directory.path()).unwrap_err().to_string();

        assert!(error.contains("empty `component`"));
    }

    #[test]
    fn creates_a_round_trippable_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("ui/main window.slint");
        write(&entry, "export component MainWindow inherits Window {}\n");

        let created = create_project_manifest(directory.path(), &entry, "MainWindow").unwrap();
        let loaded = load_project_run_target(directory.path()).unwrap().unwrap();

        assert_eq!(loaded, created);
        assert_eq!(
            std::fs::read_to_string(directory.path().join(PROJECT_MANIFEST_FILE)).unwrap(),
            "entry = \"ui/main window.slint\"\ncomponent = \"MainWindow\"\n"
        );
    }

    #[test]
    fn does_not_replace_an_existing_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("main.slint");
        write(&entry, "export component App {}\n");
        write(&directory.path().join(PROJECT_MANIFEST_FILE), "existing = true\n");

        let error = create_project_manifest(directory.path(), &entry, "App").unwrap_err();

        assert!(error.to_string().contains("Failed to create project manifest"));
        assert_eq!(
            std::fs::read_to_string(directory.path().join(PROJECT_MANIFEST_FILE)).unwrap(),
            "existing = true\n"
        );
    }
}
