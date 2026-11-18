// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("editor source directory is readable") {
        let entry = entry.expect("editor source entry is readable");
        if entry.file_type().expect("editor source entry has a file type").is_dir() {
            collect_files(&entry.path(), files);
        } else {
            files.push(entry.path());
        }
    }
}

fn editor_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for directory in ["ui", "controls", "gallery"] {
        collect_files(&root.join(directory), &mut files);
    }
    files.sort();
    files
}

fn is_asset_directory(path: &Path) -> bool {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    ["ui/icons", "ui/cursors", "ui/images"]
        .iter()
        .any(|directory| path.starts_with(root.join(directory)))
}

fn relative_path(path: &Path) -> &Path {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR")).expect("path belongs to the editor")
}

#[test]
fn slint_image_sources_are_centralized() {
    let mut violations = Vec::new();
    for path in editor_files() {
        if is_asset_directory(&path)
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("slint"))
        {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("Slint source is readable");
        for (line, text) in source.lines().enumerate() {
            if text.contains("@image-url") {
                violations.push(format!("{}:{}", relative_path(&path).display(), line + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Move image sources into ui/icons, ui/cursors, or ui/images:\n{}",
        violations.join("\n")
    );
}

#[test]
fn image_files_are_centralized() {
    let mut violations = Vec::new();
    for path in editor_files() {
        let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or("");
        if matches!(
            extension.to_ascii_lowercase().as_str(),
            "svg"
                | "svgz"
                | "png"
                | "apng"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "bmp"
                | "ico"
                | "tif"
                | "tiff"
                | "avif"
                | "jxl"
                | "qoi"
                | "tga"
                | "dds"
                | "exr"
                | "hdr"
                | "pbm"
                | "pgm"
                | "ppm"
                | "pnm"
        ) && !is_asset_directory(&path)
        {
            violations.push(relative_path(&path).display().to_string());
        }
    }
    assert!(
        violations.is_empty(),
        "Move image files into ui/icons, ui/cursors, or ui/images:\n{}",
        violations.join("\n")
    );
}
