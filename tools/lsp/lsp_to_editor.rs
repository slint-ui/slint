// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Messages the language server sends to its editor client.

use i_slint_editor_preview::VersionedDiagnostics;
#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
use i_slint_live_preview::protocol::SourceFileVersion;

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub fn notify_lsp_diagnostics(
    sender: &crate::ServerNotifier,
    uri: lsp_types::Url,
    version: SourceFileVersion,
    diagnostics: Vec<lsp_types::Diagnostic>,
) -> Option<()> {
    sender
        .send_notification::<lsp_types::notification::PublishDiagnostics>(
            lsp_types::PublishDiagnosticsParams { uri, diagnostics, version },
        )
        .ok()
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
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

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub async fn send_show_document_to_editor(
    sender: crate::ServerNotifier,
    file: lsp_types::Url,
    range: lsp_types::Range,
    take_focus: bool,
) {
    let Some(params) = show_document_request_from_element_callback(file, range, take_focus) else {
        return;
    };
    let Ok(fut) = sender.send_request::<lsp_types::request::ShowDocument>(params) else {
        return;
    };

    let _ = fut.await;
}

/// Publish the diagnostics returned by the document lifecycle functions to the LSP client.
///
/// Does nothing unless the `preview-engine` feature is enabled.
#[cfg_attr(not(feature = "preview-engine"), allow(unused_variables))]
pub fn publish_diagnostics(
    server_notifier: &crate::ServerNotifier,
    diagnostics: VersionedDiagnostics,
) {
    #[cfg(feature = "preview-engine")]
    for (uri, version, diagnostics) in diagnostics {
        let _ = notify_lsp_diagnostics(server_notifier, uri, version, diagnostics);
    }
}
