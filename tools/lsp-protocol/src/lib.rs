// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use i_slint_compiler::diagnostics::ByteFormat;
use lsp_types::Url;

#[cfg(target_family = "wasm")]
pub mod wasm_prelude;
#[cfg(target_family = "wasm")]
use wasm_prelude::UrlWasm;

pub type SourceFileVersion = Option<i32>;
pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

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

#[derive(Default, Clone, PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct PreviewConfig {
    pub hide_ui: Option<bool>,
    pub style: String,
    pub include_paths: Vec<PathBuf>,
    pub library_paths: HashMap<String, PathBuf>,
    pub format_utf8: bool,
    pub enable_experimental: bool,
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
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum LspToPreviewMessage {
    InvalidateContents { url: Url },
    ForgetFile { url: Url },
    SetContents { url: VersionedUrl, contents: Vec<u8> },
    SetConfiguration { config: PreviewConfig },
    ShowPreview(PreviewComponent),
    HighlightFromEditor { url: Option<Url>, offset: u32 },
    Quit,
}

impl lsp_types::notification::Notification for LspToPreviewMessage {
    type Params = Self;
    const METHOD: &'static str = "slint/lsp_to_preview";
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewTarget {
    #[allow(dead_code)]
    ChildProcess,
    #[allow(dead_code)]
    EmbeddedWasm,
    #[allow(dead_code)]
    Remote,
    #[allow(dead_code)]
    Dummy,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PreviewToLspMessage {
    /// Report diagnostics to editor.
    Diagnostics { uri: Url, version: SourceFileVersion, diagnostics: Vec<lsp_types::Diagnostic> },
    /// Show a document in the editor.
    ShowDocument { file: Url, selection: lsp_types::Range, take_focus: bool },
    /// Switch between native, WASM, and remote preview (if supported)
    PreviewTypeChanged { target: PreviewTarget },
    /// Request all documents and configuration to be sent from the LSP to the
    /// Preview.
    RequestState { unused: bool },
    /// Pass a `WorkspaceEdit` on to the editor
    SendWorkspaceEdit { label: Option<String>, edit: lsp_types::WorkspaceEdit },
    /// Pass a `ShowMessage` notification on to the editor
    SendShowMessage { message: lsp_types::ShowMessageParams },
    /// Send a telemetry event
    TelemetryEvent(serde_json::Map<String, serde_json::Value>),
}

pub fn uri_to_file(uri: &Url) -> Option<PathBuf> {
    if ["builtin", "vscode-remote"].contains(&uri.scheme()) {
        Some(PathBuf::from(uri.to_string()))
    } else {
        let path = uri.to_file_path().ok()?;
        let cleaned_path = i_slint_compiler::pathutils::clean_path(&path);
        Some(cleaned_path)
    }
}

pub fn file_to_uri(path: &Path) -> Option<Url> {
    if ["builtin:/", "vscode-remote:/"].iter().any(|prefix| path.starts_with(prefix)) {
        Url::parse(path.to_str()?).ok()
    } else {
        Url::from_file_path(path).ok()
    }
}
