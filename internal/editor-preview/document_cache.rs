// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::{BuildDiagnostics, SourceFile};
use i_slint_compiler::object_tree::Document;
use i_slint_compiler::parser::{TextSize, syntax_nodes};
use i_slint_compiler::source_path::SourcePath;
use i_slint_compiler::typeloader::TypeLoader;
use i_slint_compiler::typeregister::TypeRegister;
use i_slint_live_preview::protocol::SourceFileVersion;
use lsp_types::Url;

use std::{cell::RefCell, collections::HashMap, future::Future, path::PathBuf, pin::Pin, rc::Rc};

use crate::{ElementRcNode, Result};
use std::collections::HashSet;

pub type SourceFileVersionMap = HashMap<SourcePath, SourceFileVersion>;

fn default_cc() -> i_slint_compiler::CompilerConfiguration {
    let mut cc = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    // All LSP document handling (diagnostics, completion, hover, live preview) is tooling
    // around the .slint source, never a real host application driving it.
    cc.is_preview = true;
    cc
}

/// This is i_slint_compiler::OpenImportCallback with version information
pub type OpenImportCallback = Rc<
    dyn Fn(
        SourcePath,
    )
        -> Pin<Box<dyn Future<Output = Option<std::io::Result<(SourceFileVersion, String)>>>>>,
>;

#[derive(Clone)]
pub struct CompilerConfiguration {
    pub compiler_config: i_slint_compiler::CompilerConfiguration,
    pub open_import_callback: Option<OpenImportCallback>,
    pub format: crate::ByteFormat,
}

impl Default for CompilerConfiguration {
    fn default() -> Self {
        Self {
            compiler_config: default_cc(),
            open_import_callback: None,
            format: crate::ByteFormat::Utf8,
        }
    }
}

/// A cache of loaded documents
pub struct DocumentCache {
    type_loader: TypeLoader,
    open_import_callback: Option<OpenImportCallback>,
    source_file_versions: Rc<RefCell<SourceFileVersionMap>>,
    pub format: crate::ByteFormat,
    config: CompilerConfiguration,
}

#[cfg(feature = "preview-engine")]
pub fn document_cache_parts_setup(
    compiler_config: &mut i_slint_compiler::CompilerConfiguration,
    open_import_callback: Option<OpenImportCallback>,
    initial_file_versions: SourceFileVersionMap,
) -> (Option<OpenImportCallback>, Rc<RefCell<SourceFileVersionMap>>) {
    let source_file_versions = Rc::new(RefCell::new(initial_file_versions));
    DocumentCache::wire_up_import_fallback(
        compiler_config,
        open_import_callback,
        source_file_versions,
    )
}

impl DocumentCache {
    fn wire_up_import_fallback(
        compiler_config: &mut i_slint_compiler::CompilerConfiguration,
        open_import_callback: Option<OpenImportCallback>,
        source_file_versions: Rc<RefCell<SourceFileVersionMap>>,
    ) -> (Option<OpenImportCallback>, Rc<RefCell<SourceFileVersionMap>>) {
        let source_versions = source_file_versions.clone();
        if let Some(open_import_callback) = open_import_callback.clone() {
            compiler_config.open_import_callback = Some(Rc::new(move |path: SourcePath| {
                let open_import = open_import_callback(path.clone());
                let source_versions = source_versions.clone();
                Box::pin(async move {
                    open_import.await.map(|r| match r {
                        Ok((v, c)) => {
                            source_versions.borrow_mut().insert(path, v);
                            Ok(c)
                        }
                        Err(e) => {
                            source_versions.borrow_mut().remove(&path);
                            Err(e)
                        }
                    })
                })
            }))
        }

        (open_import_callback, source_file_versions)
    }

    pub fn new(config: CompilerConfiguration) -> Self {
        let format = config.format;
        let mut compiler_config = config.compiler_config.clone();
        let open_import_callback = config.open_import_callback.clone();

        let (open_import_callback, source_file_versions) = Self::wire_up_import_fallback(
            &mut compiler_config,
            open_import_callback,
            Rc::new(RefCell::new(SourceFileVersionMap::default())),
        );

        Self {
            type_loader: TypeLoader::new(compiler_config, &mut BuildDiagnostics::default()),
            open_import_callback,
            source_file_versions,
            format,
            config,
        }
    }

    pub fn new_from_raw_parts(
        mut type_loader: TypeLoader,
        open_import_callback: Option<OpenImportCallback>,
        source_file_versions: Rc<RefCell<SourceFileVersionMap>>,
        format: super::ByteFormat,
    ) -> Self {
        let mut compiler_config = type_loader.compiler_config.clone();
        if open_import_callback.is_some() {
            compiler_config.open_import_callback = None;
        }
        let config = CompilerConfiguration {
            compiler_config,
            open_import_callback: open_import_callback.clone(),
            format,
        };
        let (open_import_callback, source_file_versions) = Self::wire_up_import_fallback(
            &mut type_loader.compiler_config,
            open_import_callback,
            source_file_versions,
        );

        Self { type_loader, open_import_callback, source_file_versions, format, config }
    }

    pub fn snapshot(&self) -> Option<Self> {
        let open_import_callback = self.open_import_callback.clone();
        let source_file_versions =
            Rc::new(RefCell::new(self.source_file_versions.borrow().clone()));
        i_slint_compiler::typeloader::snapshot(&self.type_loader).map(|tl| {
            Self::new_from_raw_parts(tl, open_import_callback, source_file_versions, self.format)
        })
    }

    pub fn resolve_import_path(
        &self,
        import_token: Option<&i_slint_compiler::parser::NodeOrToken>,
        maybe_relative_path_or_url: &str,
    ) -> Option<SourcePath> {
        self.type_loader.resolve_import_path(import_token, maybe_relative_path_or_url)
    }

    pub fn document_version(&self, target_uri: &Url) -> SourceFileVersion {
        self.document_version_by_path(&SourcePath::from_url(target_uri))
    }

    pub fn document_version_by_path(&self, path: &SourcePath) -> SourceFileVersion {
        self.source_file_versions.borrow().get(path).and_then(|v| *v)
    }

    pub fn get_document<'a>(&'a self, url: &'_ Url) -> Option<&'a Document> {
        self.type_loader.get_document(&SourcePath::from_url(url))
    }

    /// Iterator over every fully-loaded `object_tree::Document` in the cache.
    pub fn all_documents(&self) -> impl Iterator<Item = &Document> + '_ {
        self.type_loader.all_documents()
    }

    fn uses_widgets_impl(&self, doc_path: SourcePath, dedup: &mut HashSet<SourcePath>) -> bool {
        if dedup.contains(&doc_path) {
            return false;
        }

        if doc_path.is_builtin() && doc_path.file_name() == Some("std-widgets.slint") {
            return true;
        }

        let Some(doc) = self.get_document_by_path(&doc_path) else {
            return false;
        };

        dedup.insert(doc_path);

        for import in doc.imports.iter().filter_map(|i| i.resolved.clone()) {
            if self.uses_widgets_impl(import, dedup) {
                return true;
            }
        }

        false
    }

    /// Returns true if doc_url uses (possibly indirectly) widgets from "std-widgets.slint"
    pub fn uses_widgets(&self, doc_url: &Url) -> bool {
        self.uses_widgets_impl(SourcePath::from_url(doc_url), &mut HashSet::new())
    }

    pub fn get_document_by_path<'a>(&'a self, path: &'_ SourcePath) -> Option<&'a Document> {
        self.type_loader.get_document(path)
    }

    pub fn get_document_for_source_file<'a>(
        &'a self,
        source_file: &'_ SourceFile,
    ) -> Option<&'a Document> {
        self.type_loader.get_document(source_file.path())
    }

    pub fn get_document_and_offset<'a>(
        &'a self,
        text_document_uri: &'_ Url,
        pos: &'_ lsp_types::Position,
    ) -> Option<(&'a i_slint_compiler::object_tree::Document, TextSize)> {
        let doc = self.get_document(text_document_uri)?;
        let o = (doc.node.as_ref()?.source_file.offset(
            pos.line as usize + 1,
            pos.character as usize + 1,
            self.format,
        ) as u32)
            .into();
        doc.node.as_ref()?.text_range().contains_inclusive(o).then_some((doc, o))
    }

    pub fn all_url_documents(&self) -> impl Iterator<Item = (Url, &syntax_nodes::Document)> + '_ {
        self.type_loader.all_file_documents().filter_map(|(p, d)| Some((p.to_url()?, d)))
    }

    pub fn all_urls(&self) -> impl Iterator<Item = Url> + '_ {
        self.type_loader.all_files().filter_map(SourcePath::to_url)
    }

    pub fn global_type_registry(&self) -> std::cell::Ref<'_, TypeRegister> {
        self.type_loader.global_type_registry.borrow()
    }

    pub fn revision(&self) -> u64 {
        self.type_loader.revision()
    }

    fn invalidate_everything(&mut self) {
        let all_files = self.type_loader.all_files().cloned().collect::<Vec<_>>();

        for path in all_files {
            self.type_loader.invalidate_document(&path);
        }
    }

    /// Re-apply a complete configuration to the document cache
    ///
    /// Returns the new compiler configuration and the set of paths that were reloaded.
    pub async fn reconfigure(
        &mut self,
        config: CompilerConfiguration,
        roots: &HashSet<Url>,
        diag: &mut BuildDiagnostics,
    ) -> (CompilerConfiguration, HashSet<lsp_types::Url>) {
        let mut compiler_config = config.compiler_config.clone();
        let (open_import_callback, source_file_versions) = Self::wire_up_import_fallback(
            &mut compiler_config,
            config.open_import_callback.clone(),
            self.source_file_versions.clone(),
        );

        if self.type_loader.compiler_config.enable_experimental
            != compiler_config.enable_experimental
        {
            *self.type_loader.global_type_registry.borrow_mut() =
                Rc::into_inner(if compiler_config.enable_experimental {
                    TypeRegister::builtin_experimental()
                } else {
                    TypeRegister::builtin()
                })
                .unwrap()
                .into_inner();
        }

        self.config = config.clone();
        self.open_import_callback = open_import_callback;
        self.source_file_versions = source_file_versions;
        self.format = config.format;
        self.type_loader.compiler_config = compiler_config;

        self.invalidate_everything();

        self.preload_builtins().await;

        let all_urls = self.all_urls().filter(|url| roots.contains(url)).collect::<HashSet<_>>();
        for url in &all_urls {
            self.reload_cached_file(url, diag).await;
        }

        let mut config = config;
        config.open_import_callback = None;
        (config, all_urls)
    }

    pub async fn preload_builtins(&mut self) {
        // Always load the widgets so we can auto-complete them
        let mut diag = BuildDiagnostics::default();
        self.type_loader.import_component("std-widgets.slint", "StyleMetrics", &mut diag).await;
        assert!(!diag.has_errors());
    }

    pub async fn load_url(
        &mut self,
        url: &Url,
        version: SourceFileVersion,
        content: String,
        diag: &mut BuildDiagnostics,
    ) -> Result<()> {
        let path = SourcePath::from_url(url);
        self.type_loader.load_file(&path, content, false, diag).await;
        self.source_file_versions.borrow_mut().insert(path, version);
        Ok(())
    }

    pub async fn reload_cached_file(&mut self, url: &Url, diag: &mut BuildDiagnostics) {
        self.type_loader.reload_cached_file(&SourcePath::from_url(url), diag).await;
    }

    /// Drop a document from the cache.
    /// Returns the list of dependencies that were invalidated.
    ///
    /// Compared to [Self::invalidate_url], this actually causes the document to be reloaded from
    /// disk, not just reparse.
    pub fn drop_document(&mut self, url: &Url) -> Result<HashSet<Url>> {
        let path = SourcePath::from_url(url);
        self.source_file_versions.borrow_mut().remove(&path);
        Ok(self.type_loader.drop_document(&path)?.iter().filter_map(|path| path.to_url()).collect())
    }

    /// Invalidate a document and all its dependencies.
    /// return the list of dependencies that were invalidated.
    ///
    /// Compared to [Self::drop_document], the CST remains in the cache, and only the type
    /// information is dropped from the cache, which causes the document to be re-analyzed.
    pub fn invalidate_url(&mut self, url: &Url) -> HashSet<Url> {
        self.type_loader
            .invalidate_document(&SourcePath::from_url(url))
            .into_iter()
            .filter_map(|x| x.to_url())
            .collect()
    }

    pub(crate) fn retain_documents(&mut self, roots: &HashSet<Url>) -> Result<HashSet<Url>> {
        let mut retained = roots.clone();
        let mut pending = roots.iter().cloned().collect::<Vec<_>>();
        while let Some(url) = pending.pop() {
            if let Some(document) = self.get_document(&url) {
                for import in &document.imports {
                    if let Some(import_url) = import.resolved.as_ref().and_then(SourcePath::to_url)
                        && retained.insert(import_url.clone())
                    {
                        pending.push(import_url);
                    }
                }
            }
        }
        let removed = self
            .all_urls()
            .filter(|url| url.scheme() != "builtin" && !retained.contains(url))
            .collect::<HashSet<_>>();
        for url in &removed {
            self.drop_document(url)?;
        }
        Ok(removed)
    }

    pub fn compiler_configuration(&self) -> CompilerConfiguration {
        let mut config = self.config.clone();
        config.open_import_callback = None;
        config.compiler_config.open_import_callback = None;
        config
    }

    pub(crate) fn configuration_with_import_callback(&self) -> CompilerConfiguration {
        self.config.clone()
    }

    fn element_at_document_and_offset(
        &self,
        document: &i_slint_compiler::object_tree::Document,
        offset: TextSize,
    ) -> Option<ElementRcNode> {
        fn element_contains(
            element: &i_slint_compiler::object_tree::ElementRc,
            offset: TextSize,
        ) -> Option<usize> {
            element
                .borrow()
                .debug
                .iter()
                .position(|n| n.node.parent().is_some_and(|n| n.text_range().contains(offset)))
        }

        for component in &document.inner_components {
            let root_element = component.root_element.clone();
            let Some(root_debug_index) = element_contains(&root_element, offset) else {
                continue;
            };

            let mut element =
                ElementRcNode { element: root_element, debug_index: root_debug_index };
            while element.contains_offset(offset) {
                if let Some((c, i)) = element
                    .element
                    .clone()
                    .borrow()
                    .children
                    .iter()
                    .find_map(|c| element_contains(c, offset).map(|i| (c, i)))
                {
                    element = ElementRcNode { element: c.clone(), debug_index: i };
                } else {
                    return Some(element);
                }
            }
        }
        None
    }

    pub fn element_at_offset(
        &self,
        text_document_uri: &Url,
        offset: TextSize,
    ) -> Option<ElementRcNode> {
        let doc = self.get_document(text_document_uri)?;
        self.element_at_document_and_offset(doc, offset)
    }

    pub fn element_at_position(
        &self,
        text_document_uri: &Url,
        pos: &lsp_types::Position,
    ) -> Option<ElementRcNode> {
        let (doc, offset) = self.get_document_and_offset(text_document_uri, pos)?;
        self.element_at_document_and_offset(doc, offset)
    }

    pub fn all_paths_to_watch(&self) -> HashSet<PathBuf> {
        self.type_loader
            .all_files_to_watch()
            .into_iter()
            .filter_map(SourcePath::into_native_path)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::test::complex_document_cache;

    use super::*;

    fn id_at_position(dc: &DocumentCache, url: &Url, line: u32, character: u32) -> Option<String> {
        let result = dc.element_at_position(url, &lsp_types::Position { line, character })?;
        let element = result.element.borrow();
        Some(element.id.to_string())
    }

    fn base_type_at_position(
        dc: &DocumentCache,
        url: &Url,
        line: u32,
        character: u32,
    ) -> Option<String> {
        let result = dc.element_at_position(url, &lsp_types::Position { line, character })?;
        let element = result.element.borrow();
        Some(element.base_type.to_string())
    }

    #[test]
    fn test_element_at_position_no_element() {
        let (dc, url, _) = complex_document_cache();
        assert_eq!(id_at_position(&dc, &url, 0, 10), None);
        // TODO: This is past the end of the line and should thus return None
        assert_eq!(id_at_position(&dc, &url, 42, 90), Some(String::new()));
        assert_eq!(id_at_position(&dc, &url, 1, 0), None);
        assert_eq!(id_at_position(&dc, &url, 55, 1), None);
        assert_eq!(id_at_position(&dc, &url, 56, 5), None);
    }

    #[test]
    fn test_document_version() {
        let (dc, url, _) = complex_document_cache();
        assert_eq!(dc.document_version(&url), Some(42));
    }

    #[test]
    fn test_snapshot_preserves_editor_configuration() {
        let config = CompilerConfiguration {
            compiler_config: {
                let mut compiler_config =
                    crate::document_cache::CompilerConfiguration::default().compiler_config;
                compiler_config.style = Some("fluent".into());
                compiler_config.resource_url_mapper = Some(Rc::new(|_| Box::pin(async { None })));
                compiler_config.translation_domain = Some("test-domain".into());
                compiler_config.const_scale_factor = Some(2.0);
                compiler_config
            },
            format: crate::ByteFormat::Utf16,
            ..Default::default()
        };

        let snapshot = DocumentCache::new(config).snapshot().expect("snapshot");
        let config = snapshot.compiler_configuration();

        assert_eq!(config.compiler_config.style.as_deref(), Some("fluent"));
        assert!(config.compiler_config.resource_url_mapper.is_some());
        assert_eq!(config.compiler_config.translation_domain.as_deref(), Some("test-domain"));
        assert_eq!(config.compiler_config.const_scale_factor, Some(2.0));
        assert!(config.open_import_callback.is_none());
        assert_eq!(config.format, crate::ByteFormat::Utf16);
    }

    #[test]
    fn versioned_import_callback_survives_snapshot_and_reconfiguration() {
        let fail_import = Rc::new(std::cell::Cell::new(false));
        let fail_import_for_callback = fail_import.clone();
        let config = CompilerConfiguration {
            open_import_callback: Some(Rc::new(move |_| {
                let fail_import = fail_import_for_callback.get();
                Box::pin(async move {
                    Some(if fail_import {
                        Err(std::io::Error::other("test import failure"))
                    } else {
                        Ok((Some(42), "export component Imported {}".into()))
                    })
                })
            })),
            ..Default::default()
        };
        let cache = DocumentCache::new(config);
        let snapshot = cache.snapshot().unwrap();
        let path = SourcePath::new("callback-import.slint");

        for mut cache in [cache, snapshot] {
            fail_import.set(false);
            let config = cache.configuration_with_import_callback();
            assert!(config.open_import_callback.is_some());
            assert!(config.compiler_config.open_import_callback.is_none());
            assert!(cache.compiler_configuration().open_import_callback.is_none());
            spin_on::spin_on(cache.reconfigure(
                config,
                &HashSet::new(),
                &mut BuildDiagnostics::default(),
            ));
            let import_callback =
                cache.type_loader.compiler_config.open_import_callback.as_ref().unwrap();
            assert!(spin_on::spin_on(import_callback(path.clone())).unwrap().is_ok());
            assert_eq!(cache.document_version_by_path(&path), Some(42));

            fail_import.set(true);
            assert!(spin_on::spin_on(import_callback(path.clone())).unwrap().is_err());
            assert_eq!(cache.document_version_by_path(&path), None);
        }
    }

    #[test]
    fn test_element_at_position_no_such_document() {
        let (dc, _, _) = complex_document_cache();
        assert_eq!(id_at_position(&dc, &Url::parse("https://foo.bar/baz").unwrap(), 5, 0), None);
    }

    #[test]
    fn test_element_at_position_root() {
        let (dc, url, _) = complex_document_cache();

        assert_eq!(id_at_position(&dc, &url, 2, 30), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 2, 32), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 2, 42), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 3, 0), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 3, 53), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 4, 19), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 5, 0), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 6, 8), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 6, 15), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 6, 23), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 8, 15), Some("root".to_string()));
        assert_eq!(id_at_position(&dc, &url, 12, 3), Some("root".to_string())); // right before child // TODO: Seems wrong!
        assert_eq!(id_at_position(&dc, &url, 51, 5), Some("root".to_string())); // right after child // TODO: Why does this not work?
        assert_eq!(id_at_position(&dc, &url, 52, 0), Some("root".to_string()));
    }

    #[test]
    fn test_element_at_position_child() {
        let (dc, url, _) = complex_document_cache();

        assert_eq!(base_type_at_position(&dc, &url, 12, 4), Some("VerticalBox".to_string()));
        assert_eq!(base_type_at_position(&dc, &url, 14, 22), Some("HorizontalBox".to_string()));
        assert_eq!(base_type_at_position(&dc, &url, 15, 33), Some("Text".to_string()));
        assert_eq!(base_type_at_position(&dc, &url, 27, 4), Some("VerticalBox".to_string()));
        assert_eq!(base_type_at_position(&dc, &url, 28, 8), Some("Text".to_string()));
        assert_eq!(base_type_at_position(&dc, &url, 51, 4), Some("VerticalBox".to_string()));
    }
}
