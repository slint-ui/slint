// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The state of a Slint project as it is being edited, shared between the
//! language server and the visual editor.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::project_file::ProjectFile;
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

#[derive(Clone, Debug, Default)]
pub struct SessionConfigOverrides {
    pub hide_ui: Option<bool>,
    pub include_paths: Option<Vec<PathBuf>>,
    pub library_paths: Option<HashMap<String, PathBuf>>,
    pub style: Option<String>,
    pub experimental: Option<bool>,
}

impl SessionConfigOverrides {
    fn merge(&mut self, other: Self) {
        if other.hide_ui.is_some() {
            self.hide_ui = other.hide_ui;
        }
        if other.include_paths.is_some() {
            self.include_paths = other.include_paths;
        }
        if other.library_paths.is_some() {
            self.library_paths = other.library_paths;
        }
        if other.style.is_some() {
            self.style = other.style;
        }
        if other.experimental.is_some() {
            self.experimental = other.experimental;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectCompilerBaseline {
    include_paths: Option<Vec<PathBuf>>,
    library_paths: Option<HashMap<String, PathBuf>>,
    style: Option<String>,
    enable_experimental: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveProjectFile {
    source_path: PathBuf,
    baseline: Option<ProjectCompilerBaseline>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum ActiveProjectSelection {
    #[default]
    Unset,
    NoProject,
    Project(ActiveProjectFile),
}

enum DiscoveredProjectFile {
    None,
    File(ProjectFile),
    Invalid { path: PathBuf, error: Box<dyn std::error::Error> },
}

enum ActiveProjectUpdate {
    Unchanged,
    Reapply(ActiveProjectSelection),
}

/// The documents currently being edited, together with the state the preview
/// needs to follow along.
pub struct EditorSession {
    pub document_cache: crate::DocumentCache,
    pub preview_config: PreviewConfig,
    /// The last component for which the user clicked "show preview"
    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    pub to_show: Option<PreviewComponent>,
    /// File currently open in the editor
    pub open_urls: HashSet<lsp_types::Url>,
    pub to_preview: Rc<crate::LspToPreviews>,
    /// Files to recompile after all other operations are done
    /// (i.e. recompilations triggered by updates to unopened files)
    pub pending_recompile: HashSet<lsp_types::Url>,
    compiler_config_defaults: crate::document_cache::CompilerConfiguration,
    startup_config_overrides: SessionConfigOverrides,
    workspace_config_overrides: SessionConfigOverrides,
    active_project: ActiveProjectSelection,
}

impl EditorSession {
    pub fn new(document_cache: crate::DocumentCache, to_preview: Rc<crate::LspToPreviews>) -> Self {
        let compiler_config_defaults = document_cache.configuration_with_import_callback();
        Self {
            document_cache,
            preview_config: Default::default(),
            #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
            to_show: None,
            open_urls: Default::default(),
            to_preview,
            pending_recompile: Default::default(),
            compiler_config_defaults,
            startup_config_overrides: Default::default(),
            workspace_config_overrides: Default::default(),
            active_project: Default::default(),
        }
    }

    pub fn set_startup_config_overrides(&mut self, overrides: SessionConfigOverrides) {
        self.startup_config_overrides = overrides;
    }

    pub async fn set_workspace_config_overrides(
        &mut self,
        overrides: SessionConfigOverrides,
    ) -> crate::Result<VersionedDiagnostics> {
        self.workspace_config_overrides = overrides;
        self.reapply_effective_configuration().await
    }

    pub fn active_project_file_path(&self) -> Option<&std::path::Path> {
        match &self.active_project {
            ActiveProjectSelection::Project(active_project) => Some(&active_project.source_path),
            _ => None,
        }
    }

    fn effective_config_overrides(&self) -> SessionConfigOverrides {
        let mut overrides = self.startup_config_overrides.clone();
        overrides.merge(self.workspace_config_overrides.clone());
        overrides
    }

    fn project_baseline(project_file: ProjectFile) -> ProjectCompilerBaseline {
        let has_include_paths = project_file.include_directories().is_some();
        let has_library_paths = project_file.library_paths().is_some();
        let has_style = project_file.style().is_some();
        let enable_experimental = project_file.enable_experimental_features();
        let config = project_file.into_compiler_configuration(OutputFormat::Interpreter);

        ProjectCompilerBaseline {
            include_paths: has_include_paths.then_some(config.include_paths),
            library_paths: has_library_paths.then_some(config.library_paths),
            style: has_style.then_some(config.style).flatten(),
            enable_experimental,
        }
    }

    fn effective_compiler_configuration(&self) -> crate::document_cache::CompilerConfiguration {
        let mut config = self.compiler_config_defaults.clone();

        if let ActiveProjectSelection::Project(active_project) = &self.active_project
            && let Some(baseline) = &active_project.baseline
        {
            if let Some(include_paths) = &baseline.include_paths {
                config.include_paths = include_paths.clone();
            }
            if let Some(library_paths) = &baseline.library_paths {
                config.library_paths = library_paths.clone();
            }
            if let Some(style) = &baseline.style {
                config.style = Some(style.clone());
            }
            if let Some(enable_experimental) = baseline.enable_experimental {
                config.enable_experimental = enable_experimental;
            }
        }

        let overrides = self.effective_config_overrides();
        if let Some(include_paths) = overrides.include_paths {
            config.include_paths = include_paths;
        }
        if let Some(library_paths) = overrides.library_paths {
            config.library_paths = library_paths;
        }
        if let Some(style) = overrides.style {
            config.style = Some(style);
        }
        if let Some(experimental) = overrides.experimental {
            config.enable_experimental = experimental;
        }

        config
    }

    async fn reapply_effective_configuration(&mut self) -> crate::Result<VersionedDiagnostics> {
        let overrides = self.effective_config_overrides();
        let compiler_config = self.effective_compiler_configuration();
        let mut diagnostics = BuildDiagnostics::default();
        let (compiler_config, reloaded_files) =
            self.document_cache.reconfigure(compiler_config, &mut diagnostics).await;
        let extra_files = reloaded_files.iter().filter_map(crate::uri_to_file).collect();
        let diagnostics = collect_diagnostics(&self.document_cache, &extra_files, diagnostics);

        self.preview_config = PreviewConfig {
            hide_ui: overrides.hide_ui,
            style: compiler_config.style.clone().unwrap_or_default(),
            include_paths: compiler_config.include_paths.clone(),
            library_paths: compiler_config.library_paths.clone(),
            format_utf8: compiler_config.format == crate::ByteFormat::Utf8,
            enable_experimental: compiler_config.enable_experimental,
        };
        self.to_preview
            .send(&LspToPreviewMessage::SetConfiguration { config: self.preview_config.clone() });

        Ok(diagnostics)
    }

    fn discover_project_file_for_document_url(url: &Url) -> crate::Result<DiscoveredProjectFile> {
        let Some(document_path) = crate::uri_to_file(url) else {
            return Ok(DiscoveredProjectFile::None);
        };
        let Some(candidate) =
            crate::project_file_discovery::find_project_file_path_for_document_path(
                &document_path,
            )?
        else {
            return Ok(DiscoveredProjectFile::None);
        };

        match ProjectFile::load(&candidate) {
            Ok(project_file) => Ok(DiscoveredProjectFile::File(project_file)),
            Err(error) => Ok(DiscoveredProjectFile::Invalid { path: candidate, error }),
        }
    }

    async fn maybe_update_active_project(
        &mut self,
        url: &Url,
    ) -> crate::Result<VersionedDiagnostics> {
        let discovered_project = Self::discover_project_file_for_document_url(url)?;

        let update = match (&self.active_project, discovered_project) {
            (ActiveProjectSelection::Unset, DiscoveredProjectFile::File(project_file)) => {
                let source_path = project_file.source_path().to_path_buf();
                ActiveProjectUpdate::Reapply(ActiveProjectSelection::Project(ActiveProjectFile {
                    source_path,
                    baseline: Some(Self::project_baseline(project_file)),
                }))
            }
            (ActiveProjectSelection::Unset, DiscoveredProjectFile::None) => {
                ActiveProjectUpdate::Reapply(ActiveProjectSelection::NoProject)
            }
            (ActiveProjectSelection::NoProject, DiscoveredProjectFile::File(project_file)) => {
                tracing::warn!(
                    "Ignoring project file {} for {url}; active shared cache stays without project",
                    project_file.source_path().display()
                );
                ActiveProjectUpdate::Unchanged
            }
            (
                ActiveProjectSelection::Project(active_project),
                DiscoveredProjectFile::File(project_file),
            ) if active_project.source_path == project_file.source_path() => {
                let baseline = Self::project_baseline(project_file);
                if Some(&baseline) == active_project.baseline.as_ref() {
                    ActiveProjectUpdate::Unchanged
                } else {
                    ActiveProjectUpdate::Reapply(ActiveProjectSelection::Project(
                        ActiveProjectFile {
                            source_path: active_project.source_path.clone(),
                            baseline: Some(baseline),
                        },
                    ))
                }
            }
            (
                ActiveProjectSelection::Project(active_project),
                DiscoveredProjectFile::File(project_file),
            ) => {
                tracing::warn!(
                    "Ignoring project file {} for {url}; active shared cache uses {}",
                    project_file.source_path().display(),
                    active_project.source_path.display()
                );
                ActiveProjectUpdate::Unchanged
            }
            (
                ActiveProjectSelection::Project(active_project),
                DiscoveredProjectFile::Invalid { path, error },
            ) if active_project.baseline.is_none() && active_project.source_path == path => {
                tracing::warn!(
                    "Failed to reload active project file {} for {url}: {error}; keeping fallback state",
                    path.display()
                );
                ActiveProjectUpdate::Unchanged
            }
            (
                ActiveProjectSelection::Project(active_project),
                DiscoveredProjectFile::Invalid { path, .. },
            ) if active_project.source_path != path => {
                tracing::warn!(
                    "Ignoring invalid project file {} for {url}; active shared cache uses {}",
                    path.display(),
                    active_project.source_path.display()
                );
                ActiveProjectUpdate::Unchanged
            }
            (ActiveProjectSelection::Project(_), DiscoveredProjectFile::Invalid { error, .. })
            | (ActiveProjectSelection::NoProject, DiscoveredProjectFile::Invalid { error, .. })
            | (ActiveProjectSelection::Unset, DiscoveredProjectFile::Invalid { error, .. }) => {
                tracing::warn!("Project discovery for {url} failed: {error}");
                return Err(error);
            }
            _ => ActiveProjectUpdate::Unchanged,
        };

        match update {
            ActiveProjectUpdate::Unchanged => Ok(Default::default()),
            ActiveProjectUpdate::Reapply(active_project) => {
                self.active_project = active_project;
                self.reapply_effective_configuration().await
            }
        }
    }

    fn enqueue_configuration_recompile(&mut self) {
        self.pending_recompile.extend(self.open_urls.iter().cloned());
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        if let Some(preview_url) = self.to_show.as_ref().map(|component| component.url.clone()) {
            self.pending_recompile.insert(preview_url);
        }
    }

    async fn reload_active_project_file(
        &mut self,
        change: FileChangeKind,
    ) -> crate::Result<VersionedDiagnostics> {
        let ActiveProjectSelection::Project(active_project) = &self.active_project else {
            return Ok(Default::default());
        };
        let active_project_path = active_project.source_path.clone();

        let updated_selection = match change {
            FileChangeKind::Deleted => ActiveProjectSelection::Project(ActiveProjectFile {
                source_path: active_project_path,
                baseline: None,
            }),
            FileChangeKind::Changed | FileChangeKind::Created => {
                match ProjectFile::load(&active_project_path) {
                    Ok(project_file) => ActiveProjectSelection::Project(ActiveProjectFile {
                        source_path: active_project_path,
                        baseline: Some(Self::project_baseline(project_file)),
                    }),
                    Err(error) => {
                        tracing::warn!(
                            "Failed to reload active project file {}: {error}",
                            active_project_path.display()
                        );
                        ActiveProjectSelection::Project(ActiveProjectFile {
                            source_path: active_project_path,
                            baseline: None,
                        })
                    }
                }
            }
        };

        if updated_selection == self.active_project {
            return Ok(Default::default());
        }

        self.active_project = updated_selection;
        let diagnostics = self.reapply_effective_configuration().await?;
        self.enqueue_configuration_recompile();
        Ok(diagnostics)
    }

    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    pub fn send_state_to_preview(&self) {
        let mut doc_count = 0;
        #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
        let mut fonts_sent = HashSet::<PathBuf>::new();
        for (url, node) in self.document_cache.all_url_documents() {
            if url.scheme() == "builtin" {
                continue;
            }
            let version = self.document_cache.document_version(&url);

            self.to_preview.send(&LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(url.clone(), version),
                contents: node.text().to_string().into(),
            });
            #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
            self.send_referenced_fonts(&url, &mut fonts_sent);
            doc_count += 1;
        }

        self.to_preview
            .send(&LspToPreviewMessage::SetConfiguration { config: self.preview_config.clone() });

        if let Some(c) = self.to_show.clone() {
            tracing::debug!("Sending state to preview: {} documents, showing {}", doc_count, c.url);
            self.to_preview.send(&LspToPreviewMessage::ShowPreview(c));
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
        files: &[lsp_types::Url],
        allows_file: impl Fn(&std::path::Path) -> bool,
    ) {
        #[cfg(feature = "preview-remote")]
        let mut fonts_sent = HashSet::<PathBuf>::new();
        for url in files {
            if let Some(node) =
                self.document_cache.get_document(url).and_then(|doc| doc.node.as_ref())
            {
                let version = self.document_cache.document_version_by_path(node.source_file.path());
                let contents = node.text().to_string().into();
                tracing::debug!("Sending cached file {} to preview", url);
                self.to_preview.send(&LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(url.clone(), version),
                    contents,
                });
                #[cfg(feature = "preview-remote")]
                self.send_referenced_fonts(url, &mut fonts_sent);
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
                self.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
                continue;
            }
            match std::fs::read(&path) {
                Ok(contents) => {
                    tracing::debug!("Sending file {} ({} bytes) to preview", url, contents.len());
                    self.to_preview.send(&LspToPreviewMessage::SetContents {
                        url: VersionedUrl::new(url.clone(), None),
                        contents,
                    });
                }
                Err(err) => {
                    tracing::warn!("Failed to read file {}: {err}", path.display());
                    self.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
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
    fn send_referenced_fonts(&self, doc_url: &Url, sent: &mut HashSet<PathBuf>) {
        let Some(remote) = self.to_preview.remote() else { return };
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
    pub fn show_preview(&mut self, component: PreviewComponent) {
        self.pending_recompile.insert(component.url.clone());
        self.to_show = Some(component.clone());
        self.to_preview.send(&LspToPreviewMessage::ShowPreview(component));
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
                self.to_preview.send(&LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(url.clone(), version),
                    contents: content.clone().into(),
                });
                // Fonts imported before this edit were pushed to the remote viewer
                // already; seed the sent set with them so only fonts added by this
                // edit are transferred.
                #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
                let mut fonts_sent: HashSet<PathBuf> = self
                    .document_cache
                    .get_document(&url)
                    .map(|doc| {
                        doc.custom_fonts.iter().map(|(p, _)| PathBuf::from(p.as_str())).collect()
                    })
                    .unwrap_or_default();
                let dependencies: HashSet<Url> = self.document_cache.invalidate_url(&url);
                let _ = self.document_cache.load_url(&url, version, content, &mut diag).await;
                #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
                self.send_referenced_fonts(&url, &mut fonts_sent);
                dependencies
            }
            FileAction::IgnoreFile => return Default::default(),
            FileAction::InvalidateFile => {
                self.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
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
        let mut configuration_diagnostics = self.maybe_update_active_project(&url).await?;
        let (extra_files, diag) = self.load_document_impl(content, url.clone(), version).await;

        tracing::debug!("Loaded {url} with {} diagnostics", diag.iter().count());

        configuration_diagnostics.extend(collect_diagnostics(
            &self.document_cache,
            &extra_files,
            diag,
        ));
        Ok(configuration_diagnostics)
    }

    #[cfg_attr(target_arch = "wasm32", allow(unused))]
    pub async fn reload_document(
        &mut self,
        url: lsp_types::Url,
    ) -> crate::Result<crate::VersionedDiagnostics> {
        tracing::debug!("Reloading document: {url}");
        let mut configuration_diagnostics = self.maybe_update_active_project(&url).await?;

        // Check if document is in cache (can use reload_cached_file)
        let in_cache = self.document_cache.all_urls().contains(&url);

        if in_cache {
            tracing::trace!("Document is in cache, reloading: {url}");

            let mut diagnostics = BuildDiagnostics::default();

            self.document_cache.reload_cached_file(&url, &mut diagnostics).await;
            let mut extra_files = HashSet::new();
            extra_files.extend(crate::uri_to_file(&url));

            configuration_diagnostics.extend(collect_diagnostics(
                &self.document_cache,
                &extra_files,
                diagnostics,
            ));
            Ok(configuration_diagnostics)
        } else {
            tracing::trace!("Document not in cache, loading from disk: {url}");

            let Some(path) = crate::uri_to_file(&url) else {
                // The file was likely deleted, log and move on
                tracing::debug!("Failed to locate file: {url}");
                return Ok(configuration_diagnostics);
            };
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let (extra_files, diagnostics) =
                        self.load_document_impl(content, url, None).await;
                    configuration_diagnostics.extend(collect_diagnostics(
                        &self.document_cache,
                        &extra_files,
                        diagnostics,
                    ));
                    Ok(configuration_diagnostics)
                }
                // The file was likely deleted, log and move on
                Err(err) => {
                    tracing::debug!("Failed to read {} from disk: {err}", path.display());
                    Ok(configuration_diagnostics)
                }
            }
        }
    }

    fn drop_document_impl(&mut self, url: lsp_types::Url) -> crate::Result<()> {
        let dependencies = self.document_cache.drop_document(&url)?;

        let open_dependencies = self.open_urls.intersection(&dependencies).cloned();
        self.pending_recompile.extend(open_dependencies);

        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        if let Some(preview_url) = self.to_show.as_ref().map(|c| c.url.clone()) {
            // The external preview only has access to the files the LSP recompiled, so we need to
            // ensure the preview file is recompiled if anything it depends on changes, even if it's
            // not in the open_urls.
            if preview_url == url || dependencies.contains(&preview_url) {
                self.pending_recompile.insert(preview_url);
            }
        }

        Ok(())
    }

    pub async fn drop_document(&mut self, url: lsp_types::Url) -> crate::Result<()> {
        tracing::debug!("Dropping document: {url}");
        // The preview cares about resources and slint files, so forward everything
        self.to_preview.send(&LspToPreviewMessage::InvalidateContents { url: url.clone() });

        self.drop_document_impl(url)
    }

    pub async fn delete_document(
        &mut self,
        url: lsp_types::Url,
    ) -> crate::Result<crate::VersionedDiagnostics> {
        tracing::debug!("Deleting document: {url}");
        // The preview cares about resources and slint files, so forward everything
        self.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });

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
        if crate::uri_to_file(&url)
            .zip(self.active_project_file_path())
            .is_some_and(|(path, active_path)| path == active_path)
        {
            tracing::debug!("Active project file changed: {url} (type: {typ:?})");
            return self.reload_active_project_file(typ).await;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_compiler::project_file::FILE_NAME;
    use tempfile::TempDir;

    fn session() -> EditorSession {
        let config = crate::document_cache::CompilerConfiguration {
            style: Some("fluent".into()),
            ..Default::default()
        };
        EditorSession::new(
            crate::DocumentCache::new(config),
            crate::LspToPreviews::with_one(crate::DummyLspToPreview::default()),
        )
    }

    fn write_document(path: &std::path::Path) {
        std::fs::write(path, r#"export component Main inherits Window { }"#).unwrap();
    }

    fn load_document(session: &mut EditorSession, path: &std::path::Path) -> crate::Result<()> {
        let url = Url::from_file_path(path).unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        spin_on::spin_on(session.load_document(content, url, None)).map(|_| ())
    }

    fn open_document(session: &mut EditorSession, path: &std::path::Path) -> crate::Result<()> {
        let url = Url::from_file_path(path).unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        spin_on::spin_on(session.open_document(content, url, None)).map(|_| ())
    }

    #[test]
    fn discovers_nearest_project_file() {
        let temp = TempDir::new().unwrap();
        let top_project_path = temp.path().join(FILE_NAME);
        let nested_directory = temp.path().join("a/b");
        std::fs::create_dir_all(&nested_directory).unwrap();
        let nested_project_path = nested_directory.join(FILE_NAME);
        let document_path = nested_directory.join("main.slint");
        std::fs::write(&top_project_path, r#"{ "style": "cosmic" }"#).unwrap();
        std::fs::write(&nested_project_path, r#"{ "style": "material" }"#).unwrap();
        write_document(&document_path);

        let mut session = session();
        load_document(&mut session, &document_path).unwrap();

        assert_eq!(session.active_project_file_path(), Some(nested_project_path.as_path()));
        assert_eq!(session.preview_config.style, "material");
    }

    #[test]
    fn omitted_project_settings_keep_session_defaults() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join(FILE_NAME);
        let document_path = temp.path().join("main.slint");
        std::fs::write(&project_path, r#"{ "include-directories": ["include"] }"#).unwrap();
        write_document(&document_path);

        let mut session = session();
        load_document(&mut session, &document_path).unwrap();

        assert_eq!(session.preview_config.style, "fluent");
        assert_eq!(session.preview_config.include_paths, vec![temp.path().join("include")]);
    }

    #[test]
    fn config_overrides_take_precedence_over_project_file() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join(FILE_NAME);
        let document_path = temp.path().join("main.slint");
        std::fs::write(&project_path, r#"{ "style": "material" }"#).unwrap();
        write_document(&document_path);

        let mut session = session();
        session.set_startup_config_overrides(SessionConfigOverrides {
            style: Some("cosmic".into()),
            experimental: Some(true),
            ..Default::default()
        });
        load_document(&mut session, &document_path).unwrap();
        assert_eq!(session.preview_config.style, "cosmic");
        assert!(session.preview_config.enable_experimental);

        spin_on::spin_on(session.set_workspace_config_overrides(SessionConfigOverrides {
            style: Some("cupertino".into()),
            experimental: Some(false),
            ..Default::default()
        }))
        .unwrap();
        assert_eq!(session.preview_config.style, "cupertino");
        assert!(!session.preview_config.enable_experimental);

        spin_on::spin_on(session.set_workspace_config_overrides(Default::default())).unwrap();
        assert_eq!(session.preview_config.style, "cosmic");
        assert!(session.preview_config.enable_experimental);
    }

    #[test]
    fn shared_cache_keeps_first_project_file() {
        let temp = TempDir::new().unwrap();
        let project_a = temp.path().join("project-a");
        let project_b = temp.path().join("project-b");
        std::fs::create_dir_all(&project_a).unwrap();
        std::fs::create_dir_all(&project_b).unwrap();
        let project_a_file = project_a.join(FILE_NAME);
        let project_b_file = project_b.join(FILE_NAME);
        let document_a = project_a.join("main.slint");
        let document_b = project_b.join("main.slint");
        std::fs::write(&project_a_file, r#"{ "style": "material" }"#).unwrap();
        std::fs::write(&project_b_file, r#"{ "style": "cupertino" }"#).unwrap();
        write_document(&document_a);
        write_document(&document_b);

        let mut session = session();
        load_document(&mut session, &document_a).unwrap();
        load_document(&mut session, &document_b).unwrap();

        assert_eq!(session.active_project_file_path(), Some(project_a_file.as_path()));
        assert_eq!(session.preview_config.style, "material");
    }

    #[test]
    fn invalid_initial_project_file_prevents_document_load() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join(FILE_NAME);
        let document_path = temp.path().join("main.slint");
        std::fs::write(&project_path, "{").unwrap();
        write_document(&document_path);

        let mut session = session();
        assert!(load_document(&mut session, &document_path).is_err());
        assert_eq!(session.active_project_file_path(), None);
    }

    #[test]
    fn active_project_file_recovers_after_invalid_change() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join(FILE_NAME);
        let document_path = temp.path().join("main.slint");
        std::fs::write(&project_path, r#"{ "style": "material" }"#).unwrap();
        write_document(&document_path);

        let mut session = session();
        open_document(&mut session, &document_path).unwrap();
        let document_url = Url::from_file_path(&document_path).unwrap();
        let project_url = Url::from_file_path(&project_path).unwrap();

        std::fs::write(&project_path, "{").unwrap();
        spin_on::spin_on(
            session.trigger_file_watcher(project_url.clone(), FileChangeKind::Changed),
        )
        .unwrap();

        assert_eq!(session.preview_config.style, "fluent");
        assert_eq!(session.active_project_file_path(), Some(project_path.as_path()));
        assert!(session.pending_recompile.contains(&document_url));

        std::fs::write(&project_path, r#"{ "style": "cupertino" }"#).unwrap();
        spin_on::spin_on(session.trigger_file_watcher(project_url, FileChangeKind::Changed))
            .unwrap();

        assert_eq!(session.preview_config.style, "cupertino");
        assert!(session.pending_recompile.contains(&document_url));
    }

    #[test]
    fn deleted_active_project_file_remains_selected() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join(FILE_NAME);
        let document_path = temp.path().join("main.slint");
        std::fs::write(&project_path, r#"{ "style": "material" }"#).unwrap();
        write_document(&document_path);

        let mut session = session();
        open_document(&mut session, &document_path).unwrap();
        let project_url = Url::from_file_path(&project_path).unwrap();

        std::fs::remove_file(&project_path).unwrap();
        spin_on::spin_on(
            session.trigger_file_watcher(project_url.clone(), FileChangeKind::Deleted),
        )
        .unwrap();

        assert_eq!(session.active_project_file_path(), Some(project_path.as_path()));
        assert_eq!(session.preview_config.style, "fluent");

        std::fs::write(&project_path, r#"{ "style": "cupertino" }"#).unwrap();
        spin_on::spin_on(session.trigger_file_watcher(project_url, FileChangeKind::Created))
            .unwrap();

        assert_eq!(session.preview_config.style, "cupertino");
    }
}
