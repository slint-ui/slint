// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::{
    SinkExt as _,
    lock::Mutex,
    stream::{SplitSink, SplitStream, StreamExt as _},
};
use i_slint_live_preview::protocol::{
    self, LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PreviewTarget, PreviewToLspMessage,
    SLINT_PROTOCOLS_HEADER, SLINT_VERSION, SLINT_VERSION_HEADER, TXT_PROTOCOLS_KEY,
    TXT_SLINT_VERSION_KEY,
};
use tokio::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_tungstenite_wasm::{Message, WebSocketStream};

use crate::language::{LspError, LspErrorCode};
use crate::preview::connector::remote::remote_notifications::{
    ConnectionState, RemoteViewerConnectionState,
};

mod remote_notifications;

pub fn connect_remote_preview_command(
    params: &[serde_json::Value],
    ctx: &crate::language::Context,
) -> Result<Option<serde_json::Value>, LspError> {
    let addresses = params.first().and_then(serde_json::Value::as_array).map(|addresses| {
        addresses.iter().filter_map(serde_json::Value::as_str).map(String::from).collect::<Vec<_>>()
    });
    let port = params.get(1).and_then(serde_json::Value::as_u64);

    if let Some(addresses) = addresses {
        if let Some(port) = port {
            ctx.to_preview.with_preview_target::<RemoteLspToPreview, Result<Option<serde_json::Value>, LspError>>(
                |remote| {
                    let preview_to_lsp_sender = ctx.preview_to_lsp_sender.clone();
                    let to_preview = ctx.to_preview.clone();
                    let future = remote.connect(addresses, port as u16);
                    crate::common::spawn_local(async move {
                        // On failure, `connect` already emits a
                        // RemoteViewerConnectionState notification carrying
                        // the version-mismatch reason (or other cause) so the
                        // editor can reset its status bar and show the
                        // explanation. Switch the active preview target only
                        // once we know the socket is up, so a failed connect
                        // does not strand the LSP in Remote mode with no
                        // socket.
                        if future.await.is_ok() {
                            let _ = to_preview.set_preview_target(PreviewTarget::Remote);
                            let _ = preview_to_lsp_sender.send(PreviewToLspMessage::RequestState { files: Vec::new() });
                        }
                    });
                    Ok(None)
                }).unwrap()
        } else {
            Err(LspError {
                code: LspErrorCode::InvalidParameter,
                message: "Need number as the second parameter".to_owned(),
            })
        }
    } else {
        Err(LspError {
            code: LspErrorCode::InvalidParameter,
            message: "Need array of string as the first parameter".to_owned(),
        })
    }
}

pub fn disconnect_remote_preview_command(ctx: &crate::language::Context) {
    let to_preview = ctx.to_preview.clone();
    tracing::debug!("disconnect_remote_preview_command");
    to_preview.with_preview_target::<RemoteLspToPreview, _>(|remote| {
        crate::common::spawn_local(remote.disconnect());
    });
}

struct RemoteLspConnection {
    sender: SplitSink<WebSocketStream, Message>,
    task: tokio::task::JoinHandle<()>,
    /// Shared with the receive_task's `ConnectionStateHandle`. Set to true
    /// before aborting the task when this connection is being replaced by
    /// a new one, so the old handle's Drop skips its Disconnected
    /// notification — otherwise the editor would see Disconnected for the
    /// old peer racing with Connected for the new peer.
    replaced: Arc<AtomicBool>,
}

pub struct RemoteLspToPreview {
    #[cfg(not(target_arch = "wasm32"))]
    browse_task: RwLock<Option<JoinHandle<()>>>,
    #[cfg(not(target_arch = "wasm32"))]
    mdns: Option<mdns_sd::ServiceDaemon>,
    connection: Arc<Mutex<Option<RemoteLspConnection>>>,
    server_notifier: crate::ServerNotifier,
    preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
}

impl RemoteLspToPreview {
    pub fn new(
        server_notifier: crate::ServerNotifier,
        preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let mdns = mdns_sd::ServiceDaemon::new()
            .inspect_err(|err| tracing::error!("Failed creating MDNS service daemon: {err}"))
            .ok();

        Self {
            #[cfg(not(target_arch = "wasm32"))]
            browse_task: RwLock::new(None),
            #[cfg(not(target_arch = "wasm32"))]
            mdns,
            connection: Arc::default(),
            server_notifier,
            preview_to_lsp_sender,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn start_browsing(&self) {
        if let Some(mdns) = &self.mdns {
            let server_notifier = self.server_notifier.clone();
            *self.browse_task.write().await = Self::browse_task(mdns, server_notifier);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn browse_task(
        mdns: &mdns_sd::ServiceDaemon,
        server_notifier: crate::ServerNotifier,
    ) -> Option<JoinHandle<()>> {
        let receiver = mdns
            .browse(protocol::SERVICE_TYPE)
            .inspect_err(|err| tracing::error!("Failed to start mDNS browsing: {err}"))
            .ok()?;

        #[allow(clippy::disallowed_methods)]
        Some(tokio::task::spawn_local(async move {
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    mdns_sd::ServiceEvent::SearchStarted(_) => {
                        // tracing::debug!("mDNS browsing started");
                    }
                    mdns_sd::ServiceEvent::ServiceFound(_, fullname) => {
                        tracing::debug!("mDNS service found: {fullname}");
                    }
                    mdns_sd::ServiceEvent::ServiceResolved(resolved_service) => {
                        use crate::preview::connector::remote::remote_notifications::RemoteViewerDiscoveredMessage;

                        tracing::debug!("mDNS service resolved: {resolved_service:?}");
                        let viewer_protocols = resolved_service
                            .txt_properties
                            .get_property_val_str(TXT_PROTOCOLS_KEY)
                            .map(str::to_owned);
                        let viewer_slint_version = resolved_service
                            .txt_properties
                            .get_property_val_str(TXT_SLINT_VERSION_KEY)
                            .map(str::to_owned);
                        if let Err(err) = server_notifier
                            .send_notification::<RemoteViewerDiscoveredMessage>(
                                RemoteViewerDiscoveredMessage {
                                    host: resolved_service.host,
                                    port: resolved_service.port,
                                    addresses: resolved_service
                                        .addresses
                                        .into_iter()
                                        .map(|addr| match addr {
                                            mdns_sd::ScopedIp::V4(scoped_ip_v4) => {
                                                scoped_ip_v4.addr().to_string()
                                            }
                                            mdns_sd::ScopedIp::V6(scoped_ip_v6) => {
                                                format!("[{}]", scoped_ip_v6.addr())
                                            }
                                            _ => unimplemented!(),
                                        })
                                        .collect(),
                                    viewer_protocols,
                                    viewer_slint_version,
                                    lsp_protocol: PROTOCOL_SUBPROTOCOL.to_owned(),
                                    lsp_slint_version: SLINT_VERSION.to_owned(),
                                },
                            )
                        {
                            tracing::error!(
                                "Failed sending remote viewer discovered notification: {err}"
                            );
                        }
                    }
                    mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                        tracing::debug!("mDNS service removed: {fullname}");
                    }
                    mdns_sd::ServiceEvent::SearchStopped(_) => {
                        tracing::debug!("mDNS browsing stopped");
                    }
                    _ => {
                        tracing::warn!("Received unexpected mDNS event: {event:?}");
                    }
                }
            }
        }))
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
        let server_notifier = self.server_notifier.clone();
        async move {
            // First address is used purely to identify the connection in the
            // disconnected-with-error notification we send on failure.
            let first_address = addresses.first().cloned().unwrap_or_default();
            let addresses = &mut addresses.into_iter();
            let mut last_error: Option<String> = None;
            let (stream, address, port) = loop {
                let Some(address) = addresses.next() else {
                    let reason =
                        last_error.unwrap_or_else(|| "Unable to connect to remote viewer".into());
                    // If a previous connection is still alive, the LSP is
                    // still routing messages to it — report the failed
                    // attempt without toggling the editor's status bar to
                    // Disconnected (that would mislead the user into
                    // thinking they have no connection).
                    let state = if connection.lock().await.is_some() {
                        ConnectionState::ConnectAttemptFailed
                    } else {
                        ConnectionState::Disconnected
                    };
                    let _ = server_notifier.send_notification::<RemoteViewerConnectionState>(
                        RemoteViewerConnectionState {
                            address: first_address.clone(),
                            port,
                            state,
                            error: Some(reason.clone()),
                        },
                    );
                    return Err(reason.into());
                };
                tracing::info!(
                    "Attempting to connect to remote preview server at {address}:{port}"
                );
                // The host parameter is not sanitized here, but since it's provided by the user, it should be fine.
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
                        // Subprotocol mismatch reports a definitive reason — keep it
                        // even if a later address also fails with a less useful error.
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
                    server_notifier,
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
        server_notifier: crate::ServerNotifier,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
    ) {
        let mut connection_state_handle =
            ConnectionStateHandle::new(server_notifier, address, port, replaced);
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
    server_notifier: crate::ServerNotifier,
    error: Option<String>,
    address: String,
    port: u16,
    replaced: Arc<AtomicBool>,
}

impl ConnectionStateHandle {
    fn new(
        server_notifier: crate::ServerNotifier,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
    ) -> Self {
        let _ = server_notifier.send_notification::<RemoteViewerConnectionState>(
            RemoteViewerConnectionState {
                address: address.clone(),
                port,
                state: ConnectionState::Connected,
                error: None,
            },
        );
        Self { server_notifier, error: None, address, port, replaced }
    }
}

impl Drop for ConnectionStateHandle {
    fn drop(&mut self) {
        if self.replaced.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.server_notifier.send_notification::<RemoteViewerConnectionState>(
            RemoteViewerConnectionState {
                address: self.address.clone(),
                port: self.port,
                state: ConnectionState::Disconnected,
                error: self.error.take(),
            },
        );
    }
}

impl Drop for RemoteLspToPreview {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(mdns) = self.mdns.take() {
                let _ = mdns.shutdown().inspect_err(|err| {
                    tracing::error!("Failed shutting down mDNS service daemon: {err}");
                });
            }
            let browse_task = std::mem::take(&mut self.browse_task);
            crate::common::spawn_local(async move {
                if let Some(join_handle) = browse_task.write().await.take()
                    && let Err(err) = join_handle.await
                {
                    tracing::error!("Failed joining mDNS thread: {err:?}");
                }
            });
        }
        if let Some(connection) = self.connection.try_lock().unwrap().take() {
            tracing::info!("Closing connection to remote preview server");
            connection.task.abort();
        }
    }
}

/// Build a human-readable explanation when the WebSocket handshake was
/// rejected because of a Slint version mismatch.
///
/// The viewer always sends `Slint-Version` / `Slint-Protocols` response
/// headers (see `internal/live-preview/remote/connection.rs`), so on a
/// 426 Upgrade Required we can name the viewer's actual version. In wasm
/// builds the browser hides those headers from us and we fall back to a
/// generic mismatch message.
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
            // Only treat as a Slint mismatch if the server identified itself
            // as a Slint viewer (otherwise this is just some random 4xx).
            if headers.contains_key(SLINT_VERSION_HEADER) {
                Some(format!(
                    "Version mismatch: viewer runs Slint {viewer_version} (protocol {}), extension speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})",
                    viewer_protocols.unwrap_or("unknown"),
                ))
            } else {
                None
            }
        }
        tokio_tungstenite_wasm::Error::Protocol(
            tokio_tungstenite_wasm::error::ProtocolError::SecWebSocketSubProtocolError(_),
        ) => Some(format!(
            "Version mismatch: viewer does not speak {PROTOCOL_SUBPROTOCOL} (this extension is Slint {SLINT_VERSION})",
        )),
        _ => None,
    }
}

impl crate::common::LspToPreview for RemoteLspToPreview {
    fn send(&self, message: &LspToPreviewMessage) {
        tracing::debug!("Sending websocket message {message:?}");
        let connection = Arc::downgrade(&self.connection);
        let message = postcard::to_allocvec(message).unwrap();
        crate::common::spawn_local(async move {
            let Some(connection) = connection.upgrade() else {
                tracing::warn!("Not connected to remote preview server, dropping message");
                return;
            };
            let mut connection = connection.lock().await;
            let Some(connection) = connection.as_mut() else {
                tracing::warn!("Not connected to remote preview server, dropping message");
                return;
            };
            let sender_future = connection.sender.send(Message::binary(message));
            if let Err(err) = sender_future.await {
                tracing::error!("Error sending message to remote preview server: {err}");
            }
            tracing::debug!("Succeeded sending websocket message!");
        });
    }

    fn preview_target(&self) -> PreviewTarget {
        PreviewTarget::Remote
    }
}
