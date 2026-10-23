// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore avif svgz ttc

//! Which files on the developer's machine a preview may ask for.
//!
//! A remote viewer runs on another device,
//! and asks the LSP or the editor for the files it needs by URL.
//! This is the boundary between the project being previewed and the rest of the disk.
//! Files the session pushes on its own don't go through here,
//! such as the loaded sources and the fonts they import.

use crate::DocumentCache;
use i_slint_compiler::source_path::clean_path;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The images and fonts a `.slint` can reference.
/// The compiler records assets only in the full pass pipeline, which the `DocumentCache` doesn't run,
/// so they're recognized by their name.
const ASSET_EXTENSIONS: &[&str] = &[
    // images, mirroring `i_slint_core::graphics::image_mime_type_from_extension`
    "png", "jpg", "jpeg", "svg", "svgz", "gif", "webp", "bmp", "ico", "avif", //
    // fonts, mirroring `i_slint_compiler::fileaccess::is_font_file`
    "ttf", "ttc", "otf",
];

/// The files a preview may be sent, read from disk.
pub(crate) struct PreviewFileAccess {
    /// Directories files may come from: the workspace the editor opened, the
    /// configured include and library paths, and the directories of the
    /// documents already loaded (which covers editing a file outside any
    /// workspace).
    roots: HashSet<PathBuf>,
    /// Every file the compiler loaded for this project. Sources aren't required
    /// to be named `.slint`, so this is what says a file belongs to the project
    /// rather than its name does.
    project_files: HashSet<PathBuf>,
}

impl PreviewFileAccess {
    pub(crate) fn new(workspace_folders: &[PathBuf], document_cache: &DocumentCache) -> Self {
        let project_files = document_cache.all_paths_to_watch();
        let compiler_config = document_cache.compiler_configuration().compiler_config;

        // Deduplicated before canonicalizing: a project has many more files
        // than directories, and each candidate costs a system call.
        let mut candidates: HashSet<PathBuf> = workspace_folders
            .iter()
            .chain(&compiler_config.include_paths)
            .chain(compiler_config.library_paths.values())
            .cloned()
            .collect();
        candidates
            .extend(project_files.iter().filter_map(|path| path.parent()).map(Path::to_path_buf));

        let roots = candidates
            .iter()
            .filter_map(|path| {
                // Canonical, so that comparing against a requested path can't
                // be sidestepped with `..` or a symlink.
                let path = path.canonicalize().ok()?;
                // A candidate given as a file — a library mapped to a single
                // `.slint` — stands for the directory it sits in, which is the
                // unit the rest of that library lives in.
                if path.is_dir() { Some(path) } else { Some(path.parent()?.to_path_buf()) }
            })
            .collect();

        Self { roots, project_files }
    }

    /// Whether a preview may be sent the contents of `path`, read from disk.
    ///
    /// A file has to sit in one of the roots — that is what keeps a viewer to
    /// the project being previewed — and be part of that project: either the
    /// compiler already loaded it, or it is named like an asset a `.slint` can
    /// reference. Without the second half, a viewer could ask for the `.env` or
    /// the `.git/config` that happen to sit next to the sources.
    pub(crate) fn allows(&self, path: &Path) -> bool {
        // The two halves normalize differently on purpose: the root test has to
        // resolve `..` and symlinks to mean anything, while the compiler's paths
        // are cleaned but not canonical, so that is what they compare against.
        let Ok(canonical) = path.canonicalize() else {
            return false;
        };
        if !self.roots.iter().any(|root| canonical.starts_with(root)) {
            return false;
        }
        self.project_files.contains(&clean_path(path)) || is_asset(path)
    }
}

fn is_asset(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        ASSET_EXTENSIONS.iter().any(|asset| extension.eq_ignore_ascii_case(asset))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::empty_document_cache;

    fn access(document_cache: &DocumentCache) -> PreviewFileAccess {
        PreviewFileAccess::new(&[], document_cache)
    }

    fn cache_with_paths(
        include_paths: Vec<PathBuf>,
        library_paths: std::collections::HashMap<String, PathBuf>,
    ) -> DocumentCache {
        let mut config = empty_document_cache().compiler_configuration();
        config.compiler_config.include_paths = include_paths;
        config.compiler_config.library_paths = library_paths;
        DocumentCache::new(config)
    }

    #[test]
    fn reads_only_project_files_within_the_roots() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let cache = cache_with_paths(vec![root.path().to_path_buf()], Default::default());
        let access = access(&cache);

        // Assets the sources reference, whatever the extension's case.
        let asset = root.path().join("logo.PNG");
        std::fs::write(&asset, []).unwrap();
        assert!(access.allows(&asset));

        // Not something a preview shows, even inside the project.
        let secret = root.path().join(".env");
        std::fs::write(&secret, "TOKEN=42").unwrap();
        assert!(!access.allows(&secret));

        // Outside the roots, whether asked for directly or through `..`.
        let outside = elsewhere.path().join("logo.png");
        std::fs::write(&outside, []).unwrap();
        assert!(!access.allows(&outside));
        let traversal =
            root.path().join("..").join(elsewhere.path().file_name().unwrap()).join("logo.png");
        assert!(!access.allows(&traversal));

        assert!(!access.allows(&root.path().join("missing.png")));
    }

    #[test]
    fn reads_a_source_the_compiler_loaded_whatever_its_name() {
        // Nothing requires a source to be named `.slint`.
        let project = tempfile::tempdir().unwrap();
        let source = project.path().join("sources.not-slint");
        let contents = "export component Main {}";
        std::fs::write(&source, contents).unwrap();

        let mut session = crate::test::session_with(empty_document_cache());
        crate::test::load(&mut session, &source, contents);
        let access = access(&session.document_cache);

        assert!(access.allows(&source));
        // ... but a file beside it that the compiler never loaded is refused.
        let secret = project.path().join(".env");
        std::fs::write(&secret, "TOKEN=42").unwrap();
        assert!(!access.allows(&secret));
    }

    #[test]
    fn a_library_mapped_to_a_file_makes_its_directory_readable() {
        let library = tempfile::tempdir().unwrap();
        let entry_point = library.path().join("lib.slint");
        std::fs::write(&entry_point, "export component Lib {}").unwrap();
        let logo = library.path().join("logo.png");
        std::fs::write(&logo, []).unwrap();

        let cache =
            cache_with_paths(vec![], [("widgets".to_string(), entry_point)].into_iter().collect());
        let access = access(&cache);

        // `@widgets` imports lib.slint, which references the files beside it.
        assert!(access.allows(&logo));
    }

    /// End to end through [`crate::EditorSession::send_files_to_preview`].
    /// A viewer asks for the `.slint` files its sources import and the assets they reference.
    /// Both have to make it through, or the preview shows a broken component.
    #[test]
    fn serves_the_files_a_viewer_asks_for() {
        use i_slint_live_preview::protocol::LspToPreviewMessage;

        let project = tempfile::tempdir().unwrap();
        std::fs::write(project.path().join("other.slint"), "export component Other { }").unwrap();
        std::fs::write(project.path().join("logo.png"), []).unwrap();
        let main = project.path().join("main.slint");
        let main_source = r#"
            import { Other } from "other.slint";
            export component Main {
                Image { source: @image-url("logo.png"); }
                Other { }
            }
        "#;
        std::fs::write(&main, main_source).unwrap();

        let (capture, recorded) = crate::test::preview_capture();
        let mut session = crate::test::session_with(empty_document_cache());
        session.replace_preview_transport(0, capture);
        crate::test::load(&mut session, &main, main_source);
        recorded.borrow_mut().clear();

        // `other.slint` is a document the compiler loaded.
        // `logo.png` isn't, because the `DocumentCache` only runs the import passes.
        let requested = ["other.slint", "logo.png"]
            .map(|file| lsp_types::Url::from_file_path(project.path().join(file)).unwrap());
        session.send_files_to_preview(0, &requested, &[]);

        let recorded = recorded.borrow();
        let served = recorded
            .iter()
            .filter(|message| matches!(message, LspToPreviewMessage::SetContents { .. }))
            .count();
        assert_eq!(served, 2, "both requested files should have been served: {recorded:?}");
    }

    #[test]
    fn a_workspace_folder_makes_its_assets_readable() {
        let workspace = tempfile::tempdir().unwrap();
        let logo = workspace.path().join("images").join("logo.png");
        std::fs::create_dir(logo.parent().unwrap()).unwrap();
        std::fs::write(&logo, []).unwrap();

        let document_cache = empty_document_cache();
        let access = PreviewFileAccess::new(&[workspace.path().to_path_buf()], &document_cache);

        assert!(access.allows(&logo));
    }
}
