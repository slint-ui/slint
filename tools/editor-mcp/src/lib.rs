// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SNAPSHOT_VERSION: u32 = 1;
const SNAPSHOT_DIRECTORY_NAME: &str = "slint-visual-editor-comments";
static PUBLISHER_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A comment attached to an element in a Slint source file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EditorComment {
    /// The comment identifier within its editor instance.
    pub id: String,
    /// The user's comment text.
    pub text: String,
    /// The absolute path of the Slint source file.
    pub file: PathBuf,
    /// The source range of the selected element.
    pub range: SourceRange,
    /// The component containing the selected element.
    pub component: Option<String>,
    /// The selected element's type.
    pub element_type: String,
    /// The selected element's ID, when it has one.
    pub element_id: Option<String>,
}

/// A zero-based source range.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceRange {
    /// The first position in the range.
    pub start: SourcePosition,
    /// The position immediately after the range.
    pub end: SourcePosition,
}

/// A zero-based position in a source file.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourcePosition {
    /// The zero-based line number.
    pub line: u32,
    /// The zero-based UTF-16 character offset.
    pub character: u32,
}

/// Publishes one visual editor instance's current comments.
pub struct SnapshotPublisher {
    snapshot_path: PathBuf,
    staged_path: PathBuf,
    editor_id: String,
}

impl SnapshotPublisher {
    /// Creates a publisher in the shared snapshot directory.
    pub fn new() -> io::Result<Self> {
        Self::in_directory(snapshot_directory())
    }

    /// Replaces this editor instance's published comments.
    pub fn publish(&self, project_root: &Path, comments: &[EditorComment]) -> io::Result<()> {
        if comments.is_empty() {
            return self.remove();
        }
        let snapshot = EditorSnapshot {
            version: SNAPSHOT_VERSION,
            editor_id: self.editor_id.clone(),
            project_root: project_root.to_path_buf(),
            comments: comments.to_vec(),
        };
        let serialized = serde_json::to_vec(&snapshot).map_err(io::Error::other)?;
        fs::write(&self.staged_path, serialized)?;
        replace_file(&self.staged_path, &self.snapshot_path)
    }

    /// Removes this editor instance's published comments.
    pub fn remove(&self) -> io::Result<()> {
        remove_if_present(&self.snapshot_path)
    }

    fn in_directory(directory: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
        let editor_id = next_editor_id();
        Ok(Self {
            snapshot_path: directory.join(format!("{editor_id}.json")),
            staged_path: directory.join(format!("{editor_id}.tmp")),
            editor_id,
        })
    }
}

impl Drop for SnapshotPublisher {
    fn drop(&mut self) {
        let _ = remove_if_present(&self.snapshot_path);
        let _ = remove_if_present(&self.staged_path);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// All comments published by running editors for one project.
pub struct ProjectComments {
    /// The shared project root.
    pub project_root: PathBuf,
    /// Comments from every running editor for this project.
    pub comments: Vec<EditorComment>,
}

#[derive(Deserialize, Serialize)]
struct EditorSnapshot {
    version: u32,
    editor_id: String,
    project_root: PathBuf,
    comments: Vec<EditorComment>,
}

/// Reads and groups every currently published snapshot.
pub fn scan_projects() -> io::Result<Vec<ProjectComments>> {
    scan_projects_in(&snapshot_directory())
}

/// Returns the MCP resource URI for a project's comments.
pub fn project_resource_uri(project_root: &Path) -> String {
    let encoded_path = project_root
        .to_string_lossy()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("slint-visual-editor://project/{encoded_path}/comments")
}

fn snapshot_directory() -> PathBuf {
    std::env::temp_dir().join(SNAPSHOT_DIRECTORY_NAME)
}

fn next_editor_id() -> String {
    let process_id = std::process::id();
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let sequence = PUBLISHER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{process_id}-{timestamp}-{sequence}")
}

fn replace_file(staged_path: &Path, snapshot_path: &Path) -> io::Result<()> {
    match fs::rename(staged_path, snapshot_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            remove_if_present(snapshot_path)?;
            fs::rename(staged_path, snapshot_path)
        }
        Err(error) => Err(error),
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn scan_projects_in(directory: &Path) -> io::Result<Vec<ProjectComments>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let snapshots = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "json"))
        .filter_map(|entry| fs::read(entry.path()).ok())
        .filter_map(|contents| serde_json::from_slice::<EditorSnapshot>(&contents).ok())
        .filter(|snapshot| snapshot.version == SNAPSHOT_VERSION);
    let mut projects = BTreeMap::<PathBuf, Vec<EditorComment>>::new();
    for snapshot in snapshots {
        projects.entry(snapshot.project_root).or_default().extend(snapshot.comments);
    }
    Ok(projects
        .into_iter()
        .map(|(project_root, comments)| ProjectComments { project_root, comments })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comment(id: &str, file: &str) -> EditorComment {
        EditorComment {
            id: id.into(),
            text: format!("Comment {id}"),
            file: file.into(),
            range: SourceRange {
                start: SourcePosition { line: 2, character: 4 },
                end: SourcePosition { line: 3, character: 8 },
            },
            component: Some("MainWindow".into()),
            element_type: "Rectangle".into(),
            element_id: Some(format!("element-{id}")),
        }
    }

    #[test]
    fn publishes_replaces_and_removes_a_snapshot() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let publisher = SnapshotPublisher::in_directory(temporary_directory.path().into()).unwrap();
        let project_root = Path::new("/projects/one");

        publisher.publish(project_root, &[comment("one", "/projects/one/main.slint")]).unwrap();
        let projects = scan_projects_in(temporary_directory.path()).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].comments[0].id, "one");

        publisher.publish(project_root, &[comment("two", "/projects/one/main.slint")]).unwrap();
        let projects = scan_projects_in(temporary_directory.path()).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].comments[0].id, "two");

        publisher.remove().unwrap();
        assert!(scan_projects_in(temporary_directory.path()).unwrap().is_empty());
    }

    #[test]
    fn publishing_no_comments_removes_the_snapshot() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let publisher = SnapshotPublisher::in_directory(temporary_directory.path().into()).unwrap();
        let project_root = Path::new("/projects/one");

        publisher.publish(project_root, &[]).unwrap();
        assert!(!publisher.snapshot_path.exists());

        publisher.publish(project_root, &[comment("one", "/projects/one/main.slint")]).unwrap();
        assert!(publisher.snapshot_path.exists());

        publisher.publish(project_root, &[]).unwrap();
        assert!(!publisher.snapshot_path.exists());
        assert!(scan_projects_in(temporary_directory.path()).unwrap().is_empty());
    }

    #[test]
    fn groups_snapshots_by_project() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let first = SnapshotPublisher::in_directory(temporary_directory.path().into()).unwrap();
        let second = SnapshotPublisher::in_directory(temporary_directory.path().into()).unwrap();
        let third = SnapshotPublisher::in_directory(temporary_directory.path().into()).unwrap();

        first
            .publish(Path::new("/projects/one"), &[comment("one", "/projects/one/one.slint")])
            .unwrap();
        second
            .publish(Path::new("/projects/one"), &[comment("two", "/projects/one/two.slint")])
            .unwrap();
        third
            .publish(Path::new("/projects/two"), &[comment("three", "/projects/two/main.slint")])
            .unwrap();

        let projects = scan_projects_in(temporary_directory.path()).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].comments.len(), 2);
        assert_eq!(projects[1].comments.len(), 1);
    }

    #[test]
    fn ignores_malformed_and_unknown_snapshots() {
        let temporary_directory = tempfile::tempdir().unwrap();
        fs::write(temporary_directory.path().join("malformed.json"), b"not json").unwrap();
        fs::write(
            temporary_directory.path().join("future.json"),
            br#"{"version":2,"editor_id":"future","project_root":"/future","comments":[]}"#,
        )
        .unwrap();

        assert!(scan_projects_in(temporary_directory.path()).unwrap().is_empty());
    }

    #[test]
    fn creates_stable_distinct_resource_uris() {
        let first = project_resource_uri(Path::new("/projects/one"));
        assert_eq!(first, project_resource_uri(Path::new("/projects/one")));
        assert_ne!(first, project_resource_uri(Path::new("/projects/two")));
        assert!(first.starts_with("slint-visual-editor://project/"));
    }
}
