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
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::expression_types::ImageReference;
use i_slint_compiler::object_tree::visit_all_expressions;
use i_slint_compiler::source_path::clean_path;
use i_slint_live_preview::protocol::PreviewConfig;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The images and fonts a `.slint` can reference.
/// Within the roots, these are served even when no loaded document references them,
/// such as from a document that changed on disk and isn't reloaded yet.
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
    /// The images and fonts the loaded documents reference, wherever they are.
    referenced_resources: HashSet<PathBuf>,
}

impl PreviewFileAccess {
    pub(crate) fn new(
        workspace_folders: &[PathBuf],
        preview_config: &PreviewConfig,
        document_cache: &DocumentCache,
    ) -> Self {
        let project_files = document_cache.all_paths_to_watch();

        // Deduplicated before canonicalizing: a project has many more files
        // than directories, and each candidate costs a system call.
        let mut candidates: HashSet<PathBuf> = workspace_folders
            .iter()
            .chain(&preview_config.include_paths)
            .chain(preview_config.library_paths.values())
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

        Self { roots, project_files, referenced_resources: referenced_resources(document_cache) }
    }

    /// Whether a preview may be sent the contents of `path`, read from disk.
    ///
    /// A file a loaded document references is part of the project wherever it is.
    /// Any other file has to sit in one of the roots — that is what keeps a viewer to
    /// the project being previewed — and be part of that project: either the
    /// compiler already loaded it, or it is named like an asset a `.slint` can
    /// reference. Without the second half, a viewer could ask for the `.env` or
    /// the `.git/config` that happen to sit next to the sources.
    pub(crate) fn allows(&self, path: &Path) -> bool {
        // The two halves normalize differently on purpose: the root test has to
        // resolve `..` and symlinks to mean anything, while the compiler's paths
        // are cleaned but not canonical, so that is what they compare against.
        let cleaned = clean_path(path);
        if self.referenced_resources.contains(&cleaned) {
            return true;
        }
        let Ok(canonical) = path.canonicalize() else {
            return false;
        };
        if !self.roots.iter().any(|root| canonical.starts_with(root)) {
            return false;
        }
        self.project_files.contains(&cleaned) || is_asset(path)
    }
}

/// The files the `@image-url`s and font imports of the loaded documents resolve to.
fn referenced_resources(document_cache: &DocumentCache) -> HashSet<PathBuf> {
    fn collect_images(expression: &Expression, resources: &mut HashSet<PathBuf>) {
        if let Expression::ImageReference { resource_ref: ImageReference::Source(path), .. } =
            expression
            && let Some(path) = path.as_native_path()
        {
            resources.insert(clean_path(path));
        }
        expression.visit(|expression| collect_images(expression, resources));
    }

    let mut resources = HashSet::new();
    for document in document_cache.all_documents() {
        resources.extend(
            document
                .custom_fonts
                .iter()
                .filter_map(|(path, _)| path.as_native_path())
                .map(clean_path),
        );
        for component in &document.inner_components {
            visit_all_expressions(component, |expression, _| {
                collect_images(expression, &mut resources)
            });
        }
    }
    resources
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

    fn access(preview_config: PreviewConfig, document_cache: &DocumentCache) -> PreviewFileAccess {
        PreviewFileAccess::new(&[], &preview_config, document_cache)
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

        let mut session = crate::test::session_with(empty_document_cache());
        crate::test::load(&mut session, &source, contents);
        let access = access(PreviewConfig::default(), &session.document_cache);

        assert!(access.allows(&source));
        // ... but a file beside it that the compiler never loaded is refused.
        let secret = project.path().join(".env");
        std::fs::write(&secret, "TOKEN=42").unwrap();
        assert!(!access.allows(&secret));
    }

    #[test]
    fn reads_the_resources_a_loaded_document_references_wherever_they_are() {
        let project = tempfile::tempdir().unwrap();
        let [ui, assets] = ["ui", "assets"].map(|directory| project.path().join(directory));
        std::fs::create_dir(&ui).unwrap();
        std::fs::create_dir(&assets).unwrap();
        // Outside the roots, and not named like an asset.
        let background = assets.join("background.tga");
        let font = assets.join("font.ttf");
        let unreferenced = assets.join("other.tga");
        for file in [&background, &font, &unreferenced] {
            std::fs::write(file, []).unwrap();
        }
        let source = ui.join("main.slint");
        let contents = r#"
            import "../assets/font.ttf";
            export component Main {
                Image { source: @image-url("../assets/background.tga"); }
            }
        "#;
        std::fs::write(&source, contents).unwrap();

        let mut session = crate::test::session_with(empty_document_cache());
        crate::test::load(&mut session, &source, contents);
        let access = access(PreviewConfig::default(), &session.document_cache);

        assert!(access.allows(&background));
        assert!(access.allows(&font));
        assert!(!access.allows(&unreferenced));
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
        session.primary_preview_mut().to_preview = capture;
        crate::test::load(&mut session, &main, main_source);
        recorded.borrow_mut().clear();

        // `other.slint` is a document the compiler loaded,
        // `logo.png` a resource that `main.slint` references.
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
        let access = PreviewFileAccess::new(
            &[workspace.path().to_path_buf()],
            &PreviewConfig::default(),
            &document_cache,
        );

        assert!(access.allows(&logo));
    }
}
