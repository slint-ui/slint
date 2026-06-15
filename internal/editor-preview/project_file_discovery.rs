// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};

use i_slint_compiler::project_file::{FILE_NAME, ProjectFile};
use lsp_types::Url;

use crate::{Result, uri_to_file};

#[allow(dead_code)]
pub fn find_project_file_for_document_url(document_url: &Url) -> Result<Option<ProjectFile>> {
    let Some(document_path) = uri_to_file(document_url) else {
        return Ok(None);
    };

    find_project_file_for_document_path(&document_path)
}

pub fn find_project_file_for_document_path(document_path: &Path) -> Result<Option<ProjectFile>> {
    let Some(candidate) = find_project_file_path_for_document_path(document_path)? else {
        return Ok(None);
    };

    ProjectFile::load(&candidate).map(Some)
}

pub fn find_project_file_path_for_document_path(document_path: &Path) -> Result<Option<PathBuf>> {
    let mut directory = if document_path.is_dir() {
        Some(document_path.to_path_buf())
    } else {
        document_path.parent().map(PathBuf::from)
    };

    while let Some(current_directory) = directory {
        let candidate = current_directory.join(FILE_NAME);
        match candidate.try_exists() {
            Ok(true) => return Ok(Some(candidate)),
            Ok(false) => directory = current_directory.parent().map(PathBuf::from),
            Err(error) => return Err(error.into()),
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{find_project_file_for_document_path, find_project_file_path_for_document_path};
    use i_slint_compiler::project_file::FILE_NAME;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn nearest_parent_wins() {
        with_test_directory(|root| {
            let top_project = root.join(FILE_NAME);
            let nested_directory = root.join("a/b");
            fs::create_dir_all(&nested_directory).unwrap();
            let nested_project = nested_directory.join(FILE_NAME);
            let document_directory = nested_directory.join("c");
            fs::create_dir_all(&document_directory).unwrap();
            let document = document_directory.join("main.slint");
            fs::write(&top_project, r#"{ "style": "fluent" }"#).unwrap();
            fs::write(&nested_project, r#"{ "style": "material" }"#).unwrap();

            let project = find_project_file_for_document_path(&document)
                .unwrap()
                .expect("project file expected");

            assert_eq!(project.source_path(), nested_project);
            assert_eq!(project.style(), Some("material"));
        });
    }

    #[test]
    fn no_project_file_yields_none() {
        with_test_directory(|root| {
            let document = root.join("src/main.slint");
            fs::create_dir_all(document.parent().unwrap()).unwrap();

            let project = find_project_file_for_document_path(&document).unwrap();

            assert!(project.is_none());
        });
    }

    #[test]
    fn nearest_parent_path_wins() {
        with_test_directory(|root| {
            let top_project = root.join(FILE_NAME);
            let nested_directory = root.join("a/b");
            fs::create_dir_all(&nested_directory).unwrap();
            let nested_project = nested_directory.join(FILE_NAME);
            let document_directory = nested_directory.join("c");
            fs::create_dir_all(&document_directory).unwrap();
            let document = document_directory.join("main.slint");
            fs::write(&top_project, r#"{ "style": "fluent" }"#).unwrap();
            fs::write(&nested_project, r#"{ "style": "material" }"#).unwrap();

            let project_path = find_project_file_path_for_document_path(&document)
                .unwrap()
                .expect("project file expected");

            assert_eq!(project_path, nested_project);
        });
    }

    #[test]
    fn invalid_present_project_file_errors() {
        with_test_directory(|root| {
            let top_project = root.join(FILE_NAME);
            let nested_directory = root.join("a/b");
            fs::create_dir_all(&nested_directory).unwrap();
            let nested_project = nested_directory.join(FILE_NAME);
            let document = nested_directory.join("main.slint");
            fs::write(&top_project, r#"{ "style": "fluent" }"#).unwrap();
            fs::write(&nested_project, "{").unwrap();

            let error = find_project_file_for_document_path(&document).unwrap_err();

            assert!(!error.to_string().is_empty());
        });
    }

    fn with_test_directory<R>(f: impl FnOnce(&Path) -> R) -> R {
        let temp_dir = TempDir::new().unwrap();
        f(temp_dir.path())
    }
}
