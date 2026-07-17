// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore backgrounded

//! LSP-server side of the remote-preview connection: owns the WebSocket.
//! Discovery and the dialog live in the preview process; see
//! [`crate::preview::remote`].

use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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

/// How often the keepalive probes the remote viewer.
const PING_INTERVAL: Duration = Duration::from_secs(5);
/// Without a pong for this long, the connection counts as dead.
/// Mobile devices abort a backgrounded app's connections without notifying
/// the peer, so the socket alone can't tell us.
const PONG_TIMEOUT: Duration = Duration::from_secs(15);
/// Pause between reconnect attempts.
const RECONNECT_DELAY: Duration = Duration::from_secs(3);
/// Cap on a single connection attempt.
/// A device that blocks network for a backgrounded viewer can swallow
/// packets; an uncapped dial would then hang for minutes on TCP retransmissions.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

struct RemoteLspConnection {
    sender: SplitSink<WebSocketStream, Message>,
    task: tokio::task::JoinHandle<()>,
    /// Set when this connection is being replaced, so the old handle's
    /// `Drop` skips its `Disconnected` emission and the UI doesn't see
    /// Disconnected racing Connected for the new peer.
    replaced: Arc<AtomicBool>,
}

/// State shared between the connector and its spawned tasks.
/// Everything runs on the LSP's `LocalSet` thread.
#[derive(Clone)]
struct SharedState {
    connection: Arc<Mutex<Option<RemoteLspConnection>>>,
    preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
    /// Back-reference to the owning [`LspToPreviews`]. Used to forward
    /// `RemoteConnectionState` updates to the dialog. `Weak` so it can
    /// be stored inside the owner without forming an `Rc` cycle.
    to_previews: Weak<LspToPreviews>,
    /// Bumped on every user-driven connect or disconnect.
    /// Tasks spawned for an older generation stand down.
    generation: Rc<Cell<u64>>,
}

impl SharedState {
    fn bump_generation(&self) -> u64 {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        generation
    }

    /// Forward a connection-state transition to the local preview dialog.
    fn emit_state(&self, state: RemoteConnectionState, target: String, error: Option<String>) {
        RemoteLspToPreview::emit_state(&self.to_previews, state, target, error);
    }
}

pub struct RemoteLspToPreview {
    shared: SharedState,
}

impl RemoteLspToPreview {
    pub fn new(
        preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
        to_previews: Weak<LspToPreviews>,
    ) -> Self {
        Self {
            shared: SharedState {
                connection: Arc::default(),
                preview_to_lsp_sender,
                to_previews,
                generation: Rc::default(),
            },
        }
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
        let connection = Arc::downgrade(&self.shared.connection);
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
        let shared = self.shared.clone();
        let addresses = addresses.into_iter().map(Into::into).collect::<Vec<_>>();
        async move {
            // First address identifies the connection in the state notifications.
            let Some(first_address) = addresses.first() else {
                return Err("No address to connect to".into());
            };
            let target = format!("{first_address}:{port}");
            let generation = shared.bump_generation();
            shared.emit_state(RemoteConnectionState::Connecting, target.clone(), None);
            if let Err(reason) = Self::connect_impl(&shared, &addresses, port, generation).await {
                // A superseded attempt no longer owns the dialog state.
                if shared.generation.get() == generation {
                    // Don't flip the UI to `Failed` if a live peer is still routing;
                    // that would contradict Connected and disable the Disconnect button.
                    if shared.connection.lock().await.is_some() {
                        tracing::warn!(
                            "Connect attempt to {target} failed but previous remote connection is still active: {reason}"
                        );
                    } else {
                        shared.emit_state(
                            RemoteConnectionState::Failed,
                            target,
                            Some(reason.to_string()),
                        );
                    }
                }
                return Err(reason);
            }
            Ok(())
        }
    }

    /// Dial `addresses` in order and install the resulting session.
    /// The callers own the dialog state updates.
    async fn connect_impl(
        shared: &SharedState,
        addresses: &[String],
        port: u16,
        generation: u64,
    ) -> crate::common::Result<()> {
        let mut last_error: Option<String> = None;
        let mut connected = None;
        for address in addresses {
            tracing::info!("Attempting to connect to remote preview server at {address}:{port}");
            let url = format!("ws://{address}:{port}");
            let connect_future =
                tokio_tungstenite_wasm::connect_with_protocols(&url, &[PROTOCOL_SUBPROTOCOL]);
            match tokio::time::timeout(CONNECT_TIMEOUT, connect_future).await {
                Ok(Ok(stream)) => {
                    tracing::info!("Connected to remote preview server at {address}:{port}");
                    connected = Some((stream, address.clone()));
                    break;
                }
                Ok(Err(err)) => {
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
                Err(_) => {
                    tracing::debug!("Connection attempt to {address}:{port} timed out");
                    if last_error.is_none() {
                        last_error = Some(format!("Connection attempt to {address} timed out"));
                    }
                }
            }
        }
        let Some((stream, address)) = connected else {
            return Err(last_error
                .unwrap_or_else(|| "Unable to connect to remote viewer".into())
                .into());
        };

        if shared.generation.get() != generation {
            // The user disconnected or connected elsewhere while we dialed.
            tracing::info!("Discarding connection to {address}:{port}: superseded");
            return Err("Connection superseded".into());
        }

        let (socket_sender, socket_receiver) = stream.split();
        let replaced = Arc::new(AtomicBool::new(false));
        #[allow(clippy::disallowed_methods)]
        let task = tokio::task::spawn_local(Self::run_session(
            shared.clone(),
            socket_receiver,
            addresses.to_vec(),
            address,
            port,
            replaced.clone(),
            generation,
        ));
        if let Some(mut old) = shared.connection.lock().await.replace(RemoteLspConnection {
            sender: socket_sender,
            task,
            replaced,
        }) {
            tracing::info!("Closing previous connection to remote preview server");
            old.replaced.store(true, Ordering::Relaxed);
            // Close handshake so the old viewer sees a clean end of session
            // instead of a connection reset.
            old.sender.close().await.ok();
            old.task.abort();
        }

        // Have the LSP push configuration, file contents, and the previewed
        // component, so the viewer leaves its idle screen on its own.
        let _ = shared
            .preview_to_lsp_sender
            // TODO: Settings
            .send(PreviewToLspMessage::RequestState {
                files: Vec::new(),
                settings: Default::default(),
            });

        Ok(())
    }

    /// Drive one established connection.
    /// When the session ends on its own (peer closed, socket error, missing
    /// pongs), reconnect; user-driven disconnects and replacements abort
    /// this task instead.
    async fn run_session(
        shared: SharedState,
        socket_receiver: SplitStream<WebSocketStream>,
        addresses: Vec<String>,
        connected_address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
        generation: u64,
    ) {
        let last_pong = Cell::new(Instant::now());
        let receive = Self::receive_task(
            &shared,
            socket_receiver,
            connected_address,
            port,
            replaced,
            &last_pong,
        );
        let keepalive = Self::keepalive_task(&shared, &last_pong);
        tokio::select! {
            _ = receive => {}
            _ = keepalive => {}
        }
        Self::reconnect_loop(&shared, &addresses, port, generation).await;
    }

    /// Ping the viewer every [`PING_INTERVAL`] and return — ending the
    /// session — when pongs stay out for [`PONG_TIMEOUT`] or sending fails.
    async fn keepalive_task(shared: &SharedState, last_pong: &Cell<Instant>) {
        let Ok(ping) = postcard::to_allocvec(&LspToPreviewMessage::Ping) else { return };
        let ping = Message::binary(ping);
        loop {
            tokio::time::sleep(PING_INTERVAL).await;
            if last_pong.get().elapsed() > PONG_TIMEOUT {
                tracing::warn!(
                    "Remote viewer answered no ping for {PONG_TIMEOUT:?}; treating the connection as dead"
                );
                return;
            }
            let mut guard = shared.connection.lock().await;
            let Some(connection) = guard.as_mut() else { return };
            // Bound the send: it holds the connection lock, which would
            // otherwise block all LSP→viewer sends behind a stalled socket.
            match tokio::time::timeout(PONG_TIMEOUT, connection.sender.send(ping.clone())).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    tracing::warn!("Failed sending keepalive ping to remote viewer: {err}");
                    return;
                }
                Err(_) => {
                    tracing::warn!("Keepalive ping send stalled; treating the connection as dead");
                    return;
                }
            }
        }
    }

    /// Redial a dropped connection every [`RECONNECT_DELAY`] until it
    /// succeeds or a generation bump tells us to stand down.
    async fn reconnect_loop(
        shared: &SharedState,
        addresses: &[String],
        port: u16,
        generation: u64,
    ) {
        if shared.generation.get() != generation {
            return;
        }
        // Drop the dead connection's write half; its task is this very task.
        drop(shared.connection.lock().await.take());
        let target =
            format!("{}:{port}", addresses.first().map(String::as_str).unwrap_or_default());
        tracing::info!("Connection to remote viewer lost; reconnecting to {target}");
        shared.emit_state(RemoteConnectionState::Connecting, target.clone(), None);
        loop {
            match Self::connect_impl(shared, addresses, port, generation).await {
                Ok(()) => {
                    tracing::info!("Reconnected to remote viewer at {target}");
                    return;
                }
                Err(err) => {
                    tracing::debug!("Reconnect attempt to {target} failed: {err}");
                }
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
            if shared.generation.get() != generation {
                return;
            }
        }
    }

    async fn receive_task(
        shared: &SharedState,
        mut socket_receiver: SplitStream<WebSocketStream>,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
        last_pong: &Cell<Instant>,
    ) {
        let mut connection_state_handle =
            ConnectionStateHandle::new(shared.to_previews.clone(), address, port, replaced);
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
                                Ok(PreviewToLspMessage::Pong) => {
                                    last_pong.set(Instant::now());
                                }
                                Ok(msg) => {
                                    shared.preview_to_lsp_sender.send(msg).unwrap_or_else(|err| {
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
                Err(tokio_tungstenite_wasm::Error::Protocol(
                    tokio_tungstenite_wasm::error::ProtocolError::ResetWithoutClosingHandshake,
                )) => {
                    // The viewer vanished without a close handshake (app killed,
                    // network drop) — a normal way for a session to end.
                    tracing::info!("Connection to remote viewer lost");
                    return;
                }
                Err(err) => {
                    tracing::error!("WebSocket error: {err}");
                }
            }
        }
    }

    pub fn disconnect(&self) -> impl Future<Output = ()> + 'static {
        let shared = self.shared.clone();
        async move {
            shared.bump_generation();
            if let Some(mut connection) = shared.connection.lock().await.take() {
                // Close handshake so the viewer sees a clean end of session
                // instead of a connection reset.
                connection.sender.close().await.ok();
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
        // Stop any reconnect loop that is between attempts.
        self.shared.bump_generation();
        // Best-effort: an in-flight future may hold the lock, in which case
        // LocalSet teardown aborts the receive task. Panicking here would
        // abort the LSP.
        if let Some(mut guard) = self.shared.connection.try_lock()
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

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_live_preview::remote::{Connection, ConnectionMessage};

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

    #[tokio::test]
    async fn reconnects_after_connection_loss() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0).await;
                let port = viewer.local_port();

                let (to_lsp_tx, mut to_lsp_rx) = mpsc::unbounded_channel();
                let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());
                connector.connect(["127.0.0.1"], port).await.unwrap();
                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::Connected { .. }),
                    "viewer connection",
                )
                .await;
                // Consume the initial state push, so the wait below can only
                // match the reconnect's.
                expect_message(
                    &mut to_lsp_rx,
                    |m| matches!(m, PreviewToLspMessage::RequestState { .. }),
                    "RequestState after connecting",
                )
                .await;

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
}
