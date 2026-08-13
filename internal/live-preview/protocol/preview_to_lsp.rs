// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

use lsp_types::Url;

use super::{PairingRejection, PreviewComponent, Result, SourceFileVersion};

#[cfg(target_arch = "wasm32")]
use super::wasm_prelude::*;

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
    /// Preview. `settings` names the user-settings files the preview wants
    /// restored; the LSP replies with a
    /// [`super::LspToPreviewMessage::SetUserSettings`] for each one that exists.
    RequestState {
        #[serde(default)]
        files: Vec<Url>,
        #[serde(default)]
        settings: Vec<String>,
    },
    /// Persist a user-settings blob. The LSP writes `contents` verbatim to the
    /// file named `name`; it never interprets the payload.
    UpdateUserSettings { name: String, contents: String },
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
    /// The user typed a pairing code into the preview UI. Carries the code
    /// itself; the LSP runs the SPAKE2 exchange with it.
    SubmitPairingCode { code: String },
    /// The user dismissed the pairing prompt in the preview UI.
    CancelPairing,
    /// The user agreed to connect to a viewer that has pairing disabled,
    /// knowing the session will not be encrypted.
    AcceptUnpairedConnection,
    /// Answer to [`super::LspToPreviewMessage::Ping`], consumed by the LSP's
    /// WebSocket connector.
    Pong,
    /// Ask the LSP to load a component and answer with
    /// [`super::LspToPreviewMessage::ShowPreview`].
    RequestPreview { component: PreviewComponent },
    /// First message on a remote connection: the viewer is ready for
    /// [`super::LspToPreviewMessage::PairingHello`]. See [`super::pairing`].
    PairingReady,
    /// The client offered no usable token, so the viewer is now showing a
    /// pairing code and waiting for the user to type it. `element` starts
    /// the SPAKE2 exchange the code is established through; a fresh one is
    /// sent for every attempt.
    PairingRequired { attempts_left: u8, expires_in_seconds: u16, element: super::pairing::Element },
    /// The client announced a token this viewer issued, so the viewer opens
    /// the reconnect exchange, with the token as the secret and nobody on
    /// the screen. Answered like a code prompt, by
    /// [`super::LspToPreviewMessage::PairingResponse`].
    PairingTokenChallenge { element: super::pairing::Element },
    /// The viewer derived the same key, and proves it. The session is
    /// sealed from the next frame on.
    PairingConfirm { confirmation: super::pairing::Confirmation },
    /// Pairing is disabled on this viewer, so there is nothing to prove and
    /// the session stays plaintext. Nothing else ends with this message: a
    /// code or token exchange ends with [`Self::PairingConfirm`].
    PairingAccepted,
    /// The client is not authenticated. Whether retrying is worthwhile is
    /// [`PairingRejection::is_terminal`].
    PairingRejected { reason: PairingRejection },
}

/// One transport from a preview back to the LSP.
pub trait PreviewToLsp {
    fn send(&self, message: &PreviewToLspMessage) -> Result<()>;

    /// Tell the editor about diagnostics
    fn notify_diagnostics(
        &self,
        diagnostics: HashMap<lsp_types::Url, (SourceFileVersion, Vec<lsp_types::Diagnostic>)>,
    ) -> Result<()> {
        for (uri, (version, diagnostics)) in diagnostics {
            self.send(&PreviewToLspMessage::Diagnostics { uri, version, diagnostics })?;
        }
        Ok(())
    }

    /// Ask the editor to show some document
    fn ask_editor_to_show_document(
        &self,
        file: &str,
        selection: lsp_types::Range,
        take_focus: bool,
    ) -> Result<()> {
        let file = match lsp_types::Url::from_file_path(file) {
            Ok(file) => file,
            Err(()) => {
                tracing::error!("Failed to convert file path to URL for ShowDocument: {file}");
                return Err("Failed to convert file path to URL".to_string().into());
            }
        };
        if selection.start.character == 0 || selection.end.character == 0 {
            return Ok(());
        }
        self.send(&PreviewToLspMessage::ShowDocument { file, selection, take_focus })
    }

    /// Sends a telemetry event
    fn send_telemetry(&self, data: &mut [(String, serde_json::Value)]) -> Result<()> {
        let object = {
            let mut object = serde_json::Map::new();
            for (name, value) in data.iter_mut() {
                object.insert(std::mem::take(name), std::mem::take(value));
            }
            object
        };
        if let Err(err) = self.send(&PreviewToLspMessage::TelemetryEvent(object)) {
            tracing::error!("Failed to send telemetry event: {err}");
            return Err(err);
        }
        Ok(())
    }
}
