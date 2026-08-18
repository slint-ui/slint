// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! LSP adapter for the reusable remote-preview client.

use std::pin::Pin;
use std::rc::Weak;

use i_slint_live_preview::protocol::{
    Error, LspToPreviewMessage, PreviewToLspMessage, RemoteConnectionState,
};
use i_slint_live_preview::remote_client::{
    RemoteClientEvent, RemoteClientState, RemotePreviewClient,
};
use tokio::sync::mpsc;

use crate::editor_preview::LspToPreviews;

/// Adapts the shared remote client to the LSP preview fan-out.
pub struct RemoteLspToPreview {
    client: RemotePreviewClient,
}

impl RemoteLspToPreview {
    pub fn new(
        preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
        to_previews: Weak<LspToPreviews>,
    ) -> Self {
        let (connection_event_sender, connection_event_receiver) = mpsc::unbounded_channel();
        crate::editor_preview::spawn_local(forward_connection_events(
            connection_event_receiver,
            to_previews,
        ));

        let source_sink = move |message| {
            preview_to_lsp_sender.send(message).unwrap_or_else(|error| {
                tracing::error!("Failed forwarding a remote-viewer message to the LSP: {error}");
            });
        };
        let event_sink = move |event| {
            connection_event_sender.send(event).unwrap_or_else(|error| {
                tracing::debug!("Remote-preview event receiver has stopped: {error}");
            });
        };
        Self { client: RemotePreviewClient::new(source_sink, event_sink) }
    }

    pub fn send(&self, message: &LspToPreviewMessage) {
        self.client.send(message);
    }

    pub fn connect<S: Into<String>>(
        &self,
        addresses: impl IntoIterator<Item = S>,
        port: u16,
    ) -> impl Future<Output = crate::editor_preview::Result<()>> + 'static {
        let connect = self.client.connect(addresses, port);
        async move { connect.await.map_err(|error| Box::new(error) as Error) }
    }

    pub fn disconnect(&self) -> impl Future<Output = ()> + 'static {
        self.client.disconnect()
    }
}

async fn forward_connection_events(
    mut receiver: mpsc::UnboundedReceiver<RemoteClientEvent>,
    to_previews: Weak<LspToPreviews>,
) {
    while let Some(event) = receiver.recv().await {
        let Some(to_previews) = to_previews.upgrade() else {
            return;
        };
        let state = match event.state {
            RemoteClientState::Disconnected => RemoteConnectionState::Disconnected,
            RemoteClientState::Connecting => RemoteConnectionState::Connecting,
            RemoteClientState::Connected => RemoteConnectionState::Connected,
            RemoteClientState::Failed => RemoteConnectionState::Failed,
        };
        to_previews.send_to_local_preview(&LspToPreviewMessage::RemoteConnectionState {
            state,
            target: event.target,
            error: event.error,
        });
    }
}

// These forwards name the inherent methods explicitly. An unqualified call
// would recurse through the trait if an inherent method were removed.
impl crate::editor_preview::RemoteTransport for RemoteLspToPreview {
    fn send(&self, message: &LspToPreviewMessage) {
        RemoteLspToPreview::send(self, message);
    }

    fn connect(
        &self,
        addresses: Vec<String>,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = crate::editor_preview::Result<()>>>> {
        Box::pin(RemoteLspToPreview::connect(self, addresses, port))
    }

    fn disconnect(&self) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(RemoteLspToPreview::disconnect(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_live_preview::remote::{Connection, ConnectionMessage};
    use std::time::Duration;

    async fn listen(port: u16) -> (Connection, mpsc::UnboundedReceiver<ConnectionMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let connection = Connection::listen(
            Some(std::net::SocketAddr::from(([127, 0, 0, 1], port))),
            None,
            move |msg| {
                let _ = tx.send(msg);
            },
        )
        .await
        .unwrap();
        (connection, rx)
    }

    /// Wait until `rx` yields a message matching `pred`.
    async fn expect_message<T>(
        rx: &mut mpsc::UnboundedReceiver<T>,
        pred: impl Fn(&T) -> bool,
        what: &str,
    ) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let msg = rx.recv().await.expect("message channel closed");
                if pred(&msg) {
                    return;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
    }

    /// A viewer on an arbitrary port with a connector attached to it, past the
    /// handshake and the initial state push both sides start a session with.
    async fn connected_viewer() -> (
        Connection,
        mpsc::UnboundedReceiver<ConnectionMessage>,
        RemoteLspToPreview,
        mpsc::UnboundedReceiver<PreviewToLspMessage>,
    ) {
        let (viewer, mut viewer_rx) = listen(0).await;

        let (to_lsp_tx, mut to_lsp_rx) = mpsc::unbounded_channel();
        let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());
        connector.connect(["127.0.0.1"], viewer.local_port()).await.unwrap();
        expect_message(
            &mut viewer_rx,
            |m| matches!(m, ConnectionMessage::Connected { .. }),
            "viewer connection",
        )
        .await;
        expect_message(
            &mut to_lsp_rx,
            |m| matches!(m, PreviewToLspMessage::RequestState { .. }),
            "RequestState after connecting",
        )
        .await;

        (viewer, viewer_rx, connector, to_lsp_rx)
    }

    #[tokio::test]
    async fn reconnects_after_connection_loss() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, viewer_rx, connector, mut to_lsp_rx) = connected_viewer().await;
                let port = viewer.local_port();

                // Replace the viewer on the same port, like an app whose
                // connection the OS cut while backgrounded.
                drop(viewer);
                drop(viewer_rx);
                let (_viewer, mut viewer_rx) = listen(port).await;

                // The connector reconnects on its own ...
                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::Connected { .. }),
                    "viewer reconnection",
                )
                .await;
                // ... and asks the LSP to re-push the preview state.
                expect_message(
                    &mut to_lsp_rx,
                    |m| matches!(m, PreviewToLspMessage::RequestState { .. }),
                    "RequestState after reconnecting",
                )
                .await;

                connector.disconnect().await;
            })
            .await;
    }

    /// A viewer is on the far end of the network: it may not drive the editor.
    #[tokio::test]
    async fn drops_messages_a_viewer_may_not_send() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, _viewer_rx, connector, mut to_lsp_rx) = connected_viewer().await;

                // A message that drives the editor, followed by one a viewer is
                // allowed to send.
                viewer
                    .send(PreviewToLspMessage::ShowDocument {
                        file: lsp_types::Url::parse("file:///test.slint").unwrap(),
                        selection: lsp_types::Range::default(),
                        take_focus: true,
                    })
                    .unwrap();
                viewer
                    .send(PreviewToLspMessage::Diagnostics {
                        uri: lsp_types::Url::parse("file:///test.slint").unwrap(),
                        version: None,
                        diagnostics: Vec::new(),
                    })
                    .unwrap();

                // The channel keeps the order the viewer sent in, so the
                // diagnostics arriving first means the other one was dropped.
                let message = tokio::time::timeout(Duration::from_secs(15), to_lsp_rx.recv())
                    .await
                    .expect("timed out waiting for the diagnostics")
                    .expect("message channel closed");
                assert!(
                    matches!(message, PreviewToLspMessage::Diagnostics { .. }),
                    "a message the viewer may not send reached the LSP: {message:?}"
                );

                connector.disconnect().await;
            })
            .await;
    }
}
