// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use lsp_types::Url;

use crate::SourceFileVersion;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// Switch between native and WASM preview (if supported)
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
