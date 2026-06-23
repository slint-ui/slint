// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use lsp_types::Url;

use super::SourceFileVersion;

/// Where the local preview is rendered. Remote viewers are layered on top
/// of one of these via [`super::LspToPreviewMessage::RemoteConnectionState`];
/// they aren't a target of their own.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewTarget {
    ChildProcess,
    EmbeddedWasm,
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
    RequestState {
        #[serde(default)]
        files: Vec<Url>,
    },
    /// Pass a `WorkspaceEdit` on to the editor
    SendWorkspaceEdit { label: Option<String>, edit: lsp_types::WorkspaceEdit },
    /// Pass a `ShowMessage` notification on to the editor
    SendShowMessage { message: lsp_types::ShowMessageParams },
    /// Send a telemetry event
    TelemetryEvent(serde_json::Map<String, serde_json::Value>),
    /// A debug message from the preview, to be shown by the LSP
    DebugMessage {
        /// location is the file path, plus the line and column
        location: Option<(std::path::PathBuf, usize, usize)>,
        message: String,
    },
    /// The preview UI asked to connect to a remote viewer. The LSP main
    /// process owns the WebSocket; the addresses are tried in order.
    ConnectRemote { addresses: Vec<String>, port: u16 },
    /// The preview UI asked to disconnect the remote viewer.
    DisconnectRemote,
    /// Answer to [`super::LspToPreviewMessage::Ping`], consumed by the LSP's
    /// WebSocket connector.
    Pong,
}
