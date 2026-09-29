// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The state of a Slint project as it is being edited, shared between the
//! language server and the visual editor.

use i_slint_compiler::diagnostics::BuildDiagnostics;
#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
use i_slint_live_preview::protocol::PreviewComponent;
use i_slint_live_preview::{
    file_watcher::FileChangeKind,
    protocol::{LspToPreviewMessage, PreviewConfig, SourceFileVersion, VersionedUrl},
};
use itertools::Itertools;
use lsp_types::Url;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

/// Diagnostics paired with the document version for which they were computed.
pub type VersionedDiagnostics = Vec<(Url, SourceFileVersion, Vec<lsp_types::Diagnostic>)>;

pub struct PreviewConnection {
    pub to_preview: Rc<crate::LspToPreviews>,
    /// The last component for which the user clicked "show preview"
    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    pub to_show: Option<PreviewComponent>,
}

/// The documents currently being edited, together with the state the preview
/// needs to follow along.
pub struct EditorSession {
    pub document_cache: crate::DocumentCache,
    pub preview_config: PreviewConfig,
    /// File currently open in the editor
    pub open_urls: HashSet<lsp_types::Url>,
    pub previews: Vec<PreviewConnection>,
    /// Files to recompile after all other operations are done
    /// (i.e. recompilations triggered by updates to unopened files)
    pub pending_recompile: HashSet<lsp_types::Url>,
}

impl EditorSession {
    pub fn primary_preview(&self) -> &PreviewConnection {
        self.previews.first().expect("EditorSession must have at least one preview")
    }

    pub fn primary_preview_mut(&mut self) -> &mut PreviewConnection {
        self.previews.first_mut().expect("EditorSession must have at least one preview")
    }

    pub fn preview(&self, preview_index: usize) -> Option<&PreviewConnection> {
        let preview = self.previews.get(preview_index);
        if preview.is_none() {
            tracing::warn!(
                "Preview index {preview_index} is out of bounds for {} previews",
                self.previews.len()
            );
        }
        preview
    }

    pub fn preview_mut(&mut self, preview_index: usize) -> Option<&mut PreviewConnection> {
        let preview_count = self.previews.len();
        let preview = self.previews.get_mut(preview_index);
        if preview.is_none() {
            tracing::warn!(
                "Preview index {preview_index} is out of bounds for {preview_count} previews"
            );
        }
        preview
    }

    pub fn send_to_preview(&self, preview_index: usize, message: &LspToPreviewMessage) {
        let Some(preview) = self.preview(preview_index) else { return };
        preview.to_preview.send(message);
    }

    pub fn send_to_previews(&self, message: &LspToPreviewMessage) {
        for preview in &self.previews {
            preview.to_preview.send(message);
        }
    }

    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    pub fn send_state_to_preview(&self, preview_index: usize) {
        let Some(preview) = self.preview(preview_index) else { return };
        let mut doc_count = 0;
        #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
        let mut fonts_sent = HashSet::<PathBuf>::new();
        for (url, node) in self.document_cache.all_url_documents() {
            if url.scheme() == "builtin" {
                continue;
            }
            let version = self.document_cache.document_version(&url);

            preview.to_preview.send(&LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(url.clone(), version),
                contents: node.text().to_string().into(),
            });
            #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
            self.send_referenced_fonts(preview, &url, &mut fonts_sent);
            doc_count += 1;
        }

        preview
            .to_preview
            .send(&LspToPreviewMessage::SetConfiguration { config: self.preview_config.clone() });

        if let Some(component) = preview.to_show.clone() {
            tracing::debug!(
                "Sending state to preview: {} documents, showing {}",
                doc_count,
                component.url
            );
            preview.to_preview.send(&LspToPreviewMessage::ShowPreview(component));
        } else {
            tracing::debug!(
                "Sending state to preview: {} documents, showing default component",
                doc_count
            );
        }
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        any(feature = "preview-external", feature = "preview-engine", feature = "preview-remote"),
    ))]
    pub fn send_files_to_preview(
        &self,
        preview_index: usize,
        files: &[lsp_types::Url],
        allows_file: impl Fn(&std::path::Path) -> bool,
    ) {
        let Some(preview) = self.preview(preview_index) else { return };
        #[cfg(feature = "preview-remote")]
        let mut fonts_sent = HashSet::<PathBuf>::new();
        for url in files {
            if let Some(node) =
                self.document_cache.get_document(url).and_then(|doc| doc.node.as_ref())
            {
                let version = self.document_cache.document_version_by_path(node.source_file.path());
                let contents = node.text().to_string().into();
                tracing::debug!("Sending cached file {} to preview", url);
                preview.to_preview.send(&LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(url.clone(), version),
                    contents,
                });
                #[cfg(feature = "preview-remote")]
                self.send_referenced_fonts(preview, url, &mut fonts_sent);
                continue;
            }
            let Some(path) = url.to_file_path().ok() else {
                tracing::warn!("Cannot convert URL to file path: {url}");
                continue;
            };
            if !allows_file(&path) {
                tracing::warn!(
                    "Refusing to send {} to the preview: not a file of the project being previewed",
                    path.display()
                );
                preview.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
                continue;
            }
            match std::fs::read(&path) {
                Ok(contents) => {
                    tracing::debug!("Sending file {} ({} bytes) to preview", url, contents.len());
                    preview.to_preview.send(&LspToPreviewMessage::SetContents {
                        url: VersionedUrl::new(url.clone(), None),
                        contents,
                    });
                }
                Err(err) => {
                    tracing::warn!("Failed to read file {}: {err}", path.display());
                    preview.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
                }
            }
        }
    }

    /// Read each font file imported by the `.slint` at `doc_url` and push it
    /// to the remote viewer via `SetContents`. Only the remote viewer needs
    /// font bytes pushed: local previews read fonts from disk. Fonts in `sent`
    /// are skipped: callers seed it with fonts that were already transferred
    /// (e.g. referenced by an earlier document in the same batch, or sent
    /// before the current edit).
    #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
    fn send_referenced_fonts(
        &self,
        preview: &PreviewConnection,
        doc_url: &Url,
        sent: &mut HashSet<PathBuf>,
    ) {
        let Some(remote) = preview.to_preview.remote() else { return };
        let Some(doc) = self.document_cache.get_document(doc_url) else { return };
        // `custom_fonts` holds the resolved path of every font import that
        // passed the compiler's existence check, plus remote URLs.
        for (font_path, _) in &doc.custom_fonts {
            let font_path = PathBuf::from(font_path.as_str());
            if i_slint_compiler::pathutils::is_url(&font_path) {
                continue;
            }
            if !sent.insert(font_path.clone()) {
                continue;
            }
            let Ok(font_url) = Url::from_file_path(&font_path) else {
                tracing::warn!("Cannot convert font path to URL: {}", font_path.display());
                continue;
            };
            match std::fs::read(&font_path) {
                Ok(contents) => {
                    tracing::debug!(
                        "Sending font {} ({} bytes) to remote viewer",
                        font_url,
                        contents.len()
                    );
                    remote.send(&LspToPreviewMessage::SetContents {
                        url: VersionedUrl::new(font_url, None),
                        contents,
                    });
                }
                Err(err) => {
                    tracing::warn!("Failed to read font {}: {err}", font_path.display());
                }
            }
        }
    }

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    pub fn show_preview(&mut self, preview_index: usize, component: PreviewComponent) {
        let component_url = component.url.clone();
        let Some(preview) = self.preview_mut(preview_index) else { return };
        preview.to_show = Some(component.clone());
        preview.to_preview.send(&LspToPreviewMessage::ShowPreview(component));
        self.pending_recompile.insert(component_url);
    }

    pub async fn load_document_impl(
        &mut self,
        content: String,
        url: lsp_types::Url,
        version: Option<i32>,
    ) -> (HashSet<PathBuf>, BuildDiagnostics) {
        enum FileAction {
            ProcessContent(String),
            IgnoreFile,
            InvalidateFile,
        }

        tracing::trace!("Loading document: {url} (version: {version:?})");

        let Some(path) = crate::uri_to_file(&url) else { return Default::default() };
        // Normalize the URL
        let Ok(url) = Url::from_file_path(path.clone()) else { return Default::default() };

        let action = if path.extension().is_some_and(|e| e == "rs") {
            match i_slint_compiler::lexer::extract_rust_macro(content) {
                Some(content) => FileAction::ProcessContent(content),
                // A rust file without a rust macro, just ignore it
                None => {
                    if self.document_cache.get_document(&url).is_some() {
                        // This had contents before: Continue so we can invalidate it!
                        FileAction::InvalidateFile
                    } else {
                        FileAction::IgnoreFile
                    }
                }
            }
        } else {
            FileAction::ProcessContent(content)
        };

        let mut diag = BuildDiagnostics::default();

        let dependencies = match action {
            FileAction::ProcessContent(content) => {
                self.send_to_previews(&LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(url.clone(), version),
                    contents: content.clone().into(),
                });
                // Fonts imported before this edit were pushed to the remote viewer
                // already; seed the sent set with them so only fonts added by this
                // edit are transferred.
                #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
                let fonts_sent: HashSet<PathBuf> = self
                    .document_cache
                    .get_document(&url)
                    .map(|doc| {
                        doc.custom_fonts.iter().map(|(p, _)| PathBuf::from(p.as_str())).collect()
                    })
                    .unwrap_or_default();
                let dependencies: HashSet<Url> = self.document_cache.invalidate_url(&url);
                let _ = self.document_cache.load_url(&url, version, content, &mut diag).await;
                #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
                for preview in &self.previews {
                    let mut fonts_sent_to_preview = fonts_sent.clone();
                    self.send_referenced_fonts(preview, &url, &mut fonts_sent_to_preview);
                }
                dependencies
            }
            FileAction::IgnoreFile => return Default::default(),
            FileAction::InvalidateFile => {
                self.send_to_previews(&LspToPreviewMessage::ForgetFile { url: url.clone() });
                self.document_cache.invalidate_url(&url)
            }
        };

        for dep in &dependencies {
            if self.open_urls.contains(dep) {
                self.document_cache.reload_cached_file(dep, &mut diag).await;
            }
        }

        let extra_files = dependencies
            .iter()
            .filter_map(crate::uri_to_file)
            .chain(core::iter::once(path))
            .collect();

        (extra_files, diag)
    }

    pub async fn open_document(
        &mut self,
        content: String,
        url: lsp_types::Url,
        version: Option<i32>,
    ) -> crate::Result<crate::VersionedDiagnostics> {
        tracing::debug!("Opening document: {url}");
        self.open_urls.insert(url.clone());

        self.load_document(content, url, version).await
    }

    pub async fn close_document(&mut self, url: lsp_types::Url) -> crate::Result<()> {
        tracing::debug!("Closing document: {url}");
        self.open_urls.remove(&url);
        self.drop_document(url).await
    }

    pub async fn load_document(
        &mut self,
        content: String,
        url: lsp_types::Url,
        version: Option<i32>,
    ) -> crate::Result<crate::VersionedDiagnostics> {
        let (extra_files, diag) = self.load_document_impl(content, url.clone(), version).await;

        tracing::debug!("Loaded {url} with {} diagnostics", diag.iter().count());

        Ok(collect_diagnostics(&self.document_cache, &extra_files, diag))
    }

    #[cfg_attr(target_arch = "wasm32", allow(unused))]
    pub async fn reload_document(
        &mut self,
        url: lsp_types::Url,
    ) -> crate::Result<crate::VersionedDiagnostics> {
        tracing::debug!("Reloading document: {url}");

        // Check if document is in cache (can use reload_cached_file)
        let in_cache = self.document_cache.all_urls().contains(&url);

        if in_cache {
            tracing::trace!("Document is in cache, reloading: {url}");

            let mut diagnostics = BuildDiagnostics::default();

            self.document_cache.reload_cached_file(&url, &mut diagnostics).await;
            let mut extra_files = HashSet::new();
            extra_files.extend(crate::uri_to_file(&url));

            Ok(collect_diagnostics(&self.document_cache, &extra_files, diagnostics))
        } else {
            tracing::trace!("Document not in cache, loading from disk: {url}");

            let Some(path) = crate::uri_to_file(&url) else {
                // The file was likely deleted, log and move on
                tracing::debug!("Failed to locate file: {url}");
                return Ok(Default::default());
            };
            match std::fs::read_to_string(&path) {
                Ok(content) => self.load_document(content, url, None).await,
                // The file was likely deleted, log and move on
                Err(err) => {
                    tracing::debug!("Failed to read {} from disk: {err}", path.display());
                    Ok(Default::default())
                }
            }
        }
    }

    fn drop_document_impl(&mut self, url: lsp_types::Url) -> crate::Result<()> {
        let dependencies = self.document_cache.drop_document(&url)?;

        let open_dependencies = self.open_urls.intersection(&dependencies).cloned();
        self.pending_recompile.extend(open_dependencies);

        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        // The external preview only has access to the files the LSP recompiled, so we need to
        // ensure the preview file is recompiled if anything it depends on changes, even if it's
        // not in the open_urls.
        for preview_url in self
            .previews
            .iter()
            .filter_map(|preview| preview.to_show.as_ref().map(|component| component.url.clone()))
            .filter(|preview_url| preview_url == &url || dependencies.contains(preview_url))
        {
            self.pending_recompile.insert(preview_url);
        }

        Ok(())
    }

    pub async fn drop_document(&mut self, url: lsp_types::Url) -> crate::Result<()> {
        tracing::debug!("Dropping document: {url}");
        // The preview cares about resources and slint files, so forward everything
        self.send_to_previews(&LspToPreviewMessage::InvalidateContents { url: url.clone() });

        self.drop_document_impl(url)
    }

    pub async fn delete_document(
        &mut self,
        url: lsp_types::Url,
    ) -> crate::Result<crate::VersionedDiagnostics> {
        tracing::debug!("Deleting document: {url}");
        // The preview cares about resources and slint files, so forward everything
        self.send_to_previews(&LspToPreviewMessage::ForgetFile { url: url.clone() });

        // The cleared diagnostics below carry the version the document had before the drop.
        let version = self.document_cache.document_version(&url);

        self.drop_document_impl(url.clone())?;

        // make sure to clear the diagnostics on this file.
        // This is especially important for deleted files, but also for renamed files to clear the diagnostics on the old file.
        // Otherwise they will stick around forever (e.g. in VS Code).
        Ok(vec![(url, version, vec![])])
    }

    pub async fn trigger_file_watcher(
        &mut self,
        url: lsp_types::Url,
        typ: FileChangeKind,
    ) -> crate::Result<crate::VersionedDiagnostics> {
        if !self.open_urls.contains(&url) {
            tracing::debug!("File watcher triggered for {url} (type: {:?})", typ);
            match typ {
                FileChangeKind::Deleted => return self.delete_document(url).await,
                // If the file was newly created, we still need to drop it as another file may
                // already depend on it by trying to import it before it exists.
                // This is especially common on file renames.
                // See also #11304
                FileChangeKind::Changed | FileChangeKind::Created => {
                    self.drop_document(url).await?
                }
            }
        } else {
            tracing::trace!("Ignoring file watcher event for open document: {url}");
        }
        Ok(Default::default())
    }
}

pub fn convert_diagnostics(
    extra_files: &HashSet<PathBuf>,
    diag: BuildDiagnostics,
    format: crate::ByteFormat,
) -> HashMap<Url, Vec<lsp_types::Diagnostic>> {
    // Always provide diagnostics for all files. Empty diagnostics clear any previous ones.
    let mut lsp_diags: HashMap<Url, Vec<lsp_types::Diagnostic>> = extra_files
        .iter()
        .chain(diag.all_loaded_files.iter())
        .filter_map(|p| Url::from_file_path(p).ok())
        .map(|uri| (uri, Default::default()))
        .collect();

    for d in diag.into_iter() {
        #[cfg(not(target_arch = "wasm32"))]
        if d.source_file().unwrap().is_relative() {
            continue;
        }
        let uri = Url::from_file_path(d.source_file().unwrap()).unwrap();
        lsp_diags
            .entry(uri)
            .or_default()
            .push(i_slint_live_preview::protocol::to_lsp_diagnostic(&d, format));
    }

    lsp_diags
}

pub fn collect_diagnostics(
    document_cache: &crate::DocumentCache,
    extra_files: &HashSet<PathBuf>,
    diag: BuildDiagnostics,
) -> crate::VersionedDiagnostics {
    let lsp_diags = convert_diagnostics(extra_files, diag, document_cache.format);
    tracing::trace!("Collected {} diagnostics", lsp_diags.values().flatten().count());

    lsp_diags
        .into_iter()
        .map(|(uri, diagnostics)| {
            let version = document_cache.document_version(&uri);
            (uri, version, diagnostics)
        })
        .collect()
}

#[cfg(all(test, any(feature = "preview-external", feature = "preview-engine")))]
mod tests {
    use super::*;

    fn session_with_recording_previews()
    -> (EditorSession, [crate::test::CapturedPreviewMessages; 2]) {
        let captures = std::array::from_fn(|_| crate::test::preview_capture());
        let previews = captures
            .iter()
            .map(|(to_preview, _)| PreviewConnection {
                to_preview: to_preview.clone(),
                to_show: None,
            })
            .collect();
        let messages = captures.map(|(_, messages)| messages);
        let session = EditorSession {
            document_cache: crate::test::empty_document_cache(),
            preview_config: Default::default(),
            open_urls: Default::default(),
            previews,
            pending_recompile: Default::default(),
        };
        (session, messages)
    }

    #[test]
    fn primary_preview_accessors_return_the_first_connection() {
        let (mut session, _) = session_with_recording_previews();
        let component = PreviewComponent {
            url: Url::from_file_path(crate::test::test_file_name("primary.slint")).unwrap(),
            component: Some("Primary".into()),
        };

        session.primary_preview_mut().to_show = Some(component.clone());

        assert_eq!(session.primary_preview().to_show, Some(component));
        assert!(session.preview(1).unwrap().to_show.is_none());
    }

    #[test]
    fn invalid_preview_indexes_are_ignored() {
        let (mut session, messages) = session_with_recording_previews();
        let component = PreviewComponent {
            url: Url::from_file_path(crate::test::test_file_name("missing.slint")).unwrap(),
            component: None,
        };

        assert!(session.preview(2).is_none());
        assert!(session.preview_mut(2).is_none());
        session.send_to_preview(2, &LspToPreviewMessage::Quit);
        session.send_state_to_preview(2);
        session.send_files_to_preview(2, &[], |_| true);
        session.show_preview(2, component);

        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
        assert!(session.pending_recompile.is_empty());
    }

    #[test]
    fn shared_messages_are_broadcast_to_every_preview() {
        let (mut session, messages) = session_with_recording_previews();
        let invalidated_url =
            Url::from_file_path(crate::test::test_file_name("invalidated.slint")).unwrap();
        let deleted_url =
            Url::from_file_path(crate::test::test_file_name("deleted.slint")).unwrap();

        spin_on::spin_on(session.load_document_impl(
            "export component Shared {}".into(),
            invalidated_url.clone(),
            Some(1),
        ));
        spin_on::spin_on(session.load_document_impl(
            "export component Deleted {}".into(),
            deleted_url.clone(),
            Some(1),
        ));
        session.send_to_previews(&LspToPreviewMessage::SetConfiguration {
            config: PreviewConfig::default(),
        });
        spin_on::spin_on(session.drop_document(invalidated_url)).unwrap();
        spin_on::spin_on(session.delete_document(deleted_url)).unwrap();

        for messages in messages {
            let messages = messages.borrow();
            assert!(
                messages
                    .iter()
                    .any(|message| { matches!(message, LspToPreviewMessage::SetContents { .. }) })
            );
            assert!(messages.iter().any(|message| {
                matches!(message, LspToPreviewMessage::InvalidateContents { .. })
            }));
            assert!(
                messages
                    .iter()
                    .any(|message| matches!(message, LspToPreviewMessage::ForgetFile { .. }))
            );
            assert!(
                messages
                    .iter()
                    .any(|message| matches!(message, LspToPreviewMessage::SetConfiguration { .. }))
            );
        }
    }

    #[test]
    fn preview_state_and_files_are_sent_only_to_the_requested_preview() {
        let (mut session, messages) = session_with_recording_previews();
        let temp_directory = tempfile::tempdir().unwrap();
        let path = temp_directory.path().join("requested.slint");
        std::fs::write(&path, "export component Requested {}").unwrap();
        let url = Url::from_file_path(path).unwrap();
        let component = PreviewComponent { url: url.clone(), component: Some("Requested".into()) };

        session.show_preview(1, component.clone());
        for recorded_messages in &messages {
            recorded_messages.borrow_mut().clear();
        }

        session.send_state_to_preview(1);
        session.send_files_to_preview(1, &[url], |_| true);

        assert!(messages[0].borrow().is_empty());
        let secondary_messages = messages[1].borrow();
        assert!(
            secondary_messages
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::SetConfiguration { .. }))
        );
        assert!(secondary_messages.iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(current) if current == &component)
        }));
        assert!(
            secondary_messages
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::SetContents { .. }))
        );
        assert!(session.primary_preview().to_show.is_none());
        assert_eq!(session.preview(1).unwrap().to_show, Some(component));
    }
}
