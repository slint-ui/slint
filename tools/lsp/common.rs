// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Data structures common between LSP and previewer

use i_slint_compiler::{
    diagnostics::{SourceFile, SourceFileVersion},
    object_tree::Element,
    parser::{syntax_nodes, SyntaxKind},
};
use lsp_types::{TextEdit, Url, WorkspaceEdit};

use std::{collections::HashMap, path::PathBuf};

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;
pub type UrlVersion = Option<i32>;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

/// Use this in nodes you want the language server and preview to
/// ignore a node for code analysis purposes.
pub const NODE_IGNORE_COMMENT: &str = "@lsp:ignore-node";

/// Filter nodes that are marked up to be ignored from the list of nodes.
pub fn filter_ignore_nodes_in_element(
    element: &Element,
) -> impl Iterator<Item = &syntax_nodes::Element> {
    element.node.iter().filter(move |e| {
        !e.children_with_tokens().any(|nt| {
            nt.as_token()
                .map(|t| t.kind() == SyntaxKind::Comment && t.text().contains(NODE_IGNORE_COMMENT))
                .unwrap_or(false)
        })
    })
}

pub fn create_workspace_edit(
    uri: Url,
    version: SourceFileVersion,
    edits: Vec<TextEdit>,
) -> WorkspaceEdit {
    let edits = edits
        .into_iter()
        .map(|te| lsp_types::OneOf::Left::<TextEdit, lsp_types::AnnotatedTextEdit>(te))
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
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct VersionedUrl {
    /// The file url
    pub url: Url,
    // The file version
    pub version: UrlVersion,
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
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PreviewToLspMessage {
    Status { message: String, health: crate::lsp_ext::Health },
    Diagnostics { uri: Url, diagnostics: Vec<lsp_types::Diagnostic> },
    ShowDocument { file: Url, selection: lsp_types::Range },
    PreviewTypeChanged { is_external: bool },
    RequestState { unused: bool }, // send all documents!
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
    /// The URL to the file containing this type
    pub defined_at: Option<Position>,
}

impl ComponentInformation {
    pub fn import_file_name(&self, current_uri: &lsp_types::Url) -> Option<String> {
        if self.is_std_widget {
            Some("std-widgets.slint".to_string())
        } else {
            let url = self.defined_at.as_ref().map(|p| &p.url)?;
            lsp_types::Url::make_relative(current_uri, url)
        }
    }
}
