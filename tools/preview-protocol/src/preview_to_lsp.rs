use lsp_types::Url;

use crate::SourceFileVersion;

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
    /// Send a telemetry event
    TelemetryEvent(serde_json::Map<String, serde_json::Value>),
}
