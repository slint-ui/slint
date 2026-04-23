use std::sync::Arc;

use futures_util::{
    SinkExt as _,
    lock::Mutex,
    stream::{SplitSink, SplitStream, StreamExt as _},
};
use i_slint_preview_protocol::{PreviewTarget, PreviewToLspMessage};
use tokio::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_tungstenite_wasm::{Message, WebSocketStream};

use crate::preview::connector::remote::remote_notifications::{
    ConnectionState, RemoteViewerConnectionState,
};

mod remote_notifications;

struct RemoteLspConnection {
    sender: SplitSink<WebSocketStream, Message>,
    task: tokio::task::JoinHandle<()>,
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
            .browse(i_slint_preview_protocol::SERVICE_TYPE)
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
            let addresses = &mut addresses.into_iter();
            let (stream, address, port) = loop {
                let Some(address) = addresses.next() else {
                    return Err("Unable to connect to remote viewer".into());
                };
                tracing::info!(
                    "Attempting to connect to remote preview server at {address}:{port}"
                );
                // The host parameter is not sanitized here, but since it's provided by the user, it should be fine.
                let connect_future =
                    tokio_tungstenite_wasm::connect(format!("ws://{address}:{port}"));
                match connect_future.await {
                    Ok(stream) => {
                        tracing::info!("Connected to remote preview server at {address}:{port}");
                        break (stream, address, port);
                    }
                    Err(err) => {
                        tracing::debug!(
                            "Failed connecting to remote viewer, trying next address: {err}"
                        );
                    }
                }
            };

            let (socket_sender, socket_receiver) = stream.split();

            #[allow(clippy::disallowed_methods)]
            let Some(old) = connection.lock().await.replace(RemoteLspConnection {
                sender: socket_sender,
                task: tokio::task::spawn_local(Self::receive_task(
                    socket_receiver,
                    preview_to_lsp_sender,
                    server_notifier,
                    address,
                    port,
                )),
            }) else {
                return Ok(());
            };

            tracing::info!("Closing previous connection to remote preview server");
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
    ) {
        let mut connection_state_handle =
            ConnectionStateHandle::new(server_notifier, address, port);
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
                            match postcard::from_bytes::<
                                i_slint_preview_protocol::PreviewToLspMessage,
                            >(&bytes)
                            {
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
                            connection_state_handle.error =
                                Some("Remote server closed the connection".into());
                            return;
                        }
                    }
                }
                Err(tokio_tungstenite_wasm::Error::ConnectionClosed)
                | Err(tokio_tungstenite_wasm::Error::AlreadyClosed) => {
                    connection_state_handle.error =
                        Some("Remote server closed the connection".into());
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
}

impl ConnectionStateHandle {
    fn new(server_notifier: crate::ServerNotifier, address: String, port: u16) -> Self {
        let _ = server_notifier.send_notification::<RemoteViewerConnectionState>(
            RemoteViewerConnectionState {
                address: address.clone(),
                port,
                state: ConnectionState::Connected,
                error: None,
            },
        );
        Self { server_notifier, error: None, address, port }
    }
}

impl Drop for ConnectionStateHandle {
    fn drop(&mut self) {
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

impl crate::common::LspToPreview for RemoteLspToPreview {
    fn send(&self, message: &i_slint_preview_protocol::LspToPreviewMessage) {
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
