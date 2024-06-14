// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Data structures common between LSP and previewer

use i_slint_compiler::diagnostics::{BuildDiagnostics, SourceFile, SourceFileVersion};
use i_slint_compiler::object_tree::{Document, ElementRc};
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode};
use i_slint_compiler::typeloader::TypeLoader;
use i_slint_compiler::typeregister::TypeRegister;
use i_slint_compiler::CompilerConfiguration;
use lsp_types::{TextEdit, Url, WorkspaceEdit};

use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

pub mod component_catalog;
pub mod rename_component;
#[cfg(test)]
pub mod test;
#[cfg(any(test, feature = "preview-engine"))]
pub mod text_edit;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;
pub type UrlVersion = Option<i32>;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub fn uri_to_file(uri: &Url) -> Option<PathBuf> {
    if uri.scheme() == "builtin" {
        Some(PathBuf::from(uri.to_string()))
    } else {
        let path = uri.to_file_path().ok()?;
        let cleaned_path = i_slint_compiler::pathutils::clean_path(&path);
        Some(cleaned_path)
    }
}

pub fn file_to_uri(path: &Path) -> Option<Url> {
    if path.starts_with("builtin:/") {
        Url::parse(path.to_str()?).ok()
    } else {
        Url::from_file_path(path).ok()
    }
}

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
        self.0
            .get_document(&uri_to_file(target_uri).unwrap_or_default())
            .and_then(|doc| doc.node.as_ref()?.source_file.version())
    }

    pub fn get_document(&self, url: &Url) -> Option<&Document> {
        let path = uri_to_file(url)?;
        self.0.get_document(&path)
    }

    pub fn get_document_by_path(&self, path: &Path) -> Option<&Document> {
        self.0.get_document(path)
    }

    pub fn get_document_for_source_file(&self, source_file: &SourceFile) -> Option<&Document> {
        self.0.get_document(source_file.path())
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
        assert!(!diag.has_error());
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
}

pub fn extract_element(node: SyntaxNode) -> Option<syntax_nodes::Element> {
    match node.kind() {
        SyntaxKind::Element => Some(node.into()),
        SyntaxKind::SubElement => extract_element(node.child_node(SyntaxKind::Element)?),
        SyntaxKind::ConditionalElement | SyntaxKind::RepeatedElement => {
            extract_element(node.child_node(SyntaxKind::SubElement)?)
        }
        _ => None,
    }
}

fn find_element_with_decoration(element: &syntax_nodes::Element) -> SyntaxNode {
    let this_node: SyntaxNode = element.clone().into();
    element
        .parent()
        .and_then(|p| match p.kind() {
            SyntaxKind::SubElement => p.parent().map(|gp| {
                if gp.kind() == SyntaxKind::ConditionalElement
                    || gp.kind() == SyntaxKind::RepeatedElement
                {
                    gp
                } else {
                    p
                }
            }),
            _ => Some(this_node.clone()),
        })
        .unwrap_or(this_node)
}

fn find_parent_component(node: &SyntaxNode) -> Option<SyntaxNode> {
    let mut current = Some(node.clone());
    while let Some(p) = current {
        if matches!(p.kind(), SyntaxKind::Component) {
            return Some(p);
        }
        current = p.parent();
    }
    None
}

#[derive(Clone)]
pub struct ElementRcNode {
    pub element: ElementRc,
    pub debug_index: usize,
}

impl std::cmp::PartialEq for ElementRcNode {
    fn eq(&self, other: &Self) -> bool {
        self.path_and_offset() == other.path_and_offset()
    }
}

impl std::fmt::Debug for ElementRcNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (path, offset) = self.path_and_offset();
        write!(f, "ElementNode {{ {path:?}:{offset} }}")
    }
}

impl ElementRcNode {
    pub fn new(element: ElementRc, debug_index: usize) -> Option<Self> {
        let _ = element.borrow().debug.get(debug_index)?;

        Some(Self { element, debug_index })
    }

    /// Some nodes get merged into the same ElementRc with no real connections between them...
    pub fn next_element_rc_node(&self) -> Option<Self> {
        Self::new(self.element.clone(), self.debug_index + 1)
    }

    pub fn find_in(element: ElementRc, path: &Path, offset: u32) -> Option<Self> {
        let debug_index = element.borrow().debug.iter().position(|d| {
            u32::from(d.node.text_range().start()) == offset && d.node.source_file.path() == path
        })?;

        Some(Self { element, debug_index })
    }

    pub fn find_in_or_below(element: ElementRc, path: &Path, offset: u32) -> Option<Self> {
        let debug_index = element.borrow().debug.iter().position(|d| {
            u32::from(d.node.text_range().start()) == offset && d.node.source_file.path() == path
        });
        if let Some(debug_index) = debug_index {
            Some(Self { element, debug_index })
        } else {
            for c in &element.borrow().children {
                let result = Self::find_in_or_below(c.clone(), path, offset);
                if result.is_some() {
                    return result;
                }
            }
            None
        }
    }

    /// Run with all the debug information on the node
    pub fn with_element_debug<R>(
        &self,
        func: impl Fn(
            &i_slint_compiler::parser::syntax_nodes::Element,
            &Option<i_slint_compiler::layout::Layout>,
        ) -> R,
    ) -> R {
        let elem = self.element.borrow();
        let d = &elem.debug.get(self.debug_index).unwrap();
        func(&d.node, &d.layout)
    }

    /// Run with the `Element` node
    pub fn with_element_node<R>(
        &self,
        func: impl Fn(&i_slint_compiler::parser::syntax_nodes::Element) -> R,
    ) -> R {
        let elem = self.element.borrow();
        func(&elem.debug.get(self.debug_index).unwrap().node)
    }

    /// Run with the SyntaxNode incl. any id, condition, etc.
    pub fn with_decorated_node<R>(&self, func: impl Fn(SyntaxNode) -> R) -> R {
        let elem = self.element.borrow();
        func(find_element_with_decoration(&elem.debug.get(self.debug_index).unwrap().node))
    }

    pub fn path_and_offset(&self) -> (PathBuf, u32) {
        self.with_element_node(|n| {
            (n.source_file.path().to_owned(), u32::from(n.text_range().start()))
        })
    }

    pub fn as_element(&self) -> &ElementRc {
        &self.element
    }

    pub fn parent(&self) -> Option<ElementRcNode> {
        let parent = self.with_element_node(|node| {
            let mut ancestor = node.parent()?;
            loop {
                if ancestor.kind() == SyntaxKind::Element {
                    return Some(ancestor);
                }
                ancestor = ancestor.parent()?;
            }
        })?;

        let (parent_path, parent_offset) =
            (parent.source_file.path().to_owned(), u32::from(parent.text_range().start()));
        let root_element = {
            let component = self.element.borrow().enclosing_component.upgrade().unwrap();
            let current_root = component.root_element.clone();

            if std::rc::Rc::ptr_eq(&current_root, &self.element)
                && !component.is_root_component.get()
            {
                component
                    .parent_element
                    .upgrade()?
                    .borrow()
                    .enclosing_component
                    .upgrade()
                    .unwrap()
                    .root_element
                    .clone()
            } else {
                current_root
            }
        };

        Self::find_in_or_below(root_element, &parent_path, parent_offset)
    }

    pub fn children(&self) -> Vec<ElementRcNode> {
        self.with_element_node(|node| {
            let mut children = Vec::new();
            for c in node.children() {
                if let Some(element) = extract_element(c.clone()) {
                    let e_path = element.source_file.path().to_path_buf();
                    let e_offset = u32::from(element.text_range().start());

                    let Some(child_node) = ElementRcNode::find_in_or_below(
                        self.as_element().clone(),
                        &e_path,
                        e_offset,
                    ) else {
                        continue;
                    };
                    children.push(child_node);
                }
            }

            children
        })
    }

    pub fn component_type(&self) -> String {
        self.with_element_node(|node| {
            node.QualifiedName().map(|qn| qn.text().to_string()).unwrap_or_default()
        })
    }

    pub fn is_same_component_as(&self, other: &Self) -> bool {
        let Some(s) = self.with_element_node(|n| find_parent_component(n)) else {
            return false;
        };
        let Some(o) = other.with_element_node(|n| find_parent_component(n)) else {
            return false;
        };

        std::rc::Rc::ptr_eq(&s.source_file, &o.source_file) && s.text_range() == o.text_range()
    }
}

pub fn create_workspace_edit(uri: Url, version: UrlVersion, edits: Vec<TextEdit>) -> WorkspaceEdit {
    let edits = edits
        .into_iter()
        .map(lsp_types::OneOf::Left::<TextEdit, lsp_types::AnnotatedTextEdit>)
        .collect();
    let edit = lsp_types::TextDocumentEdit {
        text_document: lsp_types::OptionalVersionedTextDocumentIdentifier { uri, version },
        edits,
    };
    let changes = lsp_types::DocumentChanges::Edits(vec![edit]);
    WorkspaceEdit { document_changes: Some(changes), ..Default::default() }
}

pub fn create_workspace_edit_from_source_file(
    source_file: &SourceFile,
    edits: Vec<TextEdit>,
) -> Option<WorkspaceEdit> {
    Some(create_workspace_edit(
        Url::from_file_path(source_file.path()).ok()?,
        source_file.version(),
        edits,
    ))
}

pub fn create_workspace_edit_from_source_files(
    mut inputs: Vec<(SourceFile, TextEdit)>,
) -> Option<WorkspaceEdit> {
    let mut files: HashMap<
        (Url, UrlVersion),
        Vec<lsp_types::OneOf<TextEdit, lsp_types::AnnotatedTextEdit>>,
    > = HashMap::new();
    inputs.drain(..).for_each(|(sf, edit)| {
        let url = Url::from_file_path(sf.path()).ok();
        if let Some(url) = url {
            let edit = lsp_types::OneOf::Left(edit);
            files
                .entry((url, sf.version()))
                .and_modify(|v| v.push(edit.clone()))
                .or_insert_with(|| vec![edit]);
        }
    });

    let changes = lsp_types::DocumentChanges::Edits(
        files
            .drain()
            .map(|((uri, version), edits)| lsp_types::TextDocumentEdit {
                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier { uri, version },
                edits,
            })
            .collect::<Vec<_>>(),
    );

    Some(WorkspaceEdit { document_changes: Some(changes), ..Default::default() })
}

/// A versioned file
#[derive(Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct VersionedUrl {
    /// The file url
    url: Url,
    // The file version
    version: UrlVersion,
}

impl VersionedUrl {
    pub fn new(url: Url, version: UrlVersion) -> Self {
        VersionedUrl { url, version }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn version(&self) -> &UrlVersion {
        &self.version
    }
}

impl std::fmt::Debug for VersionedUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self.version.map(|v| format!("v{v}")).unwrap_or_else(|| "none".to_string());
        write!(f, "{}@{}", self.url, version)
    }
}

/// A versioned file
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Position {
    /// The file url
    pub url: Url,
    /// The offset in the file pointed to by the `url`
    pub offset: u32,
}

/// A versioned file
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct VersionedPosition {
    /// The file url
    url: VersionedUrl,
    /// The offset in the file pointed to by the `url`
    offset: u32,
}

#[allow(unused)]
impl VersionedPosition {
    pub fn new(url: VersionedUrl, offset: u32) -> Self {
        VersionedPosition { url, offset }
    }

    pub fn url(&self) -> &Url {
        self.url.url()
    }

    pub fn version(&self) -> &UrlVersion {
        self.url.version()
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }
}

#[derive(Default, Clone, PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct PreviewConfig {
    pub hide_ui: Option<bool>,
    pub style: String,
    pub include_paths: Vec<PathBuf>,
    pub library_paths: HashMap<String, PathBuf>,
}

/// The Component to preview
#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PreviewComponent {
    /// The file name to preview
    pub url: Url,
    /// The name of the component within that file.
    /// If None, then the last component is going to be shown.
    pub component: Option<String>,

    /// The style name for the preview
    pub style: String,
}

#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum LspToPreviewMessage {
    SetContents { url: VersionedUrl, contents: String },
    SetConfiguration { config: PreviewConfig },
    ShowPreview(PreviewComponent),
    HighlightFromEditor { url: Option<Url>, offset: u32 },
}

#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Diagnostic {
    pub message: String,
    pub file: Option<String>,
    pub line: usize,
    pub column: usize,
    pub level: String,
}

#[allow(unused)]
#[derive(Clone, Eq, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PropertyChange {
    pub name: String,
    pub value: String,
}

impl PropertyChange {
    #[allow(unused)]
    pub fn new(name: &str, value: String) -> Self {
        PropertyChange { name: name.to_string(), value }
    }
}

#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PreviewToLspMessage {
    /// Show a status message in the editor
    Status { message: String, health: crate::lsp_ext::Health },
    /// Report diagnostics to editor.
    Diagnostics { uri: Url, diagnostics: Vec<lsp_types::Diagnostic> },
    /// Show a document in the editor.
    ShowDocument { file: Url, selection: lsp_types::Range },
    /// Switch between native and WASM preview (if supported)
    PreviewTypeChanged { is_external: bool },
    /// Request all documents and configuration to be sent from the LSP to the
    /// Preview.
    RequestState { unused: bool },
    /// Update properties on an element at `position`
    /// The LSP side needs to look at properties: It sees way more of them!
    UpdateElement {
        label: Option<String>,
        position: VersionedPosition,
        properties: Vec<PropertyChange>,
    },
    /// Pass a `WorkspaceEdit` on to the editor
    SendWorkspaceEdit { label: Option<String>, edit: lsp_types::WorkspaceEdit },
}

/// Information on the Element types available
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct ComponentInformation {
    /// The name of the type
    pub name: String,
    /// A broad category to group types by
    pub category: String,
    /// This type is a global component
    pub is_global: bool,
    /// This type is built into Slint
    pub is_builtin: bool,
    /// This type is a standard widget
    pub is_std_widget: bool,
    /// This type was exported
    pub is_exported: bool,
    /// This is a layout
    pub is_layout: bool,
    /// This element fills its parent
    pub fills_parent: bool,
    /// The URL to the file containing this type
    pub defined_at: Option<Position>,
    /// Default property values
    pub default_properties: Vec<PropertyChange>,
}

impl ComponentInformation {
    pub fn import_file_name(&self, current_uri: &Option<lsp_types::Url>) -> Option<String> {
        if self.is_std_widget {
            Some("std-widgets.slint".to_string())
        } else {
            let url = self.defined_at.as_ref().map(|p| &p.url)?;
            if let Some(current_uri) = current_uri {
                lsp_types::Url::make_relative(current_uri, url)
            } else {
                url.to_file_path().ok().map(|p| p.to_string_lossy().to_string())
            }
        }
    }
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub mod lsp_to_editor {
    use lsp_types::notification::Notification;

    pub fn send_status_notification(
        sender: &crate::ServerNotifier,
        message: &str,
        health: crate::lsp_ext::Health,
    ) {
        sender
            .send_notification(
                crate::lsp_ext::ServerStatusNotification::METHOD.into(),
                crate::lsp_ext::ServerStatusParams {
                    health,
                    quiescent: false,
                    message: Some(message.into()),
                },
            )
            .unwrap_or_else(|e| eprintln!("Error sending notification: {:?}", e));
    }

    pub fn notify_lsp_diagnostics(
        sender: &crate::ServerNotifier,
        uri: lsp_types::Url,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) -> Option<()> {
        sender
            .send_notification(
                "textDocument/publishDiagnostics".into(),
                lsp_types::PublishDiagnosticsParams { uri, diagnostics, version: None },
            )
            .ok()
    }

    fn show_document_request_from_element_callback(
        uri: lsp_types::Url,
        range: lsp_types::Range,
    ) -> Option<lsp_types::ShowDocumentParams> {
        if range.start.character == 0 || range.end.character == 0 {
            return None;
        }

        Some(lsp_types::ShowDocumentParams {
            uri,
            external: Some(false),
            take_focus: Some(true),
            selection: Some(range),
        })
    }

    pub async fn send_show_document_to_editor(
        sender: crate::ServerNotifier,
        file: lsp_types::Url,
        range: lsp_types::Range,
    ) {
        let Some(params) = show_document_request_from_element_callback(file, range) else {
            return;
        };
        let Ok(fut) = sender.send_request::<lsp_types::request::ShowDocument>(params) else {
            return;
        };

        let _ = fut.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_conversion_of_builtins() {
        let builtin_path = PathBuf::from("builtin:/fluent/button.slint");
        let url = file_to_uri(&builtin_path).unwrap();
        assert_eq!(url.scheme(), "builtin");

        let back_conversion = uri_to_file(&url).unwrap();
        assert_eq!(back_conversion, builtin_path);

        assert!(Url::from_file_path(&builtin_path).is_err());
    }

    #[test]
    fn test_uri_conversion_of_slashed_builtins() {
        let builtin_path1 = PathBuf::from("builtin:/fluent/button.slint");
        let builtin_path3 = PathBuf::from("builtin:///fluent/button.slint");

        let url1 = file_to_uri(&builtin_path1).unwrap();
        let url3 = file_to_uri(&builtin_path3).unwrap();
        assert_ne!(url1, url3);

        let back_conversion1 = uri_to_file(&url1).unwrap();
        let back_conversion3 = uri_to_file(&url3).unwrap();
        assert_eq!(back_conversion1, back_conversion3);

        assert_eq!(back_conversion1, builtin_path1);
    }
}
