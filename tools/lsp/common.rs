// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Data structures common between LSP and previewer

use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, TextSize};
use lsp_types::{TextEdit, Url, WorkspaceEdit};

use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

pub mod component_catalog;
pub mod document_cache;
pub use document_cache::{DocumentCache, SourceFileVersion};
pub mod rename_component;
#[cfg(test)]
pub mod test;
#[cfg(any(test, feature = "preview-engine"))]
pub mod text_edit;
pub mod token_info;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;
#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

/// Use this in nodes you want the language server and preview to
/// ignore a node for code analysis purposes.
pub const NODE_IGNORE_COMMENT: &str = "@lsp:ignore-node";

/// Check whether a node is marked to be ignored in the LSP/live preview
/// using a comment containing `@lsp:ignore-node`
pub fn is_element_node_ignored(node: &syntax_nodes::Element) -> bool {
    node.children_with_tokens().any(|nt| {
        nt.as_token()
            .map(|t| t.kind() == SyntaxKind::Comment && t.text().contains(NODE_IGNORE_COMMENT))
            .unwrap_or(false)
    })
}

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
        self.path_and_offset() == other.path_and_offset() && self.debug_index == other.debug_index
    }
}

impl std::fmt::Debug for ElementRcNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (path, offset) = self.path_and_offset();
        write!(f, "ElementNode {{ {path:?}:{offset:?} }}")
    }
}

impl ElementRcNode {
    pub fn new(element: ElementRc, debug_index: usize) -> Option<Self> {
        let _ = element.borrow().debug.get(debug_index)?;

        Some(Self { element, debug_index })
    }

    pub fn in_document_cache(&self, document_cache: &DocumentCache) -> Option<Self> {
        self.with_element_node(|en| {
            let element_start = en.text_range().start();
            let path = en.source_file.path();

            let doc = document_cache.get_document_by_path(path)?;
            let component = doc.inner_components.iter().find(|c| {
                let Some(c_node) = &c.node else {
                    return false;
                };
                c_node.text_range().contains(element_start)
            })?;
            ElementRcNode::find_in_or_below(
                component.root_element.clone(),
                path,
                u32::from(element_start),
            )
        })
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
        func: impl Fn(&i_slint_compiler::object_tree::ElementDebugInfo) -> R,
    ) -> R {
        let elem = self.element.borrow();
        let d = elem.debug.get(self.debug_index).unwrap();
        func(d)
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

    pub fn path_and_offset(&self) -> (PathBuf, TextSize) {
        self.with_element_node(|n| (n.source_file.path().to_owned(), n.text_range().start()))
    }

    pub fn as_element(&self) -> &ElementRc {
        &self.element
    }

    pub fn parent(&self) -> Option<ElementRcNode> {
        let mut ancestor = self.with_element_node(|node| node.parent());

        while let Some(parent) = ancestor {
            if parent.kind() != SyntaxKind::Element {
                ancestor = parent.parent();
                continue;
            }

            let (parent_path, parent_offset) =
                (parent.source_file.path().to_owned(), u32::from(parent.text_range().start()));

            ancestor = parent.parent();

            let component = self.element.borrow().enclosing_component.upgrade().unwrap();
            let current_root = component.root_element.clone();
            let root_element = if std::rc::Rc::ptr_eq(&current_root, &self.element) {
                component.parent_element.upgrade().map_or(current_root, |parent| {
                    parent.borrow().enclosing_component.upgrade().unwrap().root_element.clone()
                })
            } else {
                current_root
            };

            let result = Self::find_in_or_below(root_element, &parent_path, parent_offset);

            if result.is_some() {
                return result;
            }
        }

        None
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

    pub fn contains_offset(&self, offset: TextSize) -> bool {
        self.with_element_node(|node| {
            node.parent().is_some_and(|n| n.text_range().contains(offset))
        })
    }
}

pub struct SingleTextEdit {
    pub url: Url,
    pub version: SourceFileVersion,
    pub edit: TextEdit,
}

impl SingleTextEdit {
    pub fn from_path(document_cache: &DocumentCache, path: &Path, edit: TextEdit) -> Option<Self> {
        let url = Url::from_file_path(path).ok()?;
        let version = document_cache.document_version_by_path(path);
        Some(Self { url, version, edit })
    }
}

pub fn create_text_document_edit(
    uri: Url,
    version: SourceFileVersion,
    edits: Vec<TextEdit>,
) -> lsp_types::TextDocumentEdit {
    let edits = edits
        .into_iter()
        .map(lsp_types::OneOf::Left::<TextEdit, lsp_types::AnnotatedTextEdit>)
        .collect();
    lsp_types::TextDocumentEdit {
        text_document: lsp_types::OptionalVersionedTextDocumentIdentifier { uri, version },
        edits,
    }
}

pub fn create_workspace_edit_from_path(
    document_cache: &DocumentCache,
    path: &Path,
    edits: Vec<TextEdit>,
) -> Option<WorkspaceEdit> {
    let url = Url::from_file_path(path).ok()?;
    let version = document_cache.document_version_by_path(path);
    Some(create_workspace_edit(url, version, edits))
}

pub fn create_workspace_edit(
    url: Url,
    version: SourceFileVersion,
    edits: Vec<TextEdit>,
) -> WorkspaceEdit {
    create_workspace_edit_from_text_document_edits(vec![create_text_document_edit(
        url, version, edits,
    )])
}

pub fn create_workspace_edit_from_text_document_edits(
    edits: Vec<lsp_types::TextDocumentEdit>,
) -> WorkspaceEdit {
    let document_changes = Some(lsp_types::DocumentChanges::Edits(edits));
    WorkspaceEdit { document_changes, ..Default::default() }
}

pub fn create_workspace_edit_from_single_text_edits(
    mut inputs: Vec<SingleTextEdit>,
) -> WorkspaceEdit {
    let mut files: HashMap<
        (Url, SourceFileVersion),
        Vec<lsp_types::OneOf<TextEdit, lsp_types::AnnotatedTextEdit>>,
    > = HashMap::new();
    inputs.drain(..).for_each(|se| {
        let edit = lsp_types::OneOf::Left(se.edit);
        files
            .entry((se.url, se.version))
            .and_modify(|v| v.push(edit.clone()))
            .or_insert_with(|| vec![edit]);
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

    WorkspaceEdit { document_changes: Some(changes), ..Default::default() }
}

/// A versioned file
#[derive(Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct VersionedUrl {
    /// The file url
    url: Url,
    // The file version
    version: SourceFileVersion,
}

impl VersionedUrl {
    pub fn new(url: Url, version: SourceFileVersion) -> Self {
        VersionedUrl { url, version }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn version(&self) -> &SourceFileVersion {
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
    url: Url,
    /// The offset in the file pointed to by the `url`
    offset: u32,
}

#[allow(unused)]
impl Position {
    pub fn new(url: Url, offset: TextSize) -> Self {
        Self { url, offset: offset.into() }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn offset(&self) -> TextSize {
        self.offset.into()
    }
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
    pub fn new(url: VersionedUrl, offset: TextSize) -> Self {
        Self { url, offset: offset.into() }
    }

    pub fn url(&self) -> &Url {
        self.url.url()
    }

    pub fn version(&self) -> &SourceFileVersion {
        self.url.version()
    }

    pub fn offset(&self) -> TextSize {
        self.offset.into()
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
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
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
    InvalidateContents { url: lsp_types::Url },
    ForgetFile { url: lsp_types::Url },
    SetContents { url: VersionedUrl, contents: String },
    SetConfiguration { config: PreviewConfig },
    ShowPreview(PreviewComponent),
    HighlightFromEditor { url: Option<Url>, offset: u32 },
}

impl lsp_types::notification::Notification for LspToPreviewMessage {
    type Params = Self;
    const METHOD: &'static str = "slint/lsp_to_preview";
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
    /// Report diagnostics to editor.
    Diagnostics { uri: Url, version: SourceFileVersion, diagnostics: Vec<lsp_types::Diagnostic> },
    /// Show a document in the editor.
    ShowDocument { file: Url, selection: lsp_types::Range, take_focus: bool },
    /// Switch between native and WASM preview (if supported)
    PreviewTypeChanged { is_external: bool },
    /// Request all documents and configuration to be sent from the LSP to the
    /// Preview.
    RequestState { unused: bool },
    /// Pass a `WorkspaceEdit` on to the editor
    SendWorkspaceEdit { label: Option<String>, edit: lsp_types::WorkspaceEdit },
    /// Pass a `ShowMessage` notification on to the editor
    SendShowMessage { message: lsp_types::ShowMessageParams },
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
    /// This is a primitive element that reacts to events in some way
    pub is_interactive: bool,
    /// This is a layout
    pub is_layout: bool,
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
            if let Some(path) = url.path().strip_prefix("/@") {
                Some(format!("@{path}"))
            } else if let Some(current_uri) = current_uri {
                lsp_types::Url::make_relative(current_uri, url)
            } else {
                url.to_file_path().ok().map(|p| p.to_string_lossy().to_string())
            }
        }
    }
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub mod lsp_to_editor {
    pub fn notify_lsp_diagnostics(
        sender: &crate::ServerNotifier,
        uri: lsp_types::Url,
        version: super::SourceFileVersion,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) -> Option<()> {
        sender
            .send_notification::<lsp_types::notification::PublishDiagnostics>(
                lsp_types::PublishDiagnosticsParams { uri, diagnostics, version },
            )
            .ok()
    }

    fn show_document_request_from_element_callback(
        uri: lsp_types::Url,
        range: lsp_types::Range,
        take_focus: bool,
    ) -> Option<lsp_types::ShowDocumentParams> {
        if range.start.character == 0 || range.end.character == 0 {
            return None;
        }

        Some(lsp_types::ShowDocumentParams {
            uri,
            external: Some(false),
            take_focus: Some(take_focus),
            selection: Some(range),
        })
    }

    pub async fn send_show_document_to_editor(
        sender: crate::ServerNotifier,
        file: lsp_types::Url,
        range: lsp_types::Range,
        take_focus: bool,
    ) {
        let Some(params) = show_document_request_from_element_callback(file, range, take_focus)
        else {
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
