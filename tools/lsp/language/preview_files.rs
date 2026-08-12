// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore avif svgz ttc

//! Which files on the developer's machine a preview may ask for.
//!
//! The remote preview runs on another device and asks the LSP for the files it
//! needs by URL, so this is the boundary between the project being previewed
//! and the rest of the disk. Files the LSP pushes on its own — the loaded
//! sources, the fonts they import — don't go through here.

use crate::common::{DocumentCache, uri_to_file};
use i_slint_compiler::pathutils::{clean_path, is_url};
use i_slint_live_preview::protocol::PreviewConfig;
use lsp_types::InitializeParams;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The images and fonts a `.slint` can reference. Assets aren't compiled, so
/// the compiler doesn't know about them until the full pass pipeline has run
/// (which the LSP doesn't do) and they have to be recognized by their name.
const ASSET_EXTENSIONS: &[&str] = &[
    // images, mirroring `i_slint_core::graphics::image_mime_type_from_extension`
    "png", "jpg", "jpeg", "svg", "svgz", "gif", "webp", "bmp", "ico", "avif", //
    // fonts, mirroring `i_slint_compiler::pathutils::is_font_file`
    "ttf", "ttc", "otf",
];

/// The files a preview may be sent, read from disk.
pub struct PreviewFileAccess {
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
    pub fn new(
        init_param: &InitializeParams,
        preview_config: &PreviewConfig,
        document_cache: &DocumentCache,
    ) -> Self {
        let project_files = document_cache.all_paths_to_watch();

        // Deduplicated before canonicalizing: a project has many more files
        // than directories, and each candidate costs a system call. `builtin:/…`
        // paths aren't on disk at all.
        let mut candidates: HashSet<PathBuf> =
            crate::common::host_language_search::resolve_workspace_folders(init_param)
                .iter()
                .filter_map(|folder| uri_to_file(&folder.uri))
                .chain(preview_config.include_paths.iter().cloned())
                .chain(preview_config.library_paths.values().cloned())
                .collect();
        candidates.extend(
            project_files
                .iter()
                .filter(|path| !is_url(path))
                .filter_map(|path| path.parent())
                .map(Path::to_path_buf),
        );

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
    pub fn allows(&self, path: &Path) -> bool {
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
    use crate::language::test::empty_document_cache;

    fn access(preview_config: PreviewConfig, document_cache: &DocumentCache) -> PreviewFileAccess {
        PreviewFileAccess::new(&InitializeParams::default(), &preview_config, document_cache)
    }

    #[test]
    fn reads_only_project_files_within_the_roots() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let preview_config =
            PreviewConfig { include_paths: vec![root.path().to_path_buf()], ..Default::default() };
        let access = access(preview_config, &empty_document_cache());

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

        let mut ctx = crate::language::test::mock_context();
        crate::language::test::load(&mut ctx, &source, contents);
        let access = access(PreviewConfig::default(), &ctx.document_cache);

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

        let preview_config = PreviewConfig {
            library_paths: [("widgets".to_string(), entry_point)].into_iter().collect(),
            ..Default::default()
        };
        let access = access(preview_config, &empty_document_cache());

        // `@widgets` imports lib.slint, which references the files beside it.
        assert!(access.allows(&logo));
    }

    /// End to end through [`crate::language::send_files_to_preview`]: a viewer
    /// asks for the files its compiler resolves — the `.slint` its sources
    /// import, and the assets they reference — and both have to make it
    /// through, or the preview shows a broken component.
    #[test]
    fn serves_the_files_a_viewer_asks_for() {
        use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget};
        use std::cell::RefCell;
        use std::rc::Rc;

        struct Recorder(Rc<RefCell<Vec<LspToPreviewMessage>>>);
        impl crate::common::LspToPreview for Recorder {
            fn send(&self, message: &LspToPreviewMessage) {
                self.0.borrow_mut().push(message.clone());
            }
            fn preview_target(&self) -> PreviewTarget {
                PreviewTarget::Dummy
            }
        }

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

        let recorded = Rc::new(RefCell::new(Vec::new()));
        let mut ctx = crate::language::Context {
            to_preview: crate::common::LspToPreviews::with_one(Recorder(recorded.clone())),
            ..crate::language::test::mock_context()
        };
        crate::language::test::load(&mut ctx, &main, main_source);
        recorded.borrow_mut().clear();

        // `other.slint` is a document the compiler loaded; `logo.png` is not,
        // because the LSP only runs the import passes.
        let requested = ["other.slint", "logo.png"]
            .map(|file| lsp_types::Url::from_file_path(project.path().join(file)).unwrap());
        crate::language::send_files_to_preview(&ctx, &requested);

        let recorded = recorded.borrow();
        let served = recorded
            .iter()
            .filter(|message| matches!(message, LspToPreviewMessage::SetContents { .. }))
            .count();
        assert_eq!(served, 2, "both requested files should have been served: {recorded:?}");
    }
}
