// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use i_slint_core::platform::Clipboard;
use slint::{Image, ModelRc, SharedString, ToSharedString as _, VecModel};

use super::{Api, EditorSurfaceMode, FileTreeNode, FileTreeNodeKind, ImageAssetPreview};

pub fn setup(api: &Api<'_>, api_weak: slint::Weak<Api<'static>>, use_editor_ui: bool) {
    if !use_editor_ui {
        api.set_file_tree(Default::default());
        api.set_selected_project_file(Default::default());
        return;
    };

    let controller = Rc::new(RefCell::new(
        initial_file_tree_paths()
            .map(|(root, selected_path)| FileTreeController::new(root, selected_path)),
    ));
    if let Some(controller) = controller.borrow().as_ref() {
        controller.publish(api);
    } else {
        api.set_file_tree(Default::default());
        api.set_selected_project_file(Default::default());
    }

    let controller_for_select = controller.clone();
    let api_weak_for_select = api_weak.clone();
    api.on_file_tree_select(move |path| {
        if let Some(api) = api_weak_for_select.upgrade() {
            if let Some(controller) = controller_for_select.borrow_mut().as_mut() {
                controller.select(Path::new(path.as_str()), &api);
            }
        }
    });

    let controller_for_open = controller.clone();
    let api_weak_for_open = api_weak.clone();
    api.on_open_existing_project(move || {
        let Some(path) = choose_project_file() else {
            return false;
        };
        let Some(root) = path.parent().map(Path::to_path_buf) else {
            return false;
        };
        if let Some(api) = api_weak_for_open.upgrade() {
            let mut controller = controller_for_open.borrow_mut();
            *controller = Some(FileTreeController::new(root, Some(path.clone())));
            if let Some(controller) = controller.as_mut() {
                controller.publish(&api);
            }
            super::super::request_file_tree_preview(&path);
            true
        } else {
            false
        }
    });

    api.on_file_tree_toggle(move |path| {
        if let Some(api) = api_weak.upgrade() {
            if let Some(controller) = controller.borrow_mut().as_mut() {
                controller.toggle(Path::new(path.as_str()), &api);
            }
        }
    });

    api.on_image_nine_slice_expression(format_nine_slice_expression);
    api.on_image_nine_slice_preview(nine_slice_preview_image);

    api.on_copy_nine_slice_expression(|value| {
        if let Err(err) = i_slint_backend_selector::with_platform(|platform| {
            platform.set_clipboard_text(value.as_str(), Clipboard::DefaultClipboard);
            Ok(())
        }) {
            tracing::warn!("Failed to copy nine-slice expression to clipboard: {err}");
        }
    });
}

fn initial_file_tree_paths() -> Option<(PathBuf, Option<PathBuf>)> {
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

#[cfg(target_os = "macos")]
fn choose_project_file() -> Option<PathBuf> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::NSString;

    let mtm = MainThreadMarker::new()?;
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setTitle(Some(&NSString::from_str("Open Slint File")));
    panel.setMessage(Some(&NSString::from_str("Choose a .slint file to edit.")));

    if panel.runModal() != NSModalResponseOK {
        return None;
    }

    panel.URLs().firstObject()?.to_file_path()
}

#[cfg(all(not(target_os = "macos"), not(target_arch = "wasm32")))]
fn choose_project_file() -> Option<PathBuf> {
    None
}

#[cfg(target_arch = "wasm32")]
fn choose_project_file() -> Option<PathBuf> {
    None
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
            api.set_editor_surface_mode(EditorSurfaceMode::Component);
            super::super::request_file_tree_preview(&path);
        } else if is_image_file(&path) {
            api.set_selected_image_asset(load_image_asset_preview(&self.root, &path));
            api.set_image_nine_slice_top(0);
            api.set_image_nine_slice_right(0);
            api.set_image_nine_slice_bottom(0);
            api.set_image_nine_slice_left(0);
            api.set_editor_surface_mode(EditorSurfaceMode::Image);
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
        api.set_selected_project_file(
            selected_project_file(&self.root, self.selected_path.as_deref()).into(),
        );
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

fn is_image_file(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| {
        matches!(extension.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "svg")
    })
}

fn load_image_asset_preview(root: &Path, path: &Path) -> ImageAssetPreview {
    let relative_path = project_relative_path(root, path);
    let format = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_uppercase())
        .unwrap_or_default();
    let path_string = path_to_shared_string(path);

    match Image::load_from_path(path) {
        Ok(image) => ImageAssetPreview {
            path: path_string,
            relative_path: relative_path.into(),
            format: format.into(),
            image,
            error: SharedString::default(),
        },
        Err(_) => ImageAssetPreview {
            path: path_string,
            relative_path: relative_path.into(),
            format: format.into(),
            image: Image::default(),
            error: "Failed to load image".into(),
        },
    }
}

fn project_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn selected_project_file(root: &Path, selected_path: Option<&Path>) -> String {
    let Some(path) = selected_path else { return String::new() };
    if !is_slint_file(path) && !is_image_file(path) {
        return String::new();
    }
    project_relative_path(root, path)
}

fn format_nine_slice_expression(
    path: SharedString,
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
) -> SharedString {
    format!(
        "@image-url(\"{}\", nine-slice({} {} {} {}))",
        escape_slint_string(path.as_str()),
        top.max(0),
        right.max(0),
        bottom.max(0),
        left.max(0)
    )
    .into()
}

fn escape_slint_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn nine_slice_preview_image(source: Image, top: i32, right: i32, bottom: i32, left: i32) -> Image {
    let mut image = source;
    image.set_nine_slice_edges(
        nine_slice_edge_to_u16(top),
        nine_slice_edge_to_u16(right),
        nine_slice_edge_to_u16(bottom),
        nine_slice_edge_to_u16(left),
    );
    image
}

fn nine_slice_edge_to_u16(value: i32) -> u16 {
    value.clamp(0, u16::MAX as i32) as u16
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

    if is_image_file(path) { FileTreeNodeKind::Image } else { FileTreeNodeKind::File }
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
    fn image_asset_preview_loads_metadata() {
        let tree = TempTree::new();
        let path = tree.file("assets/panel.svg");
        fs::write(&path, r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="7"></svg>"#)
            .unwrap();

        let preview = load_image_asset_preview(&tree.root, &path);

        assert_eq!(preview.path, path_to_shared_string(&path));
        assert_eq!(preview.relative_path, "assets/panel.svg");
        assert_eq!(preview.format, "SVG");
        assert_eq!(preview.error, "");
        assert_eq!(preview.image.size().width, 12);
        assert_eq!(preview.image.size().height, 7);
    }

    #[test]
    fn image_asset_preview_reports_load_errors_without_stale_image() {
        let tree = TempTree::new();
        let path = tree.file("broken.png");

        let preview = load_image_asset_preview(&tree.root, &path);

        assert_eq!(preview.relative_path, "broken.png");
        assert_eq!(preview.format, "PNG");
        assert_eq!(preview.error, "Failed to load image");
        assert_eq!(preview.image.size().width, 0);
        assert_eq!(preview.image.size().height, 0);
    }

    #[test]
    fn selected_project_file_is_project_relative_for_openable_files() {
        let tree = TempTree::new();
        let slint = tree.file("ui/main.slint");
        let image = tree.file("assets/title-frame.png");

        assert_eq!(selected_project_file(&tree.root, Some(&slint)), "ui/main.slint");
        assert_eq!(selected_project_file(&tree.root, Some(&image)), "assets/title-frame.png");
    }

    #[test]
    fn selected_project_file_is_blank_without_openable_file() {
        let tree = TempTree::new();
        let folder = tree.dir("ui");
        let unsupported = tree.file("notes.txt");

        assert_eq!(selected_project_file(&tree.root, None), "");
        assert_eq!(selected_project_file(&tree.root, Some(&folder)), "");
        assert_eq!(selected_project_file(&tree.root, Some(&unsupported)), "");
    }

    #[test]
    fn nine_slice_expression_uses_slint_order_and_escapes_path() {
        let expression =
            format_nine_slice_expression("icons/quote\"slash\\tab\t.png".into(), 1, 2, 3, 4);

        assert_eq!(
            expression,
            "@image-url(\"icons/quote\\\"slash\\\\tab\\t.png\", nine-slice(1 2 3 4))"
        );
    }

    #[test]
    fn nine_slice_expression_clamps_negative_values() {
        let expression = format_nine_slice_expression("panel.png".into(), -1, 2, -3, 4);

        assert_eq!(expression, "@image-url(\"panel.png\", nine-slice(0 2 0 4))");
    }

    #[test]
    fn nine_slice_preview_image_preserves_image_size() {
        let tree = TempTree::new();
        let path = tree.file("panel.svg");
        fs::write(
            &path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="10"></svg>"#,
        )
        .unwrap();
        let source = Image::load_from_path(&path).unwrap();

        let preview = nine_slice_preview_image(source, 1, 2, 3, 4);

        assert_eq!(preview.size().width, 16);
        assert_eq!(preview.size().height, 10);
    }

    #[test]
    fn nine_slice_edges_clamp_to_u16() {
        assert_eq!(nine_slice_edge_to_u16(-1), 0);
        assert_eq!(nine_slice_edge_to_u16(12), 12);
        assert_eq!(nine_slice_edge_to_u16(i32::MAX), u16::MAX);
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
