// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The state of a Slint project as it is being edited, shared between the
//! language server and the visual editor.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::project_file::{Overrides, ProjectFile, ProjectFileData};
#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
use i_slint_live_preview::protocol::PreviewComponent;
use i_slint_live_preview::{
    file_watcher::FileChangeKind,
    protocol::{LspToPreviewMessage, PreviewConfig, SourceFileVersion, VersionedUrl},
};
use lsp_types::Url;

use i_slint_compiler::source_path::SourcePath;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

/// Diagnostics paired with the document version for which they were computed.
pub type VersionedDiagnostics = Vec<(Url, SourceFileVersion, Vec<lsp_types::Diagnostic>)>;

pub struct PreviewConnection {
    pub to_preview: Rc<crate::LspToPreviews>,
    /// The last component for which the user clicked "show preview"
    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    pub to_show: Option<PreviewComponent>,
}

#[derive(Clone, Debug, Default)]
pub struct SessionConfigOverrides {
    pub hide_ui: Option<bool>,
    pub compiler: Overrides,
}

impl SessionConfigOverrides {
    fn merge(&mut self, other: Self) {
        if other.hide_ui.is_some() {
            self.hide_ui = other.hide_ui;
        }
        self.compiler.merge(other.compiler);
    }
}

/// Inserts `project` so that the shallowest project file comes first.
/// It likely covers the most open documents, so it wins a conflict.
fn insert_broadest_first(projects: &mut Vec<PathBuf>, project: PathBuf) {
    let depth = |path: &std::path::Path| path.components().count();
    let position = projects
        .iter()
        .position(|existing| depth(existing) > depth(&project))
        .unwrap_or(projects.len());
    projects.insert(position, project);
}

fn keep_first<T: Clone + PartialEq + std::fmt::Display>(
    kept: &mut Option<(T, String)>,
    value: Option<T>,
    source: &str,
    setting: &str,
    warn: &mut impl FnMut(String, String, &str),
) {
    let Some(value) = value else { return };
    let (kept_value, kept_source) = kept.get_or_insert_with(|| (value.clone(), source.into()));
    if kept_value != &value {
        warn(format!("{setting} '{value}'"), format!("{setting} '{kept_value}'"), kept_source);
    }
}

fn merge_project_include_paths(
    include_paths: &mut Option<Vec<PathBuf>>,
    project_include_paths: Option<Vec<PathBuf>>,
    override_source: Option<&str>,
    warn: &mut impl FnMut(String, String, &str),
) {
    let Some(paths) = project_include_paths else { return };
    let include_paths = include_paths.get_or_insert_default();
    for path in paths {
        if !include_paths.contains(&path) {
            if let Some(source) = override_source {
                warn(
                    format!("include path '{}'", path.display()),
                    format!("include paths {include_paths:?}"),
                    source,
                );
            } else {
                include_paths.push(path);
            }
        }
    }
}

fn merge_project_library_paths(
    library_paths: &mut Option<HashMap<String, (PathBuf, String)>>,
    project_library_paths: Option<HashMap<String, PathBuf>>,
    override_source: Option<&str>,
    project_source: &str,
    warn: &mut impl FnMut(String, String, &str),
) {
    let Some(paths) = project_library_paths else { return };
    let library_paths = library_paths.get_or_insert_default();
    let mut paths = paths.into_iter().collect::<Vec<_>>();
    paths.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (name, path) in paths {
        if let Some(source) = override_source {
            if !library_paths.contains_key(&name) {
                warn(
                    format!("library '{name}' at '{}'", path.display()),
                    format!("no library '{name}'"),
                    source,
                );
                continue;
            }
        }
        let (kept_path, kept_source) = library_paths
            .entry(name.clone())
            .or_insert_with(|| (path.clone(), project_source.to_string()));
        if kept_path != &path {
            warn(
                format!("library '{name}' at '{}'", path.display()),
                format!("library '{name}' at '{}'", kept_path.display()),
                kept_source,
            );
        }
    }
}

enum DiscoveredProjectFile {
    None,
    File(ProjectFile),
    Invalid { path: PathBuf, error: Box<dyn std::error::Error> },
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
    compiler_config_defaults: crate::document_cache::CompilerConfiguration,
    startup_config_overrides: SessionConfigOverrides,
    workspace_config_overrides: SessionConfigOverrides,
    active_projects: HashMap<PathBuf, ProjectFile>,
    /// The project files of the open documents, broadest first.
    /// A shared document cache has one configuration, so their settings are merged.
    project_file_paths: Vec<PathBuf>,
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

    pub fn new(document_cache: crate::DocumentCache, to_preview: Rc<crate::LspToPreviews>) -> Self {
        Self::with_previews(
            document_cache,
            vec![PreviewConnection {
                to_preview,
                #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
                to_show: None,
            }],
        )
    }

    pub fn with_previews(
        document_cache: crate::DocumentCache,
        previews: Vec<PreviewConnection>,
    ) -> Self {
        let compiler_config_defaults = document_cache.configuration_with_import_callback();
        Self {
            document_cache,
            preview_config: Default::default(),
            open_urls: Default::default(),
            previews,
            pending_recompile: Default::default(),
            compiler_config_defaults,
            startup_config_overrides: Default::default(),
            workspace_config_overrides: Default::default(),
            active_projects: Default::default(),
            project_file_paths: Default::default(),
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

    pub fn active_project_file_paths(&self) -> impl Iterator<Item = &std::path::Path> {
        self.project_file_paths.iter().map(PathBuf::as_path)
    }

    fn effective_config_overrides(&self) -> SessionConfigOverrides {
        let mut overrides = self.startup_config_overrides.clone();
        overrides.merge(self.workspace_config_overrides.clone());
        overrides
    }

    fn merged_project_data(
        &self,
        overrides: ProjectFileData,
    ) -> (ProjectFileData, VersionedDiagnostics) {
        let ProjectFileData {
            mut include_paths,
            library_paths,
            style,
            enable_experimental_features,
            schema: _,
            entry: _,
        } = overrides;
        let override_source = |workspace_supplied| {
            if workspace_supplied {
                "workspace configuration override"
            } else {
                "startup configuration override"
            }
        };
        let workspace = &self.workspace_config_overrides.compiler.project;
        let include_source =
            include_paths.as_ref().map(|_| override_source(workspace.include_paths.is_some()));
        let library_source =
            library_paths.as_ref().map(|_| override_source(workspace.library_paths.is_some()));
        let mut library_paths = library_paths.map(|paths| {
            paths
                .into_iter()
                .map(|(name, path)| (name, (path, library_source.unwrap().to_string())))
                .collect::<HashMap<_, _>>()
        });
        let mut style =
            style.map(|value| (value, override_source(workspace.style.is_some()).to_string()));
        let mut enable_experimental = enable_experimental_features.map(|value| {
            (value, override_source(workspace.enable_experimental_features.is_some()).to_string())
        });
        let mut project_diagnostics = Vec::new();

        for path in &self.project_file_paths {
            let Ok(url) = Url::from_file_path(path) else { continue };
            let mut diagnostics = Vec::new();
            if let Some(project) = self.active_projects.get(path) {
                let ProjectFileData {
                    include_paths: project_include_paths,
                    library_paths: project_library_paths,
                    style: project_style,
                    enable_experimental_features: project_enable_experimental,
                    schema: _,
                    entry: _,
                } = project.resolved_data();
                let source = format!("project file {}", project.source_path().display());
                let mut warn = |setting: String, selected: String, source: &str| {
                    diagnostics.push(lsp_types::Diagnostic {
                        range: lsp_types::Range::default(),
                        severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                        source: Some("slint".into()),
                        message: format!(
                            "slint-lsp is ignoring {setting}.\nUsing {selected} from {source}\nThis is a restriction in slint-lsp - compilation will work normally."
                        ),
                        ..Default::default()
                    });
                };
                merge_project_include_paths(
                    &mut include_paths,
                    project_include_paths,
                    include_source,
                    &mut warn,
                );
                merge_project_library_paths(
                    &mut library_paths,
                    project_library_paths,
                    library_source,
                    &source,
                    &mut warn,
                );
                keep_first(&mut style, project_style, &source, "style", &mut warn);
                keep_first(
                    &mut enable_experimental,
                    project_enable_experimental,
                    &source,
                    "experimental features",
                    &mut warn,
                );
            }
            project_diagnostics.push((
                url.clone(),
                self.document_cache.document_version(&url),
                diagnostics,
            ));
        }

        (
            ProjectFileData {
                include_paths,
                library_paths: library_paths
                    .map(|paths| paths.into_iter().map(|(name, (path, _))| (name, path)).collect()),
                style: style.map(|(style, _)| style),
                enable_experimental_features: enable_experimental.map(|(enabled, _)| enabled),
                schema: None,
                entry: None,
            },
            project_diagnostics,
        )
    }

    fn effective_compiler_configuration(
        &self,
    ) -> (crate::document_cache::CompilerConfiguration, VersionedDiagnostics) {
        let mut config = self.compiler_config_defaults.clone();
        let mut overrides = self.effective_config_overrides().compiler;
        let (project_data, diagnostics) =
            self.merged_project_data(std::mem::take(&mut overrides.project));
        project_data.apply_to(&mut config.compiler_config);
        overrides.apply(None, &mut config.compiler_config);
        (config, diagnostics)
    }

    async fn reapply_effective_configuration(&mut self) -> crate::Result<VersionedDiagnostics> {
        let overrides = self.effective_config_overrides();
        let (compiler_config, project_diagnostics) = self.effective_compiler_configuration();
        let mut diagnostics = BuildDiagnostics::default();
        let roots = self.document_roots();
        let (compiler_config, reloaded_files) =
            self.document_cache.reconfigure(compiler_config, &roots, &mut diagnostics).await;
        let mut extra_files =
            reloaded_files.iter().map(SourcePath::from_url).collect::<HashSet<_>>();
        for url in roots.difference(&reloaded_files) {
            if !self.document_is_open(url) {
                extra_files.extend(
                    self.reload_document_without_project_discovery(url.clone(), &mut diagnostics)
                        .await,
                );
            }
        }
        let removed = self.document_cache.retain_documents(&roots)?;
        self.pending_recompile.retain(|url| roots.contains(url));
        extra_files.extend(roots.iter().chain(&removed).map(SourcePath::from_url));
        let mut diagnostics = collect_diagnostics(&self.document_cache, &extra_files, diagnostics);
        diagnostics.extend(removed.into_iter().map(|url| (url, None, Vec::new())));
        diagnostics.extend(project_diagnostics);

        self.preview_config = PreviewConfig {
            hide_ui: overrides.hide_ui,
            style: compiler_config.compiler_config.style.clone().unwrap_or_default(),
            include_paths: compiler_config.compiler_config.include_paths.clone(),
            library_paths: compiler_config.compiler_config.library_paths.clone(),
            format_utf8: compiler_config.format == crate::ByteFormat::Utf8,
            enable_experimental: compiler_config.compiler_config.enable_experimental,
        };
        self.send_to_previews(&LspToPreviewMessage::SetConfiguration {
            config: self.preview_config.clone(),
        });

        Ok(diagnostics)
    }

    fn discover_project_file_for_document_url(url: &Url) -> crate::Result<DiscoveredProjectFile> {
        let Some(document_path) = SourcePath::from_url(url).into_native_path() else {
            return Ok(DiscoveredProjectFile::None);
        };
        let directory =
            SourcePath::new(&document_path).parent().into_native_path().unwrap_or_default();
        let Some(candidate) = i_slint_compiler::project_file::find_project_file_path(&directory)?
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
        let reapply = match Self::discover_project_file_for_document_url(url)? {
            DiscoveredProjectFile::None => false,
            DiscoveredProjectFile::File(project_file) => {
                let source_path = project_file.source_path().to_path_buf();
                if !self.project_file_paths.contains(&source_path) {
                    insert_broadest_first(&mut self.project_file_paths, source_path.clone());
                }
                match self.active_projects.get_mut(&source_path) {
                    Some(known) if known == &project_file => false,
                    Some(known) => {
                        *known = project_file;
                        true
                    }
                    None => {
                        self.active_projects.insert(source_path, project_file);
                        true
                    }
                }
            }
            DiscoveredProjectFile::Invalid { path, error } => {
                if self.active_project_file_paths().any(|known| known == path) {
                    tracing::warn!(
                        "Failed to reload project file {} for {url}: {error}; keeping fallback state",
                        path.display()
                    );
                    false
                } else {
                    tracing::warn!("Project discovery for {url} failed: {error}");
                    return Err(error);
                }
            }
        };

        if !reapply {
            return Ok(Default::default());
        }
        self.reapply_effective_configuration().await
    }

    fn project_entry_urls(&self) -> impl Iterator<Item = Url> + '_ {
        self.active_projects
            .values()
            .filter_map(|project| Url::from_file_path(project.entry()?).ok())
    }

    fn document_roots(&self) -> HashSet<Url> {
        let mut roots = self.open_urls.clone();
        roots.extend(self.project_entry_urls());
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        roots.extend(
            self.previews.iter().filter_map(|preview| {
                preview.to_show.as_ref().map(|component| component.url.clone())
            }),
        );
        roots.into_iter().filter_map(|url| SourcePath::from_url(&url).to_url()).collect()
    }

    fn document_is_open(&self, url: &Url) -> bool {
        let path = SourcePath::from_url(url);
        self.open_urls.iter().any(|open_url| SourcePath::from_url(open_url) == path)
    }

    fn enqueue_configuration_recompile(&mut self) {
        self.pending_recompile.extend(self.document_roots());
    }

    async fn reload_active_project_file(
        &mut self,
        path: &std::path::Path,
        change: FileChangeKind,
    ) -> crate::Result<VersionedDiagnostics> {
        if !self.active_project_file_paths().any(|known| known == path) {
            return Ok(Default::default());
        }
        match change {
            FileChangeKind::Deleted => {
                self.active_projects.remove(path);
                self.project_file_paths.retain(|known| known != path);
            }
            FileChangeKind::Changed | FileChangeKind::Created => match ProjectFile::load(path) {
                Ok(project_file) => {
                    if self.active_projects.get(path) == Some(&project_file) {
                        return Ok(Default::default());
                    }
                    self.active_projects.insert(path.to_path_buf(), project_file);
                }
                Err(error) => {
                    tracing::warn!(
                        "Failed to reload active project file {}: {error}",
                        path.display()
                    );
                    if self.active_projects.remove(path).is_none() {
                        return Ok(Default::default());
                    }
                }
            },
        }
        let mut diagnostics = self.reapply_effective_configuration().await?;
        if matches!(change, FileChangeKind::Deleted) {
            if let Ok(url) = Url::from_file_path(path) {
                diagnostics.push((
                    url.clone(),
                    self.document_cache.document_version(&url),
                    Vec::new(),
                ));
            }
        }
        self.enqueue_configuration_recompile();
        Ok(diagnostics)
    }

    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    pub fn send_state_to_preview(&self, preview_index: usize) {
        let Some(preview) = self.preview(preview_index) else { return };
        let mut doc_count = 0;
        #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
        let mut fonts_sent = HashSet::<SourcePath>::new();
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
        let mut fonts_sent = HashSet::<SourcePath>::new();
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
        sent: &mut HashSet<SourcePath>,
    ) {
        let Some(remote) = preview.to_preview.remote() else { return };
        let Some(doc) = self.document_cache.get_document(doc_url) else { return };
        // `custom_fonts` holds the resolved path of every font import that
        // passed the compiler's existence check, plus remote URLs.
        for (font_path, _) in &doc.custom_fonts {
            if font_path.as_native_path().is_none() || !sent.insert(font_path.clone()) {
                continue;
            }
            let Some(font_url) = font_path.to_url() else {
                tracing::warn!("Cannot convert font path to URL: {font_path}");
                continue;
            };
            match font_path.read() {
                Ok(contents) => {
                    tracing::debug!(
                        "Sending font {} ({} bytes) to remote viewer",
                        font_url,
                        contents.len()
                    );
                    remote.send(&LspToPreviewMessage::SetContents {
                        url: VersionedUrl::new(font_url, None),
                        contents: contents.into_owned(),
                    });
                }
                Err(err) => {
                    tracing::warn!("Failed to read font {font_path}: {err}");
                }
            }
        }
    }

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    pub fn show_preview(&mut self, preview_index: usize, component: PreviewComponent) {
        let component_url =
            SourcePath::from_url(&component.url).to_url().unwrap_or_else(|| component.url.clone());
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
    ) -> (HashSet<SourcePath>, BuildDiagnostics) {
        enum FileAction {
            ProcessContent(String),
            IgnoreFile,
            InvalidateFile,
        }

        tracing::trace!("Loading document: {url} (version: {version:?})");

        let path = SourcePath::from_url(&url);
        // Normalize the URL
        let Some(url) = path.to_url() else { return Default::default() };

        let action = if path.extension() == Some("rs") {
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
                let fonts_sent: HashSet<SourcePath> = self
                    .document_cache
                    .get_document(&url)
                    .map(|doc| doc.custom_fonts.iter().map(|(p, _)| p.clone()).collect())
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

        self.reload_dependent_roots(&dependencies, &mut diag).await;

        let extra_files =
            dependencies.iter().map(SourcePath::from_url).chain(core::iter::once(path)).collect();

        (extra_files, diag)
    }

    async fn reload_dependent_roots(
        &mut self,
        dependencies: &HashSet<Url>,
        diagnostics: &mut BuildDiagnostics,
    ) {
        for url in self.document_roots().intersection(dependencies) {
            self.document_cache.reload_cached_file(url, diagnostics).await;
        }
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
        self.drop_document(url.clone()).await?;
        if self
            .document_roots()
            .iter()
            .any(|root| SourcePath::from_url(root) == SourcePath::from_url(&url))
        {
            self.reload_document_without_project_discovery(url, &mut BuildDiagnostics::default())
                .await;
        }
        Ok(())
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
        let url = SourcePath::from_url(&url).to_url().unwrap_or(url);
        let mut configuration_diagnostics = if self.project_entry_urls().any(|entry| entry == url) {
            Vec::new()
        } else {
            self.maybe_update_active_project(&url).await?
        };

        let mut diagnostics = BuildDiagnostics::default();
        let extra_files =
            self.reload_document_without_project_discovery(url, &mut diagnostics).await;
        configuration_diagnostics.extend(collect_diagnostics(
            &self.document_cache,
            &extra_files,
            diagnostics,
        ));
        Ok(configuration_diagnostics)
    }

    async fn reload_document_without_project_discovery(
        &mut self,
        url: Url,
        diagnostics: &mut BuildDiagnostics,
    ) -> HashSet<SourcePath> {
        let path = SourcePath::from_url(&url);
        let Some(url) = path.to_url() else { return HashSet::new() };
        let mut extra_files = HashSet::from([path.clone()]);
        // Check if document is in cache (can use reload_cached_file)
        let in_cache = self.document_cache.all_urls().any(|cached| cached == url);
        if in_cache {
            let dependencies = if self.document_cache.get_document(&url).is_some() {
                self.document_cache.invalidate_url(&url)
            } else {
                HashSet::new()
            };
            self.document_cache.reload_cached_file(&url, diagnostics).await;
            self.reload_dependent_roots(&dependencies, diagnostics).await;
            extra_files.extend(dependencies.iter().map(SourcePath::from_url));
        } else {
            match path.read_to_string() {
                Ok(content) => {
                    let (dependent_files, loaded_diagnostics) =
                        self.load_document_impl(content, url, None).await;
                    extra_files.extend(dependent_files);
                    diagnostics
                        .all_loaded_files
                        .extend(loaded_diagnostics.all_loaded_files.clone());
                    for diagnostic in loaded_diagnostics {
                        diagnostics.push_compiler_error(diagnostic);
                    }
                }
                Err(error) => {
                    tracing::debug!("Failed to read {path} from disk: {error}")
                }
            }
        }
        extra_files
    }

    fn drop_document_impl(&mut self, url: lsp_types::Url) -> crate::Result<()> {
        let url = SourcePath::from_url(&url).to_url().unwrap_or(url);
        let dependencies = self.document_cache.drop_document(&url)?;

        self.pending_recompile.extend(
            self.document_roots()
                .into_iter()
                .filter(|root| root == &url || dependencies.contains(root)),
        );

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
        if let Some(path) = SourcePath::from_url(&url).into_native_path()
            && self.active_project_file_paths().any(|active_path| active_path == path)
        {
            tracing::debug!("Active project file changed: {url} (type: {typ:?})");
            return self.reload_active_project_file(&path, typ).await;
        }

        if !self.document_is_open(&url) {
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
    extra_files: &HashSet<SourcePath>,
    diag: BuildDiagnostics,
    format: crate::ByteFormat,
) -> HashMap<Url, Vec<lsp_types::Diagnostic>> {
    // Always provide diagnostics for all files. Empty diagnostics clear any previous ones.
    let mut lsp_diags: HashMap<Url, Vec<lsp_types::Diagnostic>> = extra_files
        .iter()
        .filter_map(SourcePath::to_url)
        .chain(diag.all_loaded_files.iter().filter_map(SourcePath::to_url))
        .map(|uri| (uri, Default::default()))
        .collect();

    for d in diag.into_iter() {
        let Some(uri) = i_slint_live_preview::protocol::diagnostic_url(&d) else {
            continue;
        };
        lsp_diags
            .entry(uri)
            .or_default()
            .push(i_slint_live_preview::protocol::to_lsp_diagnostic(&d, format));
    }

    lsp_diags
}

pub fn collect_diagnostics(
    document_cache: &crate::DocumentCache,
    extra_files: &HashSet<SourcePath>,
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
mod preview_tests {
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
        let session = EditorSession::with_previews(crate::test::empty_document_cache(), previews);
        (session, messages)
    }

    #[test]
    fn primary_preview_accessors_return_the_first_connection() {
        let (mut session, _) = session_with_recording_previews();
        let component = PreviewComponent {
            url: crate::test::test_file_name("primary.slint").to_url().unwrap(),
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
            url: crate::test::test_file_name("missing.slint").to_url().unwrap(),
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
        let invalidated_url = crate::test::test_file_name("invalidated.slint").to_url().unwrap();
        let deleted_url = crate::test::test_file_name("deleted.slint").to_url().unwrap();

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

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_compiler::project_file::FILE_NAME;
    use tempfile::TempDir;

    fn session() -> EditorSession {
        let config = crate::document_cache::CompilerConfiguration {
            compiler_config: {
                let mut compiler_config =
                    crate::document_cache::CompilerConfiguration::default().compiler_config;
                compiler_config.style = Some("fluent".into());
                compiler_config
            },
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

    fn cached_text(session: &EditorSession, path: &std::path::Path) -> Option<String> {
        session
            .document_cache
            .get_document(&Url::from_file_path(path).unwrap())
            .and_then(|document| document.node.as_ref())
            .map(|node| node.text().to_string())
    }

    #[test]
    fn configuration_changes_preserve_open_documents_with_encoded_urls() {
        let temp = TempDir::new().unwrap();
        let document = temp.path().join("editing.slint");
        write_document(&document);
        let normalized_url = Url::from_file_path(&document).unwrap();
        let encoded_url =
            Url::parse(&normalized_url.as_str().replace("editing.slint", "editing.%73lint"))
                .unwrap();
        assert_ne!(encoded_url, normalized_url);
        let mut session = session();
        spin_on::spin_on(session.open_document(
            "export component Unsaved {}".into(),
            encoded_url.clone(),
            Some(7),
        ))
        .unwrap();
        spin_on::spin_on(session.reapply_effective_configuration()).unwrap();
        assert!(cached_text(&session, &document).unwrap().contains("Unsaved"));
        assert_eq!(session.document_cache.document_version(&encoded_url), Some(7));
        assert_eq!(session.document_roots(), HashSet::from([normalized_url]));
        spin_on::spin_on(session.trigger_file_watcher(
            Url::from_file_path(&document).unwrap(),
            FileChangeKind::Changed,
        ))
        .unwrap();
        assert!(cached_text(&session, &document).unwrap().contains("Unsaved"));
    }

    #[test]
    fn discovering_a_project_uses_editor_contents_before_disk_contents() {
        let temp = TempDir::new().unwrap();
        let document = temp.path().join("editing.slint");
        let disk_dependency = temp.path().join("disk-only.slint");
        std::fs::write(temp.path().join(FILE_NAME), "{}").unwrap();
        std::fs::write(&document, r#"import { DiskOnly } from "disk-only.slint"; export component Main inherits DiskOnly {}"#).unwrap();
        std::fs::write(&disk_dependency, "export component DiskOnly inherits DoesNotExist {}")
            .unwrap();
        let mut session = session();
        let diagnostics = spin_on::spin_on(session.open_document(
            "export component Unsaved {}".into(),
            Url::from_file_path(&document).unwrap(),
            Some(7),
        ))
        .unwrap();
        assert!(cached_text(&session, &document).unwrap().contains("Unsaved"));
        assert!(cached_text(&session, &disk_dependency).is_none());
        assert!(
            !diagnostics
                .iter()
                .any(|(url, _, _)| url == &Url::from_file_path(&disk_dependency).unwrap())
        );
    }

    #[test]
    fn loading_a_new_disk_file_clears_invalidated_dependent_diagnostics() {
        let temp = TempDir::new().unwrap();
        let dependency = temp.path().join("new.slint");
        let dependent = temp.path().join("dependent.slint");
        let dependent_url = Url::from_file_path(&dependent).unwrap();
        let mut session = session();
        spin_on::spin_on(session.load_document_impl(
            r#"import { New } from "new.slint"; export component Main inherits New {}"#.into(),
            dependent_url.clone(),
            None,
        ));
        write_document(&dependency);
        let diagnostics =
            spin_on::spin_on(session.reload_document(Url::from_file_path(&dependency).unwrap()))
                .unwrap();
        assert!(
            diagnostics
                .iter()
                .any(|(url, _, diagnostics)| url == &dependent_url && diagnostics.is_empty())
        );
    }

    #[test]
    fn project_entries_load_imports_without_discovering_other_projects() {
        let temp = TempDir::new().unwrap();
        let entry_directory = temp.path().join("entry");
        std::fs::create_dir(&entry_directory).unwrap();
        let project = temp.path().join(FILE_NAME);
        let document = temp.path().join("editing.slint");
        let entry = entry_directory.join("main.slint");
        let dependency = entry_directory.join("dependency.slint");
        std::fs::write(&project, r#"{ "entry": "entry/main.slint", "style": "fluent" }"#).unwrap();
        std::fs::write(entry_directory.join(FILE_NAME), r#"{ "style": "material" }"#).unwrap();
        std::fs::write(&entry, r#"import { Dependency } from "dependency.slint"; export component Main inherits Dependency {}"#).unwrap();
        std::fs::write(&dependency, "export component Dependency {}").unwrap();
        write_document(&document);
        let mut session = session();
        open_document(&mut session, &document).unwrap();
        for path in [&entry, &dependency] {
            assert!(cached_text(&session, path).is_some());
            assert!(!session.open_urls.contains(&Url::from_file_path(path).unwrap()));
        }
        assert_eq!(session.active_project_file_paths().collect::<Vec<_>>(), [project.as_path()]);
        assert_eq!(session.preview_config.style, "fluent");

        std::fs::write(&dependency, "export component Changed {}").unwrap();
        spin_on::spin_on(session.trigger_file_watcher(
            Url::from_file_path(&dependency).unwrap(),
            FileChangeKind::Changed,
        ))
        .unwrap();
        let entry_url = Url::from_file_path(&entry).unwrap();
        assert!(session.pending_recompile.contains(&entry_url));
        let encoded_url =
            Url::parse(&entry_url.as_str().replace("main.slint", "main.%73lint")).unwrap();
        let diagnostics = spin_on::spin_on(session.reload_document(encoded_url)).unwrap();
        assert!(diagnostics.iter().any(|(_, _, diagnostics)| {
            diagnostics.iter().any(|diagnostic| diagnostic.message.contains("Dependency"))
        }));
        assert!(cached_text(&session, &dependency).unwrap().contains("Changed"));
        assert_eq!(session.active_project_file_paths().collect::<Vec<_>>(), [project.as_path()]);
    }

    #[test]
    fn closing_an_entry_restores_disk_contents_and_version() {
        let temp = TempDir::new().unwrap();
        let entry = temp.path().join("main.slint");
        let document = temp.path().join("editing.slint");
        std::fs::write(temp.path().join(FILE_NAME), r#"{ "entry": "main.slint" }"#).unwrap();
        write_document(&entry);
        write_document(&document);
        let mut session = session();
        open_document(&mut session, &document).unwrap();
        let entry_url = Url::from_file_path(&entry).unwrap();
        spin_on::spin_on(session.open_document(
            "export component Unsaved {}".into(),
            entry_url.clone(),
            Some(42),
        ))
        .unwrap();
        spin_on::spin_on(session.reapply_effective_configuration()).unwrap();
        assert!(cached_text(&session, &entry).unwrap().contains("Unsaved"));
        spin_on::spin_on(session.close_document(entry_url.clone())).unwrap();
        assert!(cached_text(&session, &entry).unwrap().contains("Main"));
        assert_eq!(session.document_cache.document_version(&entry_url), None);
        assert!(!session.open_urls.contains(&entry_url));
    }

    #[test]
    fn closing_an_entry_reports_errors_from_disk_on_queued_reload() {
        let temp = TempDir::new().unwrap();
        let entry = temp.path().join("main.slint");
        std::fs::write(temp.path().join(FILE_NAME), r#"{ "entry": "main.slint" }"#).unwrap();
        std::fs::write(&entry, "export component Main inherits DoesNotExist {}").unwrap();
        let entry_url = Url::from_file_path(&entry).unwrap();
        let mut session = session();
        spin_on::spin_on(session.open_document(
            "export component Unsaved {}".into(),
            entry_url.clone(),
            Some(3),
        ))
        .unwrap();
        spin_on::spin_on(session.close_document(entry_url.clone())).unwrap();
        assert!(session.pending_recompile.contains(&entry_url));
        for _ in 0..2 {
            let diagnostics = spin_on::spin_on(session.reload_document(entry_url.clone())).unwrap();
            assert!(diagnostics.iter().any(|(url, _, diagnostics)| {
                url == &entry_url
                    && diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("DoesNotExist"))
            }));
        }
        std::fs::write(
            temp.path().join(FILE_NAME),
            r#"{ "entry": "main.slint", "style": "fluent" }"#,
        )
        .unwrap();
        spin_on::spin_on(
            session
                .reload_active_project_file(&temp.path().join(FILE_NAME), FileChangeKind::Changed),
        )
        .unwrap();
        let diagnostics = spin_on::spin_on(session.reload_document(entry_url.clone())).unwrap();
        assert!(diagnostics.iter().any(|(url, _, diagnostics)| url == &entry_url
            && diagnostics.iter().any(|diagnostic| diagnostic.message.contains("DoesNotExist"))));
    }

    #[test]
    fn changed_and_removed_entries_retain_shared_and_open_documents() {
        let temp = TempDir::new().unwrap();
        let project_directories = [temp.path().join("first"), temp.path().join("second")];
        let entries = [temp.path().join("shared.slint"), temp.path().join("replacement.slint")];
        let dependency = temp.path().join("dependency.slint");
        std::fs::write(&dependency, "export component Dependency {}").unwrap();
        for entry in &entries {
            std::fs::write(entry, r#"import { Dependency } from "dependency.slint"; export component Main inherits Dependency {}"#).unwrap();
        }
        let mut session = session();
        for directory in &project_directories {
            std::fs::create_dir(directory).unwrap();
            std::fs::write(directory.join(FILE_NAME), r#"{ "entry": "../shared.slint" }"#).unwrap();
            let document = directory.join("editing.slint");
            write_document(&document);
            open_document(&mut session, &document).unwrap();
        }
        let first_project = project_directories[0].join(FILE_NAME);
        std::fs::write(&first_project, r#"{ "entry": "../replacement.slint" }"#).unwrap();
        spin_on::spin_on(
            session.reload_active_project_file(&first_project, FileChangeKind::Changed),
        )
        .unwrap();
        for path in entries.iter().chain(core::iter::once(&dependency)) {
            assert!(cached_text(&session, path).is_some());
        }
        spin_on::spin_on(session.reload_active_project_file(
            &project_directories[1].join(FILE_NAME),
            FileChangeKind::Deleted,
        ))
        .unwrap();
        assert!(cached_text(&session, &entries[0]).is_none());
        assert!(cached_text(&session, &dependency).is_some());
        let replacement_url = Url::from_file_path(&entries[1]).unwrap();
        spin_on::spin_on(session.open_document(
            "export component Unsaved {}".into(),
            replacement_url,
            Some(3),
        ))
        .unwrap();
        let diagnostics = spin_on::spin_on(
            session.reload_active_project_file(&first_project, FileChangeKind::Deleted),
        )
        .unwrap();
        assert!(cached_text(&session, &entries[1]).unwrap().contains("Unsaved"));
        assert!(cached_text(&session, &dependency).is_none());
        assert!(
            diagnostics
                .iter()
                .any(|(url, _, diagnostics)| url == &Url::from_file_path(&dependency).unwrap()
                    && diagnostics.is_empty())
        );
        for directory in &project_directories {
            assert!(cached_text(&session, &directory.join("editing.slint")).is_some());
        }
    }

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    #[test]
    fn removed_entry_is_retained_by_preview() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join(FILE_NAME);
        let entry = temp.path().join("main.slint");
        let document = temp.path().join("editing.slint");
        std::fs::write(&project, r#"{ "entry": "main.slint" }"#).unwrap();
        write_document(&entry);
        write_document(&document);
        let mut session = session();
        open_document(&mut session, &document).unwrap();
        let entry_url = Url::from_file_path(&entry).unwrap();
        let encoded_url =
            Url::parse(&entry_url.as_str().replace("main.slint", "main.%73lint")).unwrap();
        session.show_preview(0, PreviewComponent { url: encoded_url, component: None });
        assert!(session.pending_recompile.contains(&entry_url));
        spin_on::spin_on(session.reload_active_project_file(&project, FileChangeKind::Deleted))
            .unwrap();
        assert!(cached_text(&session, &entry).is_some());
        assert!(session.pending_recompile.contains(&entry_url));
        spin_on::spin_on(session.drop_document(entry_url.clone())).unwrap();
        assert!(session.pending_recompile.contains(&entry_url));
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

        assert_eq!(
            session.active_project_file_paths().collect::<Vec<_>>(),
            [nested_project_path.as_path()]
        );
        assert_eq!(session.preview_config.style, "material");
    }

    #[test]
    fn omitted_project_settings_keep_session_defaults() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join(FILE_NAME);
        let document_path = temp.path().join("main.slint");
        std::fs::write(&project_path, r#"{ "include-paths": ["include"] }"#).unwrap();
        write_document(&document_path);

        let mut session = session();
        load_document(&mut session, &document_path).unwrap();

        assert_eq!(session.preview_config.style, "fluent");
        assert_eq!(session.preview_config.include_paths, vec![temp.path().join("include")]);
    }

    #[test]
    fn equal_project_settings_and_additive_paths_have_no_diagnostics() {
        let temp = TempDir::new().unwrap();
        let mut session = session();
        for name in ["first", "second"] {
            let directory = temp.path().join(name);
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join(FILE_NAME);
            std::fs::write(&path, r#"{"style":"fluent","enable-experimental-features":false,"include-paths":["include"],"library-paths":{"shared":"../shared"}}"#).unwrap();
            session.project_file_paths.push(path.clone());
            session.active_projects.insert(path.clone(), ProjectFile::load(&path).unwrap());
        }
        let diagnostics = spin_on::spin_on(session.reapply_effective_configuration()).unwrap();
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|(_, _, warnings)| warnings.is_empty()));
        assert_eq!(session.preview_config.include_paths.len(), 2);
    }

    #[test]
    fn library_conflict_diagnostics_name_the_project_supplying_the_library() {
        let temp = TempDir::new().unwrap();
        let mut session = session();
        let mut project_paths = Vec::new();
        for (name, libraries) in [
            ("first", serde_json::json!({"unrelated": "first-lib"})),
            ("second", serde_json::json!({"shared": "second-lib"})),
            ("third", serde_json::json!({"shared": "third-lib"})),
        ] {
            let directory = temp.path().join(name);
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join(FILE_NAME);
            std::fs::write(&path, serde_json::json!({"library-paths": libraries}).to_string())
                .unwrap();
            session.project_file_paths.push(path.clone());
            session.active_projects.insert(path.clone(), ProjectFile::load(&path).unwrap());
            project_paths.push(path);
        }
        let diagnostics = spin_on::spin_on(session.reapply_effective_configuration()).unwrap();
        let warnings = &diagnostics
            .iter()
            .find(|(url, _, _)| url == &Url::from_file_path(&project_paths[2]).unwrap())
            .unwrap()
            .2;
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].message.contains(&format!("project file {}", project_paths[1].display()))
        );
        let diagnostics =
            spin_on::spin_on(session.set_workspace_config_overrides(SessionConfigOverrides {
                compiler: Overrides {
                    project: ProjectFileData {
                        library_paths: Some(HashMap::from([(
                            "shared".into(),
                            temp.path().join("third/third-lib"),
                        )])),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }))
            .unwrap();
        for (project_path, expected_warnings) in project_paths.iter().zip([1, 1, 0]) {
            let warnings = &diagnostics
                .iter()
                .find(|(url, _, _)| url == &Url::from_file_path(project_path).unwrap())
                .unwrap()
                .2;
            assert_eq!(warnings.len(), expected_warnings);
            assert!(
                warnings
                    .iter()
                    .all(|warning| warning.message.contains("workspace configuration override"))
            );
        }
    }

    #[test]
    fn project_diagnostics_follow_final_configuration_and_clear() {
        let temp = TempDir::new().unwrap();
        let outer = temp.path().join(FILE_NAME);
        let inner_directory = temp.path().join("nested");
        std::fs::create_dir_all(&inner_directory).unwrap();
        let inner = inner_directory.join(FILE_NAME);
        let document = inner_directory.join("main.slint");
        write_document(&document);
        std::fs::write(&outer, r#"{"style":"fluent","enable-experimental-features":true,"library-paths":{"widgets":"outer"}}"#).unwrap();
        std::fs::write(&inner, r#"{"style":"material","enable-experimental-features":false,"include-paths":["include"],"library-paths":{"widgets":"inner","other":"other"}}"#).unwrap();
        let mut session = session();
        for path in [&outer, &inner] {
            session.project_file_paths.push(path.clone());
            session.active_projects.insert(path.clone(), ProjectFile::load(path).unwrap());
        }
        let recompute = |session: &mut EditorSession| {
            spin_on::spin_on(session.reapply_effective_configuration()).unwrap()
        };
        let warnings_for = |diagnostics: &VersionedDiagnostics, path: &std::path::Path| {
            diagnostics
                .iter()
                .find(|(url, _, _)| url == &Url::from_file_path(path).unwrap())
                .unwrap()
                .2
                .clone()
        };
        let diagnostics = recompute(&mut session);
        assert!(warnings_for(&diagnostics, &outer).is_empty());
        let warnings = warnings_for(&diagnostics, &inner);
        assert_eq!(warnings.len(), 3);
        assert!(warnings.iter().all(|warning| warning.message.contains("project file")));
        session.set_startup_config_overrides(SessionConfigOverrides {
            compiler: Overrides {
                project: ProjectFileData {
                    style: Some("cupertino".into()),
                    enable_experimental_features: Some(false),
                    include_paths: Some(Vec::new()),
                    library_paths: Some(HashMap::new()),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        });
        let diagnostics = recompute(&mut session);
        for path in [&outer, &inner] {
            let warnings = warnings_for(&diagnostics, path);
            assert!(!warnings.is_empty());
            assert!(
                warnings
                    .iter()
                    .all(|warning| warning.message.contains("startup configuration override"))
            );
        }
        let diagnostics =
            spin_on::spin_on(session.set_workspace_config_overrides(SessionConfigOverrides {
                compiler: Overrides {
                    project: ProjectFileData {
                        style: Some("material".into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }))
            .unwrap();
        let warnings = warnings_for(&diagnostics, &outer);
        assert!(warnings.iter().any(|warning| {
            warning.message.contains("style 'material' from workspace configuration override")
        }));
        assert!(
            !warnings_for(&diagnostics, &inner)
                .iter()
                .any(|warning| warning.message.contains("slint-lsp is ignoring style"))
        );
        session.workspace_config_overrides.compiler.project.include_paths =
            Some(vec![temp.path().join("workspace")]);
        session.workspace_config_overrides.compiler.project.library_paths =
            Some(HashMap::from([("other".into(), temp.path().join("workspace-lib"))]));
        let diagnostics = recompute(&mut session);
        let warnings = warnings_for(&diagnostics, &inner);
        assert_eq!(warnings.len(), 3);
        assert!(
            warnings
                .iter()
                .all(|warning| { warning.message.contains("workspace configuration override") })
        );
        assert!(warnings.iter().any(|warning| warning.message.contains("no library 'widgets'")));
        session.workspace_config_overrides.compiler.project.include_paths = Some(Vec::new());
        session.workspace_config_overrides.compiler.project.library_paths = Some(HashMap::new());
        let diagnostics = recompute(&mut session);
        assert!(
            warnings_for(&diagnostics, &inner)
                .iter()
                .all(|warning| { warning.message.contains("workspace configuration override") })
        );
        let diagnostics = spin_on::spin_on(session.load_document(
            std::fs::read_to_string(&document).unwrap(),
            Url::from_file_path(&document).unwrap(),
            Some(1),
        ))
        .unwrap();
        assert!(!diagnostics.iter().any(|(url, _, _)| {
            [&outer, &inner].iter().any(|path| url == &Url::from_file_path(path).unwrap())
        }));
        std::fs::write(&inner, "invalid").unwrap();
        let diagnostics =
            spin_on::spin_on(session.reload_active_project_file(&inner, FileChangeKind::Changed))
                .unwrap();
        assert!(warnings_for(&diagnostics, &inner).is_empty());
        let diagnostics =
            spin_on::spin_on(session.trigger_file_watcher(
                Url::from_file_path(&inner).unwrap(),
                FileChangeKind::Deleted,
            ))
            .unwrap();
        assert!(warnings_for(&diagnostics, &inner).is_empty());
        session.set_startup_config_overrides(Default::default());
        let diagnostics =
            spin_on::spin_on(session.set_workspace_config_overrides(Default::default())).unwrap();
        assert!(warnings_for(&diagnostics, &outer).is_empty());
    }

    #[test]
    fn config_overrides_replace_collections_and_fall_back_when_removed() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join(FILE_NAME);
        let document_path = temp.path().join("main.slint");
        std::fs::write(&project_path, r#"{ "style": "material", "include-paths": ["project"], "library-paths": {"project": "project-lib"} }"#).unwrap();
        write_document(&document_path);

        let mut session = session();
        let startup_paths = vec![temp.path().join("startup")];
        let startup_libraries =
            HashMap::from([("startup".into(), temp.path().join("startup-lib"))]);
        session.set_startup_config_overrides(SessionConfigOverrides {
            compiler: Overrides {
                project: ProjectFileData {
                    include_paths: Some(startup_paths.clone()),
                    library_paths: Some(startup_libraries.clone()),
                    style: Some("cosmic".into()),
                    enable_experimental_features: Some(true),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        });
        load_document(&mut session, &document_path).unwrap();

        for (paths, libraries) in [
            (
                vec![temp.path().join("workspace")],
                HashMap::from([("workspace".into(), temp.path().join("workspace-lib"))]),
            ),
            (Vec::new(), HashMap::new()),
        ] {
            spin_on::spin_on(session.set_workspace_config_overrides(SessionConfigOverrides {
                compiler: Overrides {
                    project: ProjectFileData {
                        include_paths: Some(paths.clone()),
                        library_paths: Some(libraries.clone()),
                        style: Some("cupertino".into()),
                        enable_experimental_features: Some(false),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }))
            .unwrap();
            assert_eq!(session.preview_config.include_paths, paths);
            assert_eq!(session.preview_config.library_paths, libraries);
            assert_eq!(session.preview_config.style, "cupertino");
            assert!(!session.preview_config.enable_experimental);
        }
        spin_on::spin_on(session.set_workspace_config_overrides(Default::default())).unwrap();
        assert_eq!(session.preview_config.include_paths, startup_paths);
        assert_eq!(session.preview_config.library_paths, startup_libraries);
        assert_eq!(session.preview_config.style, "cosmic");
        assert!(session.preview_config.enable_experimental);
        session.set_startup_config_overrides(Default::default());
        spin_on::spin_on(session.set_workspace_config_overrides(Default::default())).unwrap();
        assert_eq!(session.preview_config.include_paths, vec![temp.path().join("project")]);
        assert_eq!(
            session.preview_config.library_paths,
            HashMap::from([("project".into(), temp.path().join("project-lib"))])
        );
        assert_eq!(session.preview_config.style, "material");
    }

    #[test]
    fn workspace_hide_ui_wins_when_supplied() {
        for (startup, workspace, expected) in [
            (Some(true), Some(false), Some(false)),
            (Some(false), Some(true), Some(true)),
            (Some(true), None, Some(true)),
            (None, Some(false), Some(false)),
            (Some(true), Some(true), Some(true)),
        ] {
            let mut overrides = SessionConfigOverrides { hide_ui: startup, ..Default::default() };
            overrides.merge(SessionConfigOverrides { hide_ui: workspace, ..Default::default() });
            assert_eq!(overrides.hide_ui, expected);
        }
    }

    #[test]
    fn a_conflicting_style_stays_with_the_broadest_project_file() {
        let temp = TempDir::new().unwrap();
        let outer = temp.path().to_path_buf();
        let inner = outer.join("nested");
        std::fs::create_dir_all(&inner).unwrap();
        let outer_file = outer.join(FILE_NAME);
        let inner_file = inner.join(FILE_NAME);
        let outer_document = outer.join("main.slint");
        let inner_document = inner.join("main.slint");
        std::fs::write(&outer_file, r#"{ "style": "material" }"#).unwrap();
        std::fs::write(&inner_file, r#"{ "style": "cupertino" }"#).unwrap();
        write_document(&outer_document);
        write_document(&inner_document);

        let mut session = session();
        load_document(&mut session, &inner_document).unwrap();
        load_document(&mut session, &outer_document).unwrap();

        assert_eq!(
            session.active_project_file_paths().collect::<Vec<_>>(),
            [outer_file.as_path(), inner_file.as_path()]
        );
        assert_eq!(session.preview_config.style, "material");
    }

    #[test]
    fn settings_of_several_project_files_are_merged() {
        let temp = TempDir::new().unwrap();
        let project_a = temp.path().join("project-a");
        let project_b = temp.path().join("project-b");
        std::fs::create_dir_all(&project_a).unwrap();
        std::fs::create_dir_all(&project_b).unwrap();
        let document_a = project_a.join("main.slint");
        let document_b = project_b.join("main.slint");
        std::fs::write(
            project_a.join(FILE_NAME),
            r#"{ "$schema": "schema-a", "entry": "main.slint", "include-paths": ["include-a"], "style": "material" }"#,
        )
        .unwrap();
        std::fs::write(
            project_b.join(FILE_NAME),
            r#"{ "include-paths": ["include-b"], "library-paths": {"widgets": "lib.slint"} }"#,
        )
        .unwrap();
        write_document(&document_a);
        write_document(&document_b);

        let mut session = session();
        load_document(&mut session, &document_a).unwrap();
        load_document(&mut session, &document_b).unwrap();

        // Both are equally deep, so the include paths keep the order they were discovered in.
        assert_eq!(
            session.preview_config.include_paths,
            vec![project_a.join("include-a"), project_b.join("include-b")]
        );
        assert_eq!(
            session.preview_config.library_paths,
            HashMap::from([("widgets".to_string(), project_b.join("lib.slint"))])
        );
        // Only project-a has a style, so no conflict arises.
        assert_eq!(session.preview_config.style, "material");
        let (merged, _) = session.merged_project_data(Default::default());
        assert_eq!(merged.schema, None);
        assert_eq!(merged.entry, None);
        assert_eq!(merged.enable_experimental_features, None);
        assert_eq!(session.active_projects[&project_a.join(FILE_NAME)].entry(), Some(document_a));
    }

    #[test]
    fn invalid_project_recovery_preserves_equal_depth_precedence() {
        let temp = TempDir::new().unwrap();
        let mut session = session();
        let mut project_paths = Vec::new();
        for (directory, style) in [("first", "material"), ("second", "cupertino")] {
            let directory = temp.path().join(directory);
            std::fs::create_dir_all(&directory).unwrap();
            let project_path = directory.join(FILE_NAME);
            std::fs::write(&project_path, format!(r#"{{ "style": "{style}" }}"#)).unwrap();
            let document_path = directory.join("main.slint");
            write_document(&document_path);
            load_document(&mut session, &document_path).unwrap();
            project_paths.push(project_path);
        }

        for path in &project_paths {
            std::fs::write(path, "{").unwrap();
            spin_on::spin_on(session.reload_active_project_file(path, FileChangeKind::Changed))
                .unwrap();
        }
        assert!(session.active_projects.is_empty());
        assert_eq!(
            session.active_project_file_paths().collect::<Vec<_>>(),
            project_paths.iter().map(PathBuf::as_path).collect::<Vec<_>>()
        );

        for path in project_paths.iter().rev() {
            let style = if path == &project_paths[0] { "material" } else { "cupertino" };
            std::fs::write(path, format!(r#"{{ "style": "{style}" }}"#)).unwrap();
            spin_on::spin_on(session.reload_active_project_file(path, FileChangeKind::Changed))
                .unwrap();
        }
        assert_eq!(session.preview_config.style, "material");
    }

    #[test]
    fn a_conflicting_library_path_stays_with_the_broadest_project_file() {
        let temp = TempDir::new().unwrap();
        let outer = temp.path().to_path_buf();
        let inner = outer.join("nested");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(outer.join(FILE_NAME), r#"{ "library-paths": {"widgets": "outer.slint"} }"#)
            .unwrap();
        std::fs::write(inner.join(FILE_NAME), r#"{ "library-paths": {"widgets": "inner.slint"} }"#)
            .unwrap();
        write_document(&outer.join("main.slint"));
        write_document(&inner.join("main.slint"));

        let mut session = session();
        load_document(&mut session, &inner.join("main.slint")).unwrap();
        load_document(&mut session, &outer.join("main.slint")).unwrap();

        assert_eq!(
            session.preview_config.library_paths,
            HashMap::from([("widgets".to_string(), outer.join("outer.slint"))])
        );
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
        assert_eq!(session.active_project_file_paths().next(), None);
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
        assert_eq!(
            session.active_project_file_paths().collect::<Vec<_>>(),
            [project_path.as_path()]
        );
        assert!(session.pending_recompile.contains(&document_url));

        std::fs::write(&project_path, r#"{ "style": "cupertino" }"#).unwrap();
        spin_on::spin_on(session.trigger_file_watcher(project_url, FileChangeKind::Changed))
            .unwrap();

        assert_eq!(session.preview_config.style, "cupertino");
        assert!(session.pending_recompile.contains(&document_url));
    }

    #[test]
    fn a_deleted_project_file_is_dropped() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join(FILE_NAME);
        let document_path = temp.path().join("main.slint");
        std::fs::write(&project_path, r#"{ "style": "material" }"#).unwrap();
        write_document(&document_path);

        let mut session = session();
        open_document(&mut session, &document_path).unwrap();
        let document_url = Url::from_file_path(&document_path).unwrap();
        let project_url = Url::from_file_path(&project_path).unwrap();

        std::fs::remove_file(&project_path).unwrap();
        let diagnostics = spin_on::spin_on(
            session.trigger_file_watcher(project_url.clone(), FileChangeKind::Deleted),
        )
        .unwrap();
        assert!(
            diagnostics.iter().any(|(url, _, warnings)| url == &project_url && warnings.is_empty())
        );

        assert_eq!(session.active_project_file_paths().next(), None);
        assert_eq!(session.preview_config.style, "fluent");
        assert!(session.pending_recompile.contains(&document_url));

        // A project file created again is found the next time the document loads.
        std::fs::write(&project_path, r#"{ "style": "cupertino" }"#).unwrap();
        spin_on::spin_on(session.reload_document(document_url)).unwrap();

        assert_eq!(
            session.active_project_file_paths().collect::<Vec<_>>(),
            [project_path.as_path()]
        );
        assert_eq!(session.preview_config.style, "cupertino");
    }
}
