// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore automounted

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

/// The files a preview may be sent, read from disk.
pub(crate) struct PreviewFileAccess {
    files: HashSet<PathBuf>,
}

impl PreviewFileAccess {
    /// The files the project's sources name: the documents the compiler loaded or imports,
    /// and the resources the documents reference.
    pub(crate) fn new(document_cache: &DocumentCache) -> Self {
        Self { files: document_cache.all_paths_to_watch() }
    }

    pub(crate) fn allows(&self, path: &Path) -> bool {
        // Matched as spelled: resolving a UNC or an automounted path contacts the host it names.
        self.files.contains(&clean_path(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::empty_document_cache;

    fn access(document_cache: &DocumentCache) -> PreviewFileAccess {
        PreviewFileAccess::new(document_cache)
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
    fn reads_the_resources_a_loaded_document_references_wherever_they_are() {
        let project = tempfile::tempdir().unwrap();
        let [ui, assets] = ["ui", "assets"].map(|directory| project.path().join(directory));
        std::fs::create_dir(&ui).unwrap();
        std::fs::create_dir(&assets).unwrap();
        // Outside the directory of the document.
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
        let access = access(&session.document_cache);

        assert!(access.allows(&background));
        assert!(access.allows(&font));
        assert!(!access.allows(&unreferenced));
    }

    #[test]
    fn refuses_an_image_beside_the_sources_that_none_references() {
        let project = tempfile::tempdir().unwrap();
        let [logo, unused] = ["logo.png", "unused.png"].map(|file| project.path().join(file));
        for file in [&logo, &unused] {
            std::fs::write(file, []).unwrap();
        }
        let source = project.path().join("main.slint");
        let contents = r#"export component Main { Image { source: @image-url("logo.png"); } }"#;
        std::fs::write(&source, contents).unwrap();

        let mut session = crate::test::session_with(empty_document_cache());
        crate::test::load(&mut session, &source, contents);
        let access = access(&session.document_cache);

        assert!(access.allows(&logo));
        assert!(!access.allows(&unused));
    }

    #[test]
    fn reads_the_files_of_a_library_outside_the_project() {
        let [project, library] = [(); 2].map(|_| tempfile::tempdir().unwrap());
        let entry_point = library.path().join("lib.slint");
        let entry_point_source =
            r#"export component Lib { Image { source: @image-url("logo.png"); } }"#;
        std::fs::write(&entry_point, entry_point_source).unwrap();
        let logo = library.path().join("logo.png");
        std::fs::write(&logo, []).unwrap();
        let main = project.path().join("main.slint");
        let main_source = r#"import { Lib } from "@widgets"; export component Main { Lib { } }"#;
        std::fs::write(&main, main_source).unwrap();

        let mut configuration = crate::document_cache::CompilerConfiguration::default();
        configuration.compiler_config.library_paths =
            [("widgets".to_string(), entry_point.clone())].into_iter().collect();
        let mut session = crate::test::session_with(DocumentCache::new(configuration));
        crate::test::load(&mut session, &main, main_source);
        let access = access(&session.document_cache);

        assert!(access.allows(&entry_point));
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

        // `other.slint` is a document the compiler loaded,
        // `logo.png` a resource that `main.slint` references.
        let requested = ["other.slint", "logo.png"]
            .map(|file| lsp_types::Url::from_file_path(project.path().join(file)).unwrap());
        session.send_files_to_preview(0, &requested);

        let recorded = recorded.borrow();
        let served = recorded
            .iter()
            .filter(|message| matches!(message, LspToPreviewMessage::SetContents { .. }))
            .count();
        assert_eq!(served, 2, "both requested files should have been served: {recorded:?}");
    }

    // On Windows, such a URL is a UNC path.
    #[cfg(not(windows))]
    #[test]
    fn refuses_a_url_on_another_host() {
        use i_slint_live_preview::protocol::LspToPreviewMessage;

        let (capture, recorded) = crate::test::preview_capture();
        let mut session = crate::test::session_with(empty_document_cache());
        session.replace_preview_transport(0, capture);
        let url = lsp_types::Url::parse("file://example.com/share/logo.png").unwrap();

        session.send_files_to_preview(0, std::slice::from_ref(&url));

        let recorded = recorded.borrow();
        assert!(
            matches!(recorded.as_slice(), [LspToPreviewMessage::ForgetFile { url: refused }] if refused == &url),
            "{recorded:?}"
        );
    }

}
