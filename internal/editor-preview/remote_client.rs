// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::time::Duration;

use futures_util::{Sink, SinkExt as _};
use i_slint_live_preview::protocol::PreviewToLspMessage;
use tokio_tungstenite_wasm::Message;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const PING_INTERVAL: Duration = Duration::from_secs(5);
pub const PONG_TIMEOUT: Duration = Duration::from_secs(15);
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

pub fn is_allowed_message(message: &PreviewToLspMessage) -> bool {
    matches!(
        message,
        PreviewToLspMessage::Diagnostics { .. }
            | PreviewToLspMessage::DebugMessage { .. }
            | PreviewToLspMessage::RequestState { .. }
    )
}

pub async fn close(socket: &mut (impl Sink<Message> + Unpin)) {
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, socket.close()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_live_preview::protocol::PreviewDiagnostic;

    /// See #14138.
    #[test]
    fn allowed_messages_survive_postcard() {
        let url = lsp_types::Url::parse("file:///project/main.slint").unwrap();
        for message in [
            PreviewToLspMessage::Diagnostics {
                uri: url.clone(),
                version: Some(3),
                diagnostics: vec![PreviewDiagnostic {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(2, 4),
                        lsp_types::Position::new(2, 9),
                    ),
                    severity: lsp_types::DiagnosticSeverity::ERROR,
                    message: "Unknown property".into(),
                }],
            },
            PreviewToLspMessage::DebugMessage {
                location: Some(("/project/main.slint".into(), 3, 5)),
                message: "debug".into(),
            },
            PreviewToLspMessage::RequestState {
                files: vec![url],
                settings: vec!["settings.json".into()],
            },
        ] {
            assert!(is_allowed_message(&message), "{message:?}");
            let bytes = postcard::to_allocvec(&message).unwrap();
            let decoded: PreviewToLspMessage = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(format!("{decoded:?}"), format!("{message:?}"));
        }
    }
}
