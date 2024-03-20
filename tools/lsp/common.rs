// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Data structures common between LSP and previewer

use i_slint_compiler::diagnostics::{SourceFile, SourceFileVersion};
use i_slint_compiler::object_tree::ElementRc;
use lsp_types::{TextEdit, Url, WorkspaceEdit};

use std::{collections::HashMap, path::PathBuf};

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;
pub type UrlVersion = Option<i32>;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

#[derive(Clone)]
pub struct ElementRcNode {
    pub element: ElementRc,
    pub debug_index: usize,
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

    pub fn find_in(element: ElementRc, path: &std::path::Path, offset: u32) -> Option<Self> {
        let debug_index = element.borrow().debug.iter().position(|(n, _)| {
            u32::from(n.text_range().start()) == offset && n.source_file.path() == path
        })?;

        Some(Self { element, debug_index })
    }

    pub fn with_element_debug<R>(
        &self,
        func: impl Fn(
            &i_slint_compiler::parser::syntax_nodes::Element,
            &Option<i_slint_compiler::layout::Layout>,
        ) -> R,
    ) -> R {
        let elem = self.element.borrow();
        let (n, l) = &elem.debug.get(self.debug_index).unwrap();
        func(n, l)
    }

    pub fn with_element_node<R>(
        &self,
        func: impl Fn(&i_slint_compiler::parser::syntax_nodes::Element) -> R,
    ) -> R {
        let elem = self.element.borrow();
        func(&elem.debug.get(self.debug_index).unwrap().0)
    }

    pub fn path_and_offset(&self) -> (PathBuf, u32) {
        self.with_element_node(|n| {
            (n.source_file.path().to_owned(), u32::from(n.text_range().start()))
        })
    }

    pub fn is_layout(&self) -> bool {
        self.with_element_debug(|_, l| l.is_some())
    }
}

pub fn create_workspace_edit(
    uri: Url,
    version: SourceFileVersion,
    edits: Vec<TextEdit>,
) -> WorkspaceEdit {
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
    KnownComponents { url: Option<VersionedUrl>, components: Vec<ComponentInformation> },
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
