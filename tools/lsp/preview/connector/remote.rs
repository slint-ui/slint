// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! LSP-server side of the remote-preview connection: owns the WebSocket.
//! Discovery and the dialog live in the preview process; see
//! [`crate::preview::remote`].

use std::rc::Weak;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::{
    SinkExt as _,
    lock::Mutex,
    stream::{SplitSink, SplitStream, StreamExt as _},
};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PreviewToLspMessage, RemoteConnectionState,
    SLINT_PROTOCOLS_HEADER, SLINT_VERSION, SLINT_VERSION_HEADER,
};
use tokio::sync::mpsc;
use tokio_tungstenite_wasm::{Message, WebSocketStream};

use crate::common::LspToPreviews;

struct RemoteLspConnection {
    sender: SplitSink<WebSocketStream, Message>,
    task: tokio::task::JoinHandle<()>,
    /// Set when this connection is being replaced, so the old handle's
    /// `Drop` skips its `Disconnected` emission and the UI doesn't see
    /// Disconnected racing Connected for the new peer.
    replaced: Arc<AtomicBool>,
}

pub struct RemoteLspToPreview {
    connection: Arc<Mutex<Option<RemoteLspConnection>>>,
    preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
    /// Back-reference to the owning [`LspToPreviews`]. Used to forward
    /// `RemoteConnectionState` updates to the dialog. `Weak` so it can
    /// be stored inside the owner without forming an `Rc` cycle.
    to_previews: Weak<LspToPreviews>,
}

impl RemoteLspToPreview {
    pub fn new(
        preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
        to_previews: Weak<LspToPreviews>,
    ) -> Self {
        Self { connection: Arc::default(), preview_to_lsp_sender, to_previews }
    }

    /// Forward a connection-state transition to the local preview dialog.
    fn emit_state(
        to_previews: &Weak<LspToPreviews>,
        state: RemoteConnectionState,
        target: String,
        error: Option<String>,
    ) {
        if let Some(to_previews) = to_previews.upgrade() {
            to_previews.send_to_local_preview(&LspToPreviewMessage::RemoteConnectionState {
                state,
                target,
                error,
            });
        }
    }

    /// Serialize and send a wire-format message. Silently drops if no
    /// peer is connected, and logs (without panicking) on a serialization
    /// failure — this is called from the LSP's hot send path.
    pub fn send(&self, message: &LspToPreviewMessage) {
        tracing::debug!("Sending websocket message {message:?}");
        let connection = Arc::downgrade(&self.connection);
        let message = match postcard::to_allocvec(message) {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::error!("Failed to serialize message for remote preview server: {err}");
                return;
            }
        };
        crate::common::spawn_local(async move {
            let Some(connection) = connection.upgrade() else {
                return;
            };
            let mut connection = connection.lock().await;
            let Some(connection) = connection.as_mut() else {
                return;
            };
            if let Err(err) = connection.sender.send(Message::binary(message)).await {
                tracing::error!("Error sending message to remote preview server: {err}");
            }
        });
    }

    pub fn connect<S: Into<String>>(
        &self,
        addresses: impl IntoIterator<Item = S>,
        port: u16,
    ) -> impl Future<Output = crate::common::Result<()>> + 'static {
        tracing::debug!("RemoteLspToPreview::connect");
        let connection = self.connection.clone();
        let addresses = addresses.into_iter().map(Into::into).collect::<Vec<_>>();
        let preview_to_lsp_sender = self.preview_to_lsp_sender.clone();
        let to_previews = self.to_previews.clone();
        async move {
            // First address identifies the connection in the state notifications.
            let Some(first_address) = addresses.first().cloned() else {
                return Err("No address to connect to".into());
            };
            let target = format!("{first_address}:{port}");
            Self::emit_state(&to_previews, RemoteConnectionState::Connecting, target.clone(), None);

            let addresses = &mut addresses.into_iter();
            let mut last_error: Option<String> = None;
            let (stream, address, port) = loop {
                let Some(address) = addresses.next() else {
                    let reason =
                        last_error.unwrap_or_else(|| "Unable to connect to remote viewer".into());
                    // Don't flip the UI to `Failed` if a live peer is still routing;
                    // that would contradict Connected and disable the Disconnect button.
                    let previous_still_alive = connection.lock().await.is_some();
                    if previous_still_alive {
                        tracing::warn!(
                            "Connect attempt to {target} failed but previous remote connection is still active: {reason}"
                        );
                    } else {
                        Self::emit_state(
                            &to_previews,
                            RemoteConnectionState::Failed,
                            target,
                            Some(reason.clone()),
                        );
                    }
                    return Err(reason.into());
                };
                tracing::info!(
                    "Attempting to connect to remote preview server at {address}:{port}"
                );
                let url = format!("ws://{address}:{port}");
                let connect_future =
                    tokio_tungstenite_wasm::connect_with_protocols(&url, &[PROTOCOL_SUBPROTOCOL]);
                match connect_future.await {
                    Ok(stream) => {
                        tracing::info!("Connected to remote preview server at {address}:{port}");
                        break (stream, address, port);
                    }
                    Err(err) => {
                        let mismatch = describe_version_mismatch(&err);
                        tracing::debug!(
                            "Failed connecting to remote viewer, trying next address: {err}"
                        );
                        if mismatch.is_some() {
                            last_error = mismatch;
                        } else if last_error.is_none() {
                            last_error = Some(format!("{err}"));
                        }
                    }
                }
            };

            let (socket_sender, socket_receiver) = stream.split();
            let replaced = Arc::new(AtomicBool::new(false));
            #[allow(clippy::disallowed_methods)]
            let Some(old) = connection.lock().await.replace(RemoteLspConnection {
                sender: socket_sender,
                task: tokio::task::spawn_local(Self::receive_task(
                    socket_receiver,
                    preview_to_lsp_sender,
                    to_previews.clone(),
                    address,
                    port,
                    replaced.clone(),
                )),
                replaced,
            }) else {
                return Ok(());
            };

            tracing::info!("Closing previous connection to remote preview server");
            old.replaced.store(true, Ordering::Relaxed);
            old.task.abort();

            Ok(())
        }
    }

    async fn receive_task(
        mut socket_receiver: SplitStream<WebSocketStream>,
        preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
        to_previews: Weak<LspToPreviews>,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
    ) {
        let mut connection_state_handle =
            ConnectionStateHandle::new(to_previews, address, port, replaced);
        // TODO: implement a timer to send a ping every once in a while, and close the connection if we don't receive a pong in time
        while let Some(msg) = socket_receiver.next().await {
            match msg {
                Ok(msg) => {
                    tracing::debug!("Received WebSocket message: {msg:?}");
                    match msg {
                        Message::Text(utf8_bytes) => {
                            tracing::warn!(
                                "Received unexpected text message from remote preview server: {utf8_bytes}"
                            );
                        }
                        Message::Binary(bytes) => {
                            match postcard::from_bytes::<PreviewToLspMessage>(&bytes) {
                                Ok(msg) => {
                                    preview_to_lsp_sender.send(msg).unwrap_or_else(|err| {
                                        tracing::error!(
                                            "Failed sending message from remote preview server to LSP server: {err}"
                                        );
                                    });
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed decoding message from remote preview server: {e}"
                                    );
                                }
                            }
                        }
                        Message::Close(_) => {
                            return;
                        }
                    }
                }
                Err(tokio_tungstenite_wasm::Error::ConnectionClosed)
                | Err(tokio_tungstenite_wasm::Error::AlreadyClosed) => {
                    return;
                }
                Err(tokio_tungstenite_wasm::Error::Io(err))
                    if err.kind() != std::io::ErrorKind::WouldBlock =>
                {
                    tracing::error!("I/O error in WebSocket connection: {err}");
                    connection_state_handle.error = Some(format!("I/O error: {err}"));
                    return;
                }
                Err(err) => {
                    tracing::error!("WebSocket error: {err}");
                }
            }
        }
    }

    pub fn disconnect(&self) -> impl Future<Output = ()> + 'static {
        let connection = self.connection.clone();
        async move {
            if let Some(connection) = connection.lock().await.take() {
                connection.task.abort();
            }
        }
    }
}

struct ConnectionStateHandle {
    to_previews: Weak<LspToPreviews>,
    error: Option<String>,
    address: String,
    port: u16,
    replaced: Arc<AtomicBool>,
}

impl ConnectionStateHandle {
    fn new(
        to_previews: Weak<LspToPreviews>,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
    ) -> Self {
        RemoteLspToPreview::emit_state(
            &to_previews,
            RemoteConnectionState::Connected,
            format!("{address}:{port}"),
            None,
        );
        Self { to_previews, error: None, address, port, replaced }
    }
}

impl Drop for ConnectionStateHandle {
    fn drop(&mut self) {
        if self.replaced.load(Ordering::Relaxed) {
            return;
        }
        RemoteLspToPreview::emit_state(
            &self.to_previews,
            RemoteConnectionState::Disconnected,
            format!("{}:{}", self.address, self.port),
            self.error.take(),
        );
    }
}

impl Drop for RemoteLspToPreview {
    fn drop(&mut self) {
        // Best-effort: an in-flight future may hold the lock, in which case
        // LocalSet teardown aborts the receive task. Panicking here would
        // abort the LSP.
        if let Some(mut guard) = self.connection.try_lock()
            && let Some(connection) = guard.take()
        {
            tracing::info!("Closing connection to remote preview server");
            connection.task.abort();
        }
    }
}

/// Human-readable explanation when the handshake was rejected for a Slint
/// version mismatch. The viewer sends `Slint-Version` / `Slint-Protocols`
/// headers; the browser hides them from WASM so we fall back to a generic
/// message there.
fn describe_version_mismatch(err: &tokio_tungstenite_wasm::Error) -> Option<String> {
    match err {
        tokio_tungstenite_wasm::Error::Http(response) => {
            let headers = response.headers();
            let viewer_version = headers
                .get(SLINT_VERSION_HEADER)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("an unknown version");
            let viewer_protocols =
                headers.get(SLINT_PROTOCOLS_HEADER).and_then(|v| v.to_str().ok());
            if headers.contains_key(SLINT_VERSION_HEADER) {
                Some(format!(
                    "Version mismatch: viewer runs Slint {viewer_version} (protocol {}), LSP speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})",
                    viewer_protocols.unwrap_or("unknown"),
                ))
            } else {
                None
            }
        }
        tokio_tungstenite_wasm::Error::Protocol(
            tokio_tungstenite_wasm::error::ProtocolError::SecWebSocketSubProtocolError(_),
        ) => Some(format!(
            "Version mismatch: viewer does not speak {PROTOCOL_SUBPROTOCOL} (this LSP is Slint {SLINT_VERSION})",
        )),
        _ => None,
    }
}
