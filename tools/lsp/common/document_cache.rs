// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Data structures common between LSP and previewer

use i_slint_compiler::diagnostics::{BuildDiagnostics, SourceFile};
use i_slint_compiler::object_tree::Document;
use i_slint_compiler::typeloader::TypeLoader;
use i_slint_compiler::typeregister::TypeRegister;
use i_slint_compiler::CompilerConfiguration;
use lsp_types::Url;

use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

pub type SourceFileVersion = Option<i32>;

use crate::common::{file_to_uri, uri_to_file, ElementRcNode, Result};

/// A cache of loaded documents
pub struct DocumentCache(TypeLoader);

impl DocumentCache {
    pub fn new(config: CompilerConfiguration) -> Self {
        Self(TypeLoader::new(
            i_slint_compiler::typeregister::TypeRegister::builtin(),
            config,
            &mut BuildDiagnostics::default(),
        ))
    }

    pub fn new_from_type_loader(type_loader: TypeLoader) -> Self {
        Self(type_loader)
    }

    pub fn snapshot(&self) -> Option<Self> {
        i_slint_compiler::typeloader::snapshot(&self.0).map(Self::new_from_type_loader)
    }

    pub fn resolve_import_path(
        &self,
        import_token: Option<&i_slint_compiler::parser::NodeOrToken>,
        maybe_relative_path_or_url: &str,
    ) -> Option<(PathBuf, Option<&'static [u8]>)> {
        self.0.resolve_import_path(import_token, maybe_relative_path_or_url)
    }

    pub fn document_version(&self, target_uri: &Url) -> SourceFileVersion {
        self.document_version_by_path(&uri_to_file(target_uri).unwrap_or_default())
    }

    pub fn document_version_by_path(&self, path: &Path) -> SourceFileVersion {
        self.0.get_document(&path).and_then(|doc| doc.node.as_ref()?.source_file.version())
    }

    pub fn get_document<'a>(&'a self, url: &'_ Url) -> Option<&'a Document> {
        let path = uri_to_file(url)?;
        self.0.get_document(&path)
    }

    pub fn get_document_by_path<'a>(&'a self, path: &'_ Path) -> Option<&'a Document> {
        self.0.get_document(path)
    }

    pub fn get_document_for_source_file<'a>(
        &'a self,
        source_file: &'_ SourceFile,
    ) -> Option<&'a Document> {
        self.0.get_document(source_file.path())
    }

    pub fn get_document_and_offset<'a>(
        &'a self,
        text_document_uri: &'_ Url,
        pos: &'_ lsp_types::Position,
    ) -> Option<(&'a i_slint_compiler::object_tree::Document, u32)> {
        let doc = self.get_document(text_document_uri)?;
        let o = doc
            .node
            .as_ref()?
            .source_file
            .offset(pos.line as usize + 1, pos.character as usize + 1) as u32;
        doc.node.as_ref()?.text_range().contains_inclusive(o.into()).then_some((doc, o))
    }

    pub fn all_url_documents(&self) -> impl Iterator<Item = (Url, &Document)> + '_ {
        self.0.all_file_documents().filter_map(|(p, d)| Some((file_to_uri(p)?, d)))
    }

    pub fn all_urls(&self) -> impl Iterator<Item = Url> + '_ {
        self.0.all_files().filter_map(|p| file_to_uri(p))
    }

    pub fn global_type_registry(&self) -> std::cell::Ref<TypeRegister> {
        self.0.global_type_registry.borrow()
    }

    pub async fn reconfigure(
        &mut self,
        style: Option<String>,
        include_paths: Option<Vec<PathBuf>>,
        library_paths: Option<HashMap<String, PathBuf>>,
    ) -> Result<CompilerConfiguration> {
        if style.is_none() && include_paths.is_none() && library_paths.is_none() {
            return Ok(self.0.compiler_config.clone());
        }

        if let Some(s) = style {
            if s.is_empty() {
                self.0.compiler_config.style = None;
            } else {
                self.0.compiler_config.style = Some(s);
            }
        }

        if let Some(ip) = include_paths {
            self.0.compiler_config.include_paths = ip;
        }

        if let Some(lp) = library_paths {
            self.0.compiler_config.library_paths = lp;
        }

        self.preload_builtins().await;

        Ok(self.0.compiler_config.clone())
    }

    pub async fn preload_builtins(&mut self) {
        // Always load the widgets so we can auto-complete them
        let mut diag = BuildDiagnostics::default();
        self.0.import_component("std-widgets.slint", "StyleMetrics", &mut diag).await;
        assert!(!diag.has_errors());
    }

    pub async fn load_url(
        &mut self,
        url: &Url,
        version: SourceFileVersion,
        content: String,
        diag: &mut BuildDiagnostics,
    ) -> Result<()> {
        let path = uri_to_file(url).ok_or("Failed to convert path")?;
        self.0.load_file(&path, version, &path, content, false, diag).await;
        Ok(())
    }

    pub fn compiler_configuration(&self) -> &CompilerConfiguration {
        &self.0.compiler_config
    }

    fn element_at_document_and_offset(
        &self,
        document: &i_slint_compiler::object_tree::Document,
        offset: u32,
    ) -> Option<ElementRcNode> {
        fn element_contains(
            element: &i_slint_compiler::object_tree::ElementRc,
            offset: u32,
        ) -> Option<usize> {
            element.borrow().debug.iter().position(|n| {
                n.node.parent().map_or(false, |n| n.text_range().contains(offset.into()))
            })
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

    pub fn element_at_offset(&self, text_document_uri: &Url, offset: u32) -> Option<ElementRcNode> {
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
    use crate::test::complex_document_cache;

    use super::*;

    fn id_at_position(dc: &DocumentCache, url: &Url, line: u32, character: u32) -> Option<String> {
        let result = dc.element_at_position(url, &lsp_types::Position { line, character })?;
        let element = result.element.borrow();
        Some(element.id.clone())
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
