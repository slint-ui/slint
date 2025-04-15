// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Data structures common between LSP and previewer

use i_slint_compiler::diagnostics::{BuildDiagnostics, SourceFile};
use i_slint_compiler::object_tree::Document;
use i_slint_compiler::parser::{syntax_nodes, TextSize};
use i_slint_compiler::typeloader::TypeLoader;
use i_slint_compiler::typeregister::TypeRegister;
use lsp_types::Url;

use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    rc::Rc,
};

use crate::common::{file_to_uri, uri_to_file, ElementRcNode, Result};
use std::collections::HashSet;

pub type SourceFileVersion = Option<i32>;

pub type SourceFileVersionMap = HashMap<PathBuf, SourceFileVersion>;

fn default_cc() -> i_slint_compiler::CompilerConfiguration {
    i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    )
}

pub type OpenImportFallback = Option<
    Rc<
        dyn Fn(
            String,
        ) -> Pin<
            Box<dyn Future<Output = Option<std::io::Result<(SourceFileVersion, String)>>>>,
        >,
    >,
>;

pub struct CompilerConfiguration {
    pub include_paths: Vec<std::path::PathBuf>,
    pub library_paths: HashMap<String, std::path::PathBuf>,
    pub style: Option<String>,
    pub open_import_fallback: OpenImportFallback,
    pub resource_url_mapper:
        Option<Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Option<String>>>>>>,
}

impl Default for CompilerConfiguration {
    fn default() -> Self {
        let mut cc = default_cc();

        Self {
            include_paths: std::mem::take(&mut cc.include_paths),
            library_paths: std::mem::take(&mut cc.library_paths),
            style: std::mem::take(&mut cc.style),
            open_import_fallback: None,
            resource_url_mapper: std::mem::take(&mut cc.resource_url_mapper),
        }
    }
}

impl CompilerConfiguration {
    fn build(mut self) -> (i_slint_compiler::CompilerConfiguration, OpenImportFallback) {
        let mut result = default_cc();
        result.include_paths = std::mem::take(&mut self.include_paths);
        result.library_paths = std::mem::take(&mut self.library_paths);
        result.style = std::mem::take(&mut self.style);
        result.resource_url_mapper = std::mem::take(&mut self.resource_url_mapper);

        (result, self.open_import_fallback)
    }
}

/// A cache of loaded documents
pub struct DocumentCache {
    type_loader: TypeLoader,
    open_import_fallback: OpenImportFallback,
    source_file_versions: Rc<RefCell<SourceFileVersionMap>>,
}

#[cfg(feature = "preview-engine")]
pub fn document_cache_parts_setup(
    compiler_config: &mut i_slint_compiler::CompilerConfiguration,
    open_import_fallback: OpenImportFallback,
    initial_file_versions: SourceFileVersionMap,
) -> (OpenImportFallback, Rc<RefCell<SourceFileVersionMap>>) {
    let source_file_versions = Rc::new(RefCell::new(initial_file_versions));
    DocumentCache::wire_up_import_fallback(
        compiler_config,
        open_import_fallback,
        source_file_versions,
    )
}

impl DocumentCache {
    fn wire_up_import_fallback(
        compiler_config: &mut i_slint_compiler::CompilerConfiguration,
        open_import_fallback: OpenImportFallback,
        source_file_versions: Rc<RefCell<SourceFileVersionMap>>,
    ) -> (OpenImportFallback, Rc<RefCell<SourceFileVersionMap>>) {
        let sfv = source_file_versions.clone();
        if let Some(open_import_fallback) = open_import_fallback.clone() {
            compiler_config.open_import_fallback = Some(Rc::new(move |file_name: String| {
                let flfb = open_import_fallback(file_name.clone());
                let sfv = sfv.clone();
                Box::pin(async move {
                    flfb.await.map(|r| {
                        let path = PathBuf::from(file_name);
                        match r {
                            Ok((v, c)) => {
                                sfv.borrow_mut().insert(path, v);
                                Ok(c)
                            }
                            Err(e) => {
                                sfv.borrow_mut().remove(&path);
                                Err(e)
                            }
                        }
                    })
                })
            }))
        }

        (open_import_fallback, source_file_versions)
    }

    pub fn new(config: CompilerConfiguration) -> Self {
        let (mut compiler_config, open_import_fallback) = config.build();

        let (open_import_fallback, source_file_versions) = Self::wire_up_import_fallback(
            &mut compiler_config,
            open_import_fallback,
            Rc::new(RefCell::new(SourceFileVersionMap::default())),
        );

        Self {
            type_loader: TypeLoader::new(
                i_slint_compiler::typeregister::TypeRegister::builtin(),
                compiler_config,
                &mut BuildDiagnostics::default(),
            ),
            open_import_fallback,
            source_file_versions,
        }
    }

    pub fn new_from_raw_parts(
        mut type_loader: TypeLoader,
        open_import_fallback: OpenImportFallback,
        source_file_versions: Rc<RefCell<SourceFileVersionMap>>,
    ) -> Self {
        let (open_import_fallback, source_file_versions) = Self::wire_up_import_fallback(
            &mut type_loader.compiler_config,
            open_import_fallback,
            source_file_versions,
        );

        Self { type_loader, open_import_fallback, source_file_versions }
    }

    pub fn snapshot(&self) -> Option<Self> {
        let open_import_fallback = self.open_import_fallback.clone();
        let source_file_versions =
            Rc::new(RefCell::new(self.source_file_versions.borrow().clone()));
        i_slint_compiler::typeloader::snapshot(&self.type_loader)
            .map(|tl| Self::new_from_raw_parts(tl, open_import_fallback, source_file_versions))
    }

    pub fn resolve_import_path(
        &self,
        import_token: Option<&i_slint_compiler::parser::NodeOrToken>,
        maybe_relative_path_or_url: &str,
    ) -> Option<(PathBuf, Option<&'static [u8]>)> {
        self.type_loader.resolve_import_path(import_token, maybe_relative_path_or_url)
    }

    pub fn document_version(&self, target_uri: &Url) -> SourceFileVersion {
        self.document_version_by_path(&uri_to_file(target_uri).unwrap_or_default())
    }

    pub fn document_version_by_path(&self, path: &Path) -> SourceFileVersion {
        self.source_file_versions.borrow().get(path).and_then(|v| *v)
    }

    pub fn get_document<'a>(&'a self, url: &'_ Url) -> Option<&'a Document> {
        let path = uri_to_file(url)?;
        self.type_loader.get_document(&path)
    }

    fn uses_widgets_impl(&self, doc_path: PathBuf, dedup: &mut HashSet<PathBuf>) -> bool {
        if dedup.contains(&doc_path) {
            return false;
        }

        if doc_path.starts_with("builtin:/") && doc_path.ends_with("std-widgets.slint") {
            return true;
        }

        let Some(doc) = self.get_document_by_path(&doc_path) else {
            return false;
        };

        dedup.insert(doc_path.to_path_buf());

        for import in doc.imports.iter().map(|i| PathBuf::from(&i.file)) {
            if self.uses_widgets_impl(import, dedup) {
                return true;
            }
        }

        false
    }

    /// Returns true if doc_url uses (possibly indirectly) widgets from "std-widgets.slint"
    pub fn uses_widgets(&self, doc_url: &Url) -> bool {
        let Some(doc_path) = uri_to_file(doc_url) else {
            return false;
        };

        let mut dedup = HashSet::new();

        self.uses_widgets_impl(doc_path, &mut dedup)
    }

    pub fn get_document_by_path<'a>(&'a self, path: &'_ Path) -> Option<&'a Document> {
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
        let o = (doc
            .node
            .as_ref()?
            .source_file
            .offset(pos.line as usize + 1, pos.character as usize + 1) as u32)
            .into();
        doc.node.as_ref()?.text_range().contains_inclusive(o).then_some((doc, o))
    }

    pub fn all_url_documents(&self) -> impl Iterator<Item = (Url, &syntax_nodes::Document)> + '_ {
        self.type_loader.all_file_documents().filter_map(|(p, d)| Some((file_to_uri(p)?, d)))
    }

    pub fn all_urls(&self) -> impl Iterator<Item = Url> + '_ {
        self.type_loader.all_files().filter_map(|p| file_to_uri(p))
    }

    pub fn global_type_registry(&self) -> std::cell::Ref<TypeRegister> {
        self.type_loader.global_type_registry.borrow()
    }

    fn invalidate_everything(&mut self) {
        let all_files = self.type_loader.all_files().cloned().collect::<Vec<_>>();

        for path in all_files {
            self.type_loader.invalidate_document(&path);
        }
    }

    pub async fn reconfigure(
        &mut self,
        style: Option<String>,
        include_paths: Option<Vec<PathBuf>>,
        library_paths: Option<HashMap<String, PathBuf>>,
    ) -> Result<CompilerConfiguration> {
        if style.is_none() && include_paths.is_none() && library_paths.is_none() {
            return Ok(self.compiler_configuration());
        }

        if let Some(s) = style {
            if s.is_empty() {
                self.type_loader.compiler_config.style = None;
            } else {
                self.type_loader.compiler_config.style = Some(s);
            }
        }

        if let Some(ip) = include_paths {
            self.type_loader.compiler_config.include_paths = ip;
        }

        if let Some(lp) = library_paths {
            self.type_loader.compiler_config.library_paths = lp;
        }

        self.invalidate_everything();

        self.preload_builtins().await;

        Ok(self.compiler_configuration())
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
        let path =
            uri_to_file(url).ok_or_else(|| format!("Failed to convert path for loading: {url}"))?;
        self.type_loader.load_file(&path, &path, content, false, diag).await;
        self.source_file_versions.borrow_mut().insert(path, version);
        Ok(())
    }

    pub async fn reload_cached_file(&mut self, url: &Url, diag: &mut BuildDiagnostics) {
        let Some(path) = uri_to_file(url) else { return };
        self.type_loader.reload_cached_file(&path, diag).await;
    }

    pub fn drop_document(&mut self, url: &Url) -> Result<()> {
        let Some(path) = uri_to_file(url) else {
            // This isn't fatal, but we might want to learn about paths/schemes to support in the future.
            eprintln!("Failed to convert path for dropping document: {url}");
            return Ok(());
        };
        Ok(self.type_loader.drop_document(&path)?)
    }

    /// Invalidate a document and all its dependencies.
    /// return the list of dependencies that were invalidated.
    pub fn invalidate_url(&mut self, url: &Url) -> HashSet<Url> {
        let Some(path) = uri_to_file(url) else { return HashSet::new() };
        self.type_loader
            .invalidate_document(&path)
            .into_iter()
            .filter_map(|x| file_to_uri(&x))
            .collect()
    }

    pub fn compiler_configuration(&self) -> CompilerConfiguration {
        CompilerConfiguration {
            include_paths: self.type_loader.compiler_config.include_paths.clone(),
            library_paths: self.type_loader.compiler_config.library_paths.clone(),
            style: self.type_loader.compiler_config.style.clone(),
            open_import_fallback: None, // We need to re-generate this anyway
            resource_url_mapper: self.type_loader.compiler_config.resource_url_mapper.clone(),
        }
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
}

#[cfg(test)]
mod tests {
    use crate::language::test::complex_document_cache;

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
        Some(format!("{}", &element.base_type))
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
