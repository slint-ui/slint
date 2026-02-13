use std::{sync::Arc, thread::JoinHandle};

use futures_util::{
    SinkExt as _,
    lock::Mutex,
    stream::{SplitSink, SplitStream, StreamExt as _},
};
use tokio_tungstenite_wasm::{Message, WebSocketStream};

struct RemoteLspConnection {
    sender: SplitSink<WebSocketStream, Message>,
    task: slint::JoinHandle<()>,
}

pub struct RemoteLspToPreview {
    #[cfg(not(target_arch = "wasm32"))]
    browse_task: Option<JoinHandle<()>>,
    #[cfg(not(target_arch = "wasm32"))]
    mdns: Option<mdns_sd::ServiceDaemon>,
    preview_to_lsp_channel: crossbeam_channel::Sender<lsp_protocol::PreviewToLspMessage>,
    connection: Arc<Mutex<Option<RemoteLspConnection>>>,
}

impl RemoteLspToPreview {
    pub fn new(
        preview_to_lsp_channel: crossbeam_channel::Sender<lsp_protocol::PreviewToLspMessage>,
        server_notifier: crate::ServerNotifier,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let mdns = mdns_sd::ServiceDaemon::new()
            .inspect_err(|err| tracing::error!("Failed creating MDNS service daemon: {err}"))
            .ok();

        Self {
            #[cfg(not(target_arch = "wasm32"))]
            browse_task: mdns.as_ref().and_then(|mdns| Self::browse_task(mdns, server_notifier)),
            #[cfg(not(target_arch = "wasm32"))]
            mdns,
            preview_to_lsp_channel,
            connection: Arc::default(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn browse_task(
        mdns: &mdns_sd::ServiceDaemon,
        server_notifier: crate::ServerNotifier,
    ) -> Option<JoinHandle<()>> {
        let receiver = mdns
            .browse(lsp_protocol::SERVICE_TYPE)
            .inspect_err(|err| tracing::error!("Failed to start mDNS browsing: {err}"))
            .ok()?;

        let server_notifier = server_notifier.clone();
        Some(std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    mdns_sd::ServiceEvent::SearchStarted(_) => {
                        tracing::debug!("mDNS browsing started");
                    }
                    mdns_sd::ServiceEvent::ServiceFound(_, fullname) => {
                        tracing::debug!("mDNS service found: {fullname}");
                    }
                    mdns_sd::ServiceEvent::ServiceResolved(resolved_service) => {
                        tracing::debug!("mDNS service resolved: {resolved_service:?}");
                        if let Err(err) = server_notifier
                            .send_notification::<RemoteViewerDiscoveredMessage>(
                                RemoteViewerDiscoveredMessage {
                                    host: resolved_service.host,
                                    port: resolved_service.port,
                                    addresses: resolved_service
                                        .addresses
                                        .into_iter()
                                        .map(|addr| addr.to_string())
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

    pub async fn connect(&self, host: String, port: u16) -> crate::common::Result<()> {
        tracing::info!("Attempting to connect to remote preview server at {host}:{port}");
        // The host parameter is not sanitized here, but since it's provided by the user, it should be fine.
        let stream = tokio_tungstenite_wasm::connect(format!("ws://{host}:{port}")).await?;
        tracing::info!("Connected to remote preview server at {host}:{port}");

        let (socket_sender, socket_receiver) = stream.split();

        let Some(old) = self.connection.lock().await.replace(RemoteLspConnection {
            sender: socket_sender,
            task: slint::spawn_local(Self::receive_task(
                socket_receiver,
                self.preview_to_lsp_channel.clone(),
            ))?,
        }) else {
            return Ok(());
        };

        tracing::info!("Closing previous connection to remote preview server");
        old.task.abort();

        Ok(())
    }

    async fn receive_task(
        mut socket_receiver: SplitStream<WebSocketStream>,
        preview_to_lsp_channel: crossbeam_channel::Sender<lsp_protocol::PreviewToLspMessage>,
    ) {
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
                            match postcard::from_bytes::<lsp_protocol::PreviewToLspMessage>(&bytes)
                            {
                                Ok(msg) => {
                                    if let Err(e) = preview_to_lsp_channel.send(msg) {
                                        tracing::error!(
                                            "Error sending message from remote preview server to LSP server: {e}"
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed decoding message from remote preview server: {e}"
                                    );
                                }
                            }
                        }
                        Message::Close(_) => {
                            tracing::info!("WebSocket connection closed by remote server");
                            return;
                        }
                    }
                }
                Err(tokio_tungstenite_wasm::Error::ConnectionClosed)
                | Err(tokio_tungstenite_wasm::Error::AlreadyClosed) => {
                    tracing::info!("WebSocket connection closed by remote server");
                    return;
                }
                Err(err) => {
                    tracing::error!("WebSocket error: {err}");
                }
            }
        }
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
            if let Some(join_handle) = self.browse_task.take()
                && let Err(err) = join_handle.join()
            {
                tracing::error!("Failed joining mDNS thread: {err:?}");
            }
        }
        if let Some(connection) = self.connection.try_lock().unwrap().take() {
            tracing::info!("Closing connection to remote preview server");
            connection.task.abort();
        }
    }
}

impl crate::common::LspToPreview for RemoteLspToPreview {
    fn send(&self, message: &lsp_protocol::LspToPreviewMessage) {
        tracing::debug!("Sending websocket message {message:?}");
        let connection = Arc::downgrade(&self.connection);
        let message = postcard::to_allocvec(message).unwrap();
        let _ = slint::spawn_local(async move {
            let Some(connection) = connection.upgrade() else {
                tracing::warn!("Not connected to remote preview server, dropping message");
                return;
            };
            let mut connection = connection.lock().await;
            let Some(connection) = connection.as_mut() else {
                tracing::warn!("Not connected to remote preview server, dropping message");
                return;
            };
            if let Err(err) = connection.sender.send(Message::binary(message)).await {
                tracing::error!("Error sending message to remote preview server: {err}");
            }
        });
    }

    fn preview_target(&self) -> lsp_protocol::PreviewTarget {
        lsp_protocol::PreviewTarget::Remote
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct RemoteViewerDiscoveredMessage {
    pub host: String,
    pub port: u16,
    pub addresses: Vec<String>,
}

impl lsp_types::notification::Notification for RemoteViewerDiscoveredMessage {
    type Params = Self;
    const METHOD: &'static str = "slint/remote_viewer_discovered";
}
