// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{ModelRc, SharedString, ToSharedString as _, VecModel};

use super::{Api, FileTreeNode, FileTreeNodeKind};

pub fn setup(api: &Api<'_>, api_weak: slint::Weak<Api<'static>>, use_editor_ui: bool) {
    let Some((root, selected_path)) = initial_file_tree_paths(use_editor_ui) else {
        api.set_file_tree(Default::default());
        return;
    };

    let controller = Rc::new(RefCell::new(FileTreeController::new(root, selected_path)));
    controller.borrow().publish(api);

    let controller_for_select = controller.clone();
    let api_weak_for_select = api_weak.clone();
    api.on_file_tree_select(move |path| {
        if let Some(api) = api_weak_for_select.upgrade() {
            controller_for_select.borrow_mut().select(Path::new(path.as_str()), &api);
        }
    });

    api.on_file_tree_toggle(move |path| {
        if let Some(api) = api_weak.upgrade() {
            controller.borrow_mut().toggle(Path::new(path.as_str()), &api);
        }
    });
}

fn initial_file_tree_paths(use_editor_ui: bool) -> Option<(PathBuf, Option<PathBuf>)> {
    if !use_editor_ui {
        return None;
    }

    #[cfg(target_arch = "wasm32")]
    {
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let selected_path = std::fs::canonicalize(std::env::args_os().nth(1)?).ok()?;
        let root = selected_path.parent()?.to_path_buf();
        Some((root, Some(selected_path)))
    }
}

struct FileTreeController {
    root: PathBuf,
    expanded: HashSet<PathBuf>,
    selected_path: Option<PathBuf>,
    active_folder_path: PathBuf,
}

impl FileTreeController {
    fn new(root: PathBuf, selected_path: Option<PathBuf>) -> Self {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let mut expanded = HashSet::new();
        expanded.insert(root.clone());

        let selected_path = selected_path
            .and_then(|path| std::fs::canonicalize(path).ok())
            .filter(|path| path.starts_with(&root));
        let active_folder_path = selected_path
            .as_deref()
            .and_then(active_folder_for_path)
            .unwrap_or(&root)
            .to_path_buf();

        Self { root, expanded, selected_path, active_folder_path }
    }

    fn select(&mut self, path: &Path, api: &Api<'_>) {
        let Some(path) = self.path_in_root(path) else {
            return;
        };
        let is_slint_file = is_slint_file(&path);

        self.selected_path = Some(path.clone());
        self.active_folder_path = active_folder_for_path(&path).unwrap_or(&self.root).to_path_buf();
        self.publish(api);

        if is_slint_file {
            super::super::request_file_tree_preview(&path);
        }
    }

    fn toggle(&mut self, path: &Path, api: &Api<'_>) {
        let Some(path) = self.path_in_root(path) else {
            return;
        };
        if !is_directory(&path) {
            return;
        }

        if self.expanded.contains(&path) {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path);
        }
        self.publish(api);
    }

    fn publish(&self, api: &Api<'_>) {
        let rows = build_file_tree_rows(
            &self.root,
            &self.expanded,
            self.selected_path.as_deref(),
            &self.active_folder_path,
        );
        api.set_file_tree(ModelRc::new(VecModel::from(rows)));
    }

    fn path_in_root(&self, path: &Path) -> Option<PathBuf> {
        let path = std::fs::canonicalize(path).ok()?;
        (path == self.root || path.starts_with(&self.root)).then_some(path)
    }
}

fn active_folder_for_path(path: &Path) -> Option<&Path> {
    match std::fs::symlink_metadata(path).ok()?.file_type() {
        file_type if file_type.is_dir() => Some(path),
        file_type if file_type.is_file() => path.parent(),
        _ => path.parent(),
    }
}

fn is_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
}

fn is_slint_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("slint"))
}

fn build_file_tree_rows(
    root: &Path,
    expanded: &HashSet<PathBuf>,
    selected_path: Option<&Path>,
    active_folder_path: &Path,
) -> Vec<FileTreeNode> {
    let mut rows = Vec::new();
    append_node(root, None, 0, expanded, selected_path, active_folder_path, &mut rows);
    rows
}

fn append_node(
    path: &Path,
    parent: Option<&Path>,
    indent_level: i32,
    expanded: &HashSet<PathBuf>,
    selected_path: Option<&Path>,
    active_folder_path: &Path,
    rows: &mut Vec<FileTreeNode>,
) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else { return };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return;
    }

    let is_folder = file_type.is_dir();
    let is_expanded = is_folder && expanded.contains(path);
    let children = if is_folder { read_directory_entries(path) } else { Vec::new() };
    rows.push(FileTreeNode {
        label: label_for_path(path),
        path: path_to_shared_string(path),
        parent_path: parent.map(path_to_shared_string).unwrap_or_default(),
        indent_level,
        has_children: !children.is_empty(),
        is_expanded,
        is_selected: selected_path.is_some_and(|selected| selected == path),
        is_slint_file: !is_folder && is_slint_file(path),
        show_selected_guide: parent.is_some_and(|parent| parent == active_folder_path),
        kind: file_tree_node_kind(path, is_folder),
    });

    if !is_expanded {
        return;
    }

    for child in children {
        append_node(
            &child.path,
            Some(path),
            indent_level + 1,
            expanded,
            selected_path,
            active_folder_path,
            rows,
        );
    }
}

fn read_directory_entries(path: &Path) -> Vec<DirectoryEntry> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    let mut entries = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            if file_type.is_symlink() {
                return None;
            }
            let kind = if file_type.is_dir() {
                DirectoryEntryKind::Folder
            } else if file_type.is_file() {
                DirectoryEntryKind::File
            } else {
                return None;
            };
            Some(DirectoryEntry { path: entry.path(), kind })
        })
        .collect::<Vec<_>>();

    entries.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| file_name_sort_key(&a.path).cmp(&file_name_sort_key(&b.path)))
            .then_with(|| a.path.cmp(&b.path))
    });
    entries
}

fn label_for_path(path: &Path) -> SharedString {
    path.file_name()
        .map(|name| name.to_string_lossy().to_shared_string())
        .unwrap_or_else(|| path.display().to_string().into())
}

fn path_to_shared_string(path: &Path) -> SharedString {
    path.to_string_lossy().to_shared_string()
}

fn file_name_sort_key(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn file_tree_node_kind(path: &Path, is_folder: bool) -> FileTreeNodeKind {
    if is_folder {
        return FileTreeNodeKind::Folder;
    }

    match path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase) {
        Some(extension) if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "svg") => {
            FileTreeNodeKind::Image
        }
        _ => FileTreeNodeKind::File,
    }
}

struct DirectoryEntry {
    path: PathBuf,
    kind: DirectoryEntryKind,
}

#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum DirectoryEntryKind {
    Folder,
    File,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new() -> Self {
            static TEMP_TREE_COUNTER: AtomicU64 = AtomicU64::new(0);
            let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let index = TEMP_TREE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join(format!("slint-file-tree-{}-{unique}-{index}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn dir(&self, path: &str) -> PathBuf {
            let path = self.root.join(path);
            fs::create_dir_all(&path).unwrap();
            path
        }

        fn file(&self, path: &str) -> PathBuf {
            let path = self.root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, "").unwrap();
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn labels(rows: &[FileTreeNode]) -> Vec<String> {
        rows.iter().map(|row| row.label.to_string()).collect()
    }

    #[test]
    fn root_row_appears() {
        let tree = TempTree::new();
        let rows =
            build_file_tree_rows(&tree.root, &HashSet::from([tree.root.clone()]), None, &tree.root);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, path_to_shared_string(&tree.root));
        assert_eq!(rows[0].kind, FileTreeNodeKind::Folder);
        assert!(rows[0].is_expanded);
    }

    #[test]
    fn directories_sort_before_files_case_insensitively() {
        let tree = TempTree::new();
        tree.file("z.slint");
        tree.dir("Beta");
        tree.file("a.slint");
        tree.dir("alpha");

        let rows =
            build_file_tree_rows(&tree.root, &HashSet::from([tree.root.clone()]), None, &tree.root);

        assert_eq!(labels(&rows)[1..], ["alpha", "Beta", "a.slint", "z.slint"]);
    }

    #[test]
    fn collapsed_directories_do_not_emit_children() {
        let tree = TempTree::new();
        let dir = tree.dir("components");
        tree.file("components/button.slint");

        let rows =
            build_file_tree_rows(&tree.root, &HashSet::from([tree.root.clone()]), None, &tree.root);

        assert_eq!(
            labels(&rows),
            vec![label_for_path(&tree.root).to_string(), "components".into()]
        );
        assert!(
            rows.iter().find(|row| row.path == path_to_shared_string(&dir)).unwrap().has_children
        );
    }

    #[test]
    fn expanded_directories_emit_direct_children() {
        let tree = TempTree::new();
        let dir = tree.dir("components");
        tree.file("components/button.slint");

        let rows = build_file_tree_rows(
            &tree.root,
            &HashSet::from([tree.root.clone(), dir]),
            None,
            &tree.root,
        );

        assert_eq!(
            labels(&rows),
            vec![
                label_for_path(&tree.root).to_string(),
                "components".into(),
                "button.slint".into()
            ]
        );
    }

    #[test]
    fn selected_file_marks_parent_branch_only() {
        let tree = TempTree::new();
        let components = tree.dir("components");
        let nested = tree.dir("components/nested");
        let selected = tree.file("components/button.slint");
        tree.file("components/nested/deeper.slint");

        let rows = build_file_tree_rows(
            &tree.root,
            &HashSet::from([tree.root.clone(), components.clone(), nested]),
            Some(&selected),
            &components,
        );

        let button = rows.iter().find(|row| row.label == "button.slint").unwrap();
        let nested = rows.iter().find(|row| row.label == "nested").unwrap();
        let deeper = rows.iter().find(|row| row.label == "deeper.slint").unwrap();
        assert!(button.show_selected_guide);
        assert!(nested.show_selected_guide);
        assert!(!deeper.show_selected_guide);
    }

    #[test]
    fn image_files_use_image_kind() {
        let tree = TempTree::new();
        tree.file("diagram.png");
        tree.file("photo.jpeg");
        tree.file("icon.svg");
        tree.file("source.slint");

        let rows =
            build_file_tree_rows(&tree.root, &HashSet::from([tree.root.clone()]), None, &tree.root);

        for label in ["diagram.png", "photo.jpeg", "icon.svg"] {
            assert_eq!(
                rows.iter().find(|row| row.label == label).unwrap().kind,
                FileTreeNodeKind::Image
            );
        }
        assert_eq!(
            rows.iter().find(|row| row.label == "source.slint").unwrap().kind,
            FileTreeNodeKind::File
        );
    }

    #[test]
    fn slint_files_are_marked_openable() {
        let tree = TempTree::new();
        tree.file("view.slint");
        tree.file("image.png");

        let rows =
            build_file_tree_rows(&tree.root, &HashSet::from([tree.root.clone()]), None, &tree.root);

        assert!(rows.iter().find(|row| row.label == "view.slint").unwrap().is_slint_file);
        assert!(!rows.iter().find(|row| row.label == "image.png").unwrap().is_slint_file);
    }

    #[test]
    fn callback_paths_must_remain_inside_root() {
        let tree = TempTree::new();
        let child = tree.file("inside.slint");
        let outside_tree = TempTree::new();
        let outside = outside_tree.file("outside.slint");
        let controller = FileTreeController::new(tree.root.clone(), None);
        let child = std::fs::canonicalize(child).unwrap();

        assert_eq!(controller.path_in_root(&child), Some(child));
        assert_eq!(controller.path_in_root(&outside), None);
    }

    #[test]
    fn toggle_targets_must_be_directories() {
        let tree = TempTree::new();
        let dir = tree.dir("components");
        let file = tree.file("components/button.slint");

        assert!(is_directory(&dir));
        assert!(!is_directory(&file));
    }
}
