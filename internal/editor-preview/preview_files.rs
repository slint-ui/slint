// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore automounted avif svgz

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
use i_slint_compiler::source_path::{SourcePath, clean_path};

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The extensions of the `.slint` sources and images that a library directory serves.
const LIBRARY_FILE_EXTENSIONS: &[&str] = &[
    "slint", //
    // images, mirroring `i_slint_core::graphics::image_mime_type_from_extension`
    "png", "jpg", "jpeg", "svg", "svgz", "gif", "webp", "bmp", "ico", "avif",
];

/// The files a preview may be sent, read from disk.
pub(crate) struct PreviewFileAccess {
    files: HashSet<PathBuf>,
    library_directories: HashSet<PathBuf>,
}

impl PreviewFileAccess {
    /// The files the project's sources name: the documents the compiler loaded or imports,
    /// the session's `document_roots`, and the resources the documents reference.
    /// Also the sources, images, and fonts in the directories of the configured libraries and include paths.
    pub(crate) fn new(
        document_cache: &DocumentCache,
        document_roots: &HashSet<lsp_types::Url>,
    ) -> Self {
        let mut files = document_cache.all_paths_to_watch();
        files.extend(
            document_roots.iter().filter_map(|url| SourcePath::from_url(url).into_native_path()),
        );
        files.extend(referenced_resources(document_cache));
        Self { files, library_directories: library_directories(document_cache) }
    }

    pub(crate) fn allows(&self, path: &Path) -> bool {
        // Matched as spelled: resolving a UNC or an automounted path contacts the host it names.
        let path = clean_path(path);
        self.files.contains(&path)
            || (is_library_file(&path)
                && self
                    .library_directories
                    .iter()
                    .any(|directory| is_below_without_symlinks(&path, directory)))
    }
}

/// Reading the file follows a symlink, which can point anywhere on the disk.
fn is_below_without_symlinks(path: &Path, directory: &Path) -> bool {
    let Ok(below) = path.strip_prefix(directory) else { return false };
    let mut current = directory.to_path_buf();
    below.components().all(|component| {
        current.push(component);
        current.symlink_metadata().is_ok_and(|metadata| !metadata.file_type().is_symlink())
    })
}

/// The directories that `@library` imports and include paths resolve in,
/// see `TypeLoader::find_file_in_library_path` and `TypeLoader::find_file_in_include_path`.
fn library_directories(document_cache: &DocumentCache) -> HashSet<PathBuf> {
    let config = document_cache.compiler_configuration().compiler_config;
    let libraries = config.library_paths.into_values().filter_map(|path| {
        let path = std::path::absolute(path).ok()?;
        if path.is_dir() { Some(path) } else { path.parent().map(Path::to_path_buf) }
    });
    let importing_directories: HashSet<PathBuf> = document_cache
        .all_urls()
        .filter_map(|url| Some(SourcePath::from_url(&url).into_native_path()?.parent()?.to_owned()))
        .collect();
    let include_paths = config.include_paths.iter().flat_map(|include_path| {
        importing_directories.iter().map(move |directory| directory.join(include_path))
    });
    libraries.chain(include_paths).map(|directory| clean_path(&directory)).collect()
}

fn is_library_file(path: &Path) -> bool {
    path.to_str().is_some_and(i_slint_compiler::fileaccess::is_font_file)
        || path.extension().is_some_and(|extension| {
            LIBRARY_FILE_EXTENSIONS.iter().any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::empty_document_cache;

    fn access(document_cache: &DocumentCache) -> PreviewFileAccess {
        PreviewFileAccess::new(document_cache, &HashSet::new())
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

    /// Loads `main.slint` from a new project with `configure` applied to the compiler configuration.
    fn access_with(
        project: &Path,
        main_source: &str,
        configure: impl FnOnce(&mut i_slint_compiler::CompilerConfiguration),
    ) -> PreviewFileAccess {
        let main = project.join("main.slint");
        std::fs::write(&main, main_source).unwrap();
        let mut configuration = crate::document_cache::CompilerConfiguration::default();
        configure(&mut configuration.compiler_config);
        let mut session = crate::test::session_with(DocumentCache::new(configuration));
        crate::test::load(&mut session, &main, main_source);
        access(&session.document_cache)
    }

    fn create_files(files: &[&Path]) {
        for file in files {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, []).unwrap();
        }
    }

    #[test]
    fn reads_the_directory_of_a_library_mapped_to_its_entry_file() {
        let [project, vendor] = [(); 2].map(|_| tempfile::tempdir().unwrap());
        let library = vendor.path().join("widgets");
        let entry_point = library.join("lib.slint");
        let [unloaded_source, unreferenced_icon, secret, beside_library] = [
            library.join("ui/button.slint"),
            library.join("icons/close.png"),
            library.join(".env"),
            vendor.path().join("other.png"),
        ];
        create_files(&[&unloaded_source, &unreferenced_icon, &secret, &beside_library]);
        // A dependency outside the library's directory is served because the compiler loads it.
        let dependency = vendor.path().join("common/theme.slint");
        create_files(&[&dependency]);
        std::fs::write(&dependency, "export global Theme { }").unwrap();
        std::fs::write(
            &entry_point,
            r#"import { Theme } from "../common/theme.slint"; export component Lib { }"#,
        )
        .unwrap();

        let access = access_with(
            project.path(),
            r#"import { Lib } from "@widgets"; export component Main { Lib { } }"#,
            |config| {
                config.library_paths = [("widgets".to_string(), entry_point)].into_iter().collect()
            },
        );

        assert!(access.allows(&dependency));
        assert!(access.allows(&unloaded_source));
        assert!(access.allows(&unreferenced_icon));
        assert!(!access.allows(&secret));
        assert!(!access.allows(&beside_library));
        assert!(!access.allows(&library.join("../other.png")));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlink_in_a_library_directory() {
        let [project, library, secrets] = [(); 3].map(|_| tempfile::tempdir().unwrap());
        let secret = secrets.path().join("id_ed25519");
        let [linked_file, linked_directory, icon] = [
            library.path().join("icon.png"),
            library.path().join("icons"),
            library.path().join("real/close.png"),
        ];
        create_files(&[&secret, &icon]);
        std::os::unix::fs::symlink(&secret, &linked_file).unwrap();
        std::fs::copy(&secret, secrets.path().join("key.png")).unwrap();
        std::os::unix::fs::symlink(secrets.path(), &linked_directory).unwrap();

        let access = access_with(project.path(), "export component Main { }", |config| {
            config.library_paths =
                [("widgets".to_string(), library.path().to_owned())].into_iter().collect()
        });

        assert!(access.allows(&icon));
        assert!(!access.allows(&linked_file));
        assert!(!access.allows(&linked_directory.join("key.png")));
    }

    #[test]
    fn reads_a_library_mapped_to_a_path_relative_to_the_working_directory() {
        let project = tempfile::tempdir().unwrap();
        let working_directory = std::env::current_dir().unwrap();
        let library = tempfile::tempdir_in(&working_directory).unwrap();
        let icon = library.path().join("close.png");
        create_files(&[&icon]);
        let relative = library.path().strip_prefix(&working_directory).unwrap().to_owned();

        let access = access_with(project.path(), "export component Main { }", |config| {
            config.library_paths = [("widgets".to_string(), relative)].into_iter().collect()
        });

        assert!(access.allows(&icon));
    }

    #[test]
    fn reads_a_library_mapped_to_its_directory_but_not_the_directory_above() {
        let [project, vendor] = [(); 2].map(|_| tempfile::tempdir().unwrap());
        let library = vendor.path().join("widgets");
        let [icon, font, beside_library] =
            [library.join("close.png"), library.join("font.ttf"), vendor.path().join("other.png")];
        create_files(&[&icon, &font, &beside_library]);
        std::fs::write(library.join("button.slint"), "export component Button { }").unwrap();

        let access = access_with(
            project.path(),
            r#"import { Button } from "@widgets/button.slint"; export component Main { Button { } }"#,
            |config| {
                config.library_paths =
                    [("widgets".to_string(), library.clone())].into_iter().collect()
            },
        );

        assert!(access.allows(&icon));
        assert!(access.allows(&font));
        assert!(!access.allows(&beside_library));
    }

    #[test]
    fn reads_an_include_path_relative_to_the_importing_document() {
        let project = tempfile::tempdir().unwrap();
        let [shared_icon, beside_main] =
            [project.path().join("shared/icon.png"), project.path().join("unused.png")];
        create_files(&[&shared_icon, &beside_main]);

        let access = access_with(project.path(), "export component Main { }", |config| {
            config.include_paths = vec!["shared".into()]
        });

        assert!(access.allows(&shared_icon));
        assert!(!access.allows(&beside_main));
        assert!(!access.allows(&project.path().join("shared/../unused.png")));
    }

    #[test]
    fn applies_an_include_path_only_to_the_directories_of_documents() {
        let [project, pictures] = [(); 2].map(|_| tempfile::tempdir().unwrap());
        let [background, other_picture] =
            [pictures.path().join("background.png"), pictures.path().join("other.png")];
        create_files(&[&background, &other_picture]);

        let main_source = format!(
            r#"export component Main {{ Image {{ source: @image-url("{}"); }} }}"#,
            background.display().to_string().replace('\\', "/")
        );
        let access = access_with(project.path(), &main_source, |config| {
            config.include_paths = vec![".".into()]
        });

        assert!(access.allows(&background));
        assert!(!access.allows(&other_picture));
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
        session.send_files_to_preview(0, &requested);

        let recorded = recorded.borrow();
        let served = recorded
            .iter()
            .filter(|message| matches!(message, LspToPreviewMessage::SetContents { .. }))
            .count();
        assert_eq!(served, 2, "both requested files should have been served: {recorded:?}");
    }

    #[test]
    fn serves_a_resource_that_a_document_loaded_after_the_last_request_references() {
        use i_slint_live_preview::protocol::LspToPreviewMessage;

        let project = tempfile::tempdir().unwrap();
        let logo = project.path().join("logo.png");
        std::fs::write(&logo, []).unwrap();
        let main = project.path().join("main.slint");
        let logo_url = lsp_types::Url::from_file_path(&logo).unwrap();

        let (capture, recorded) = crate::test::preview_capture();
        let mut session = crate::test::session_with(empty_document_cache());
        session.primary_preview_mut().to_preview = capture;
        crate::test::load(&mut session, &main, "export component Main { }");
        session.send_files_to_preview(0, std::slice::from_ref(&logo_url));
        crate::test::load(
            &mut session,
            &main,
            r#"export component Main { Image { source: @image-url("logo.png"); } }"#,
        );
        recorded.borrow_mut().clear();

        session.send_files_to_preview(0, &[logo_url]);

        let recorded = recorded.borrow();
        assert!(
            matches!(recorded.as_slice(), [LspToPreviewMessage::SetContents { .. }]),
            "{recorded:?}"
        );
    }

    #[test]
    fn refuses_a_url_on_another_host() {
        use i_slint_live_preview::protocol::LspToPreviewMessage;

        let (capture, recorded) = crate::test::preview_capture();
        let mut session = crate::test::session_with(empty_document_cache());
        session.primary_preview_mut().to_preview = capture;
        let url = lsp_types::Url::parse("file://example.com/share/logo.png").unwrap();

        session.send_files_to_preview(0, std::slice::from_ref(&url));

        let recorded = recorded.borrow();
        assert!(
            matches!(recorded.as_slice(), [LspToPreviewMessage::ForgetFile { url: refused }] if refused == &url),
            "{recorded:?}"
        );
    }

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    #[test]
    fn serves_a_previewed_document_while_it_reloads() {
        use i_slint_live_preview::file_watcher::FileChangeKind;
        use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewComponent};

        let project = tempfile::tempdir().unwrap();
        let main = project.path().join("main.slint");
        let main_source = "export component Main { }";
        std::fs::write(&main, main_source).unwrap();

        let (capture, recorded) = crate::test::preview_capture();
        let mut session = crate::test::session_with(empty_document_cache());
        session.primary_preview_mut().to_preview = capture;
        let (url, _) = crate::test::load(&mut session, &main, main_source);
        session.show_preview(0, PreviewComponent { url: url.clone(), component: None });
        spin_on::spin_on(session.trigger_file_watcher(url.clone(), FileChangeKind::Changed))
            .unwrap();
        recorded.borrow_mut().clear();

        session.send_files_to_preview(0, &[url]);

        let recorded = recorded.borrow();
        assert!(
            matches!(recorded.as_slice(), [LspToPreviewMessage::SetContents { .. }]),
            "{recorded:?}"
        );
    }
}
