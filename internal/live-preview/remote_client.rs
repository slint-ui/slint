// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Client transport for connecting a source host to a remote Slint viewer.

use std::fmt;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{
    SinkExt as _,
    stream::{SplitSink, SplitStream, StreamExt as _},
};
use tokio::sync::Mutex as AsyncMutex;
use tokio_tungstenite_wasm::{Message, WebSocketStream};

use crate::protocol::{
    LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PreviewToLspMessage, SLINT_PROTOCOLS_HEADER,
    SLINT_VERSION, SLINT_VERSION_HEADER,
};

/// How often the client probes the remote viewer.
const PING_INTERVAL: Duration = Duration::from_secs(5);
/// Without a response for this long, the connection counts as dead.
const PONG_TIMEOUT: Duration = Duration::from_secs(15);
/// Pause between reconnect attempts.
const RECONNECT_DELAY: Duration = Duration::from_secs(3);
/// Cap on one connection attempt.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// A connection state reported by [`RemotePreviewClient`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteClientState {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

/// A connection lifecycle event independent of any editor or device-manager UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteClientEvent {
    pub state: RemoteClientState,
    /// Human-readable `address:port` of the target.
    pub target: String,
    /// Present when the transition carries a diagnostic.
    pub error: Option<String>,
}

/// Receives lifecycle events from a remote-preview client.
pub trait ConnectionEventSink: Send + Sync {
    fn send(&self, event: RemoteClientEvent);
}

impl<F> ConnectionEventSink for F
where
    F: Fn(RemoteClientEvent) + Send + Sync,
{
    fn send(&self, event: RemoteClientEvent) {
        self(event);
    }
}

/// Receives protocol messages emitted by a remote viewer.
pub trait SourceMessageSink: Send + Sync {
    fn send(&self, message: PreviewToLspMessage);
}

impl<F> SourceMessageSink for F
where
    F: Fn(PreviewToLspMessage) + Send + Sync,
{
    fn send(&self, message: PreviewToLspMessage) {
        self(message);
    }
}

/// Failure to establish or maintain a remote-preview connection.
#[derive(Debug)]
pub struct RemoteClientError(String);

impl fmt::Display for RemoteClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RemoteClientError {}

struct RemoteConnection {
    sender: SplitSink<WebSocketStream, Message>,
    task: tokio::task::JoinHandle<()>,
    /// Set while this connection is being replaced so its drop guard does not
    /// race a `Disconnected` event against the new peer's `Connected` event.
    replaced: Arc<AtomicBool>,
}

#[derive(Clone)]
struct SharedState {
    connection: Arc<AsyncMutex<Option<RemoteConnection>>>,
    source_sink: Arc<dyn SourceMessageSink>,
    event_sink: Arc<dyn ConnectionEventSink>,
    /// Bumped on every caller-driven connect or disconnect. Tasks from an
    /// older generation stop reconnecting.
    generation: Arc<AtomicU64>,
}

impl SharedState {
    fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    fn emit(&self, state: RemoteClientState, target: String, error: Option<String>) {
        self.event_sink.send(RemoteClientEvent { state, target, error });
    }

    fn emit_source_message(&self, message: PreviewToLspMessage) {
        // A remote viewer may report compilation results and ask for source
        // state, but it may not drive its host application. New protocol
        // variants remain refused until deliberately added here.
        if !matches!(
            message,
            PreviewToLspMessage::Diagnostics { .. }
                | PreviewToLspMessage::DebugMessage { .. }
                | PreviewToLspMessage::RequestState { .. }
        ) {
            tracing::warn!(
                "Ignoring message that a remote preview viewer may not send: {message:?}"
            );
            return;
        }
        self.source_sink.send(message);
    }
}

/// A reusable WebSocket client for remote Slint viewers.
pub struct RemotePreviewClient {
    shared: SharedState,
}

impl RemotePreviewClient {
    pub fn new(
        source_sink: impl SourceMessageSink + 'static,
        event_sink: impl ConnectionEventSink + 'static,
    ) -> Self {
        Self {
            shared: SharedState {
                connection: Arc::default(),
                source_sink: Arc::new(source_sink),
                event_sink: Arc::new(event_sink),
                generation: Arc::default(),
            },
        }
    }

    /// Serialize and send one host-to-viewer protocol message.
    pub fn send(&self, message: &LspToPreviewMessage) {
        tracing::debug!("Sending remote-preview message {message:?}");
        let connection = Arc::downgrade(&self.shared.connection);
        let message = match postcard::to_allocvec(message) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::error!("Failed to serialize remote-preview message: {error}");
                return;
            }
        };
        tokio::spawn(async move {
            let Some(connection) = connection.upgrade() else {
                return;
            };
            let mut connection = connection.lock().await;
            let Some(connection) = connection.as_mut() else {
                return;
            };
            if let Err(error) = connection.sender.send(Message::binary(message)).await {
                tracing::error!("Failed to send remote-preview message: {error}");
            }
        });
    }

    /// Connect to the first reachable address and retain all addresses for reconnects.
    pub fn connect<S: Into<String>>(
        &self,
        addresses: impl IntoIterator<Item = S>,
        port: u16,
    ) -> impl Future<Output = Result<(), RemoteClientError>> + Send + 'static {
        let shared = self.shared.clone();
        let addresses = addresses.into_iter().map(Into::into).collect::<Vec<_>>();
        async move {
            let Some(first_address) = addresses.first() else {
                return Err(RemoteClientError("No address to connect to".into()));
            };
            let target = format!("{first_address}:{port}");
            let generation = shared.bump_generation();
            shared.emit(RemoteClientState::Connecting, target.clone(), None);
            if let Err(error) = Self::connect_impl(&shared, &addresses, port, generation).await {
                if shared.generation() == generation {
                    if shared.connection.lock().await.is_some() {
                        tracing::warn!(
                            "Connection to {target} failed while a previous remote viewer remains active: {error}"
                        );
                    } else {
                        shared.emit(RemoteClientState::Failed, target, Some(error.to_string()));
                    }
                }
                return Err(error);
            }
            Ok(())
        }
    }

    fn connect_impl<'a>(
        shared: &'a SharedState,
        addresses: &'a [String],
        port: u16,
        generation: u64,
    ) -> Pin<Box<dyn Future<Output = Result<(), RemoteClientError>> + Send + 'a>> {
        Box::pin(async move {
            let mut last_error = None;
            let mut connected = None;
            for address in addresses {
                tracing::info!("Attempting to connect to remote viewer at {address}:{port}");
                let url = format!("ws://{address}:{port}");
                let connect =
                    tokio_tungstenite_wasm::connect_with_protocols(&url, &[PROTOCOL_SUBPROTOCOL]);
                match tokio::time::timeout(CONNECT_TIMEOUT, connect).await {
                    Ok(Ok(stream)) => {
                        tracing::info!("Connected to remote viewer at {address}:{port}");
                        connected = Some((stream, address.clone()));
                        break;
                    }
                    Ok(Err(error)) => {
                        tracing::debug!("Failed connecting through {address}: {error}");
                        let mismatch = describe_version_mismatch(&error);
                        if mismatch.is_some() {
                            last_error = mismatch;
                        } else if last_error.is_none() {
                            last_error = Some(error.to_string());
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
            let Some((stream, connected_address)) = connected else {
                return Err(RemoteClientError(
                    last_error.unwrap_or_else(|| "Unable to connect to remote viewer".into()),
                ));
            };

            if shared.generation() != generation {
                tracing::info!("Discarding superseded connection to {connected_address}:{port}");
                return Err(RemoteClientError("Connection superseded".into()));
            }

            let (socket_sender, socket_receiver) = stream.split();
            let replaced = Arc::new(AtomicBool::new(false));
            let task = tokio::spawn(Self::run_session(
                shared.clone(),
                socket_receiver,
                addresses.to_vec(),
                connected_address,
                port,
                replaced.clone(),
                generation,
            ));
            if let Some(mut previous) = shared.connection.lock().await.replace(RemoteConnection {
                sender: socket_sender,
                task,
                replaced,
            }) {
                tracing::info!("Closing previous remote-preview connection");
                previous.replaced.store(true, Ordering::Relaxed);
                previous.sender.close().await.ok();
                previous.task.abort();
            }

            shared.emit_source_message(PreviewToLspMessage::RequestState {
                files: Vec::new(),
                settings: Vec::new(),
            });
            Ok(())
        })
    }

    async fn run_session(
        shared: SharedState,
        socket_receiver: SplitStream<WebSocketStream>,
        addresses: Vec<String>,
        connected_address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
        generation: u64,
    ) {
        let last_pong = Arc::new(Mutex::new(Instant::now()));
        let receive = Self::receive_task(
            shared.clone(),
            socket_receiver,
            connected_address,
            port,
            replaced,
            last_pong.clone(),
        );
        let keepalive = Self::keepalive_task(shared.clone(), last_pong);
        tokio::select! {
            _ = receive => {}
            _ = keepalive => {}
        }
        Self::reconnect_loop(&shared, &addresses, port, generation).await;
    }

    async fn keepalive_task(shared: SharedState, last_pong: Arc<Mutex<Instant>>) {
        let Ok(ping) = postcard::to_allocvec(&LspToPreviewMessage::Ping) else {
            return;
        };
        let ping = Message::binary(ping);
        loop {
            tokio::time::sleep(PING_INTERVAL).await;
            let timed_out = last_pong.lock().is_ok_and(|last| last.elapsed() > PONG_TIMEOUT);
            if timed_out {
                tracing::warn!(
                    "Remote viewer answered no ping for {PONG_TIMEOUT:?}; treating the connection as dead"
                );
                return;
            }
            let mut guard = shared.connection.lock().await;
            let Some(connection) = guard.as_mut() else {
                return;
            };
            match tokio::time::timeout(PONG_TIMEOUT, connection.sender.send(ping.clone())).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!("Failed to send keepalive ping: {error}");
                    return;
                }
                Err(_) => {
                    tracing::warn!("Keepalive ping stalled; treating the connection as dead");
                    return;
                }
            }
        }
    }

    async fn reconnect_loop(
        shared: &SharedState,
        addresses: &[String],
        port: u16,
        generation: u64,
    ) {
        if shared.generation() != generation {
            return;
        }
        drop(shared.connection.lock().await.take());
        let target =
            format!("{}:{port}", addresses.first().map(String::as_str).unwrap_or_default());
        tracing::info!("Connection to remote viewer lost; reconnecting to {target}");
        shared.emit(RemoteClientState::Connecting, target.clone(), None);
        loop {
            match Self::connect_impl(shared, addresses, port, generation).await {
                Ok(()) => {
                    tracing::info!("Reconnected to remote viewer at {target}");
                    return;
                }
                Err(error) => {
                    tracing::debug!("Reconnect attempt to {target} failed: {error}");
                }
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
            if shared.generation() != generation {
                return;
            }
        }
    }

    async fn receive_task(
        shared: SharedState,
        mut socket_receiver: SplitStream<WebSocketStream>,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
        last_pong: Arc<Mutex<Instant>>,
    ) {
        let mut connection_state =
            ConnectionStateHandle::new(shared.event_sink.clone(), address, port, replaced);
        while let Some(message) = socket_receiver.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    tracing::warn!("Received unexpected text from remote viewer: {text}");
                }
                Ok(Message::Binary(bytes)) => {
                    match postcard::from_bytes::<PreviewToLspMessage>(&bytes) {
                        Ok(PreviewToLspMessage::Pong) => {
                            if let Ok(mut last_pong) = last_pong.lock() {
                                *last_pong = Instant::now();
                            }
                        }
                        Ok(message) => shared.emit_source_message(message),
                        Err(error) => {
                            tracing::error!("Failed to decode remote-viewer message: {error}");
                        }
                    }
                }
                Ok(Message::Close(_)) => return,
                Err(tokio_tungstenite_wasm::Error::ConnectionClosed)
                | Err(tokio_tungstenite_wasm::Error::AlreadyClosed) => return,
                Err(tokio_tungstenite_wasm::Error::Io(error))
                    if error.kind() != std::io::ErrorKind::WouldBlock =>
                {
                    tracing::error!("Remote-viewer I/O error: {error}");
                    connection_state.error = Some(format!("I/O error: {error}"));
                    return;
                }
                Err(tokio_tungstenite_wasm::Error::Protocol(
                    tokio_tungstenite_wasm::error::ProtocolError::ResetWithoutClosingHandshake,
                )) => {
                    tracing::info!("Connection to remote viewer lost");
                    return;
                }
                Err(error) => tracing::error!("Remote-viewer WebSocket error: {error}"),
            }
        }
    }

    /// Close the current connection and stop any pending reconnect loop.
    pub fn disconnect(&self) -> impl Future<Output = ()> + Send + 'static {
        let shared = self.shared.clone();
        async move {
            shared.bump_generation();
            if let Some(mut connection) = shared.connection.lock().await.take() {
                connection.sender.close().await.ok();
                connection.task.abort();
            }
        }
    }
}

impl Drop for RemotePreviewClient {
    fn drop(&mut self) {
        self.shared.bump_generation();
        if let Ok(mut connection) = self.shared.connection.try_lock()
            && let Some(connection) = connection.take()
        {
            tracing::info!("Closing remote-preview connection");
            connection.task.abort();
        }
    }
}

struct ConnectionStateHandle {
    event_sink: Arc<dyn ConnectionEventSink>,
    error: Option<String>,
    address: String,
    port: u16,
    replaced: Arc<AtomicBool>,
}

impl ConnectionStateHandle {
    fn new(
        event_sink: Arc<dyn ConnectionEventSink>,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
    ) -> Self {
        event_sink.send(RemoteClientEvent {
            state: RemoteClientState::Connected,
            target: format!("{address}:{port}"),
            error: None,
        });
        Self { event_sink, error: None, address, port, replaced }
    }
}

impl Drop for ConnectionStateHandle {
    fn drop(&mut self) {
        if self.replaced.load(Ordering::Relaxed) {
            return;
        }
        self.event_sink.send(RemoteClientEvent {
            state: RemoteClientState::Disconnected,
            target: format!("{}:{}", self.address, self.port),
            error: self.error.take(),
        });
    }
}

fn describe_version_mismatch(error: &tokio_tungstenite_wasm::Error) -> Option<String> {
    match error {
        tokio_tungstenite_wasm::Error::Http(response) => {
            let headers = response.headers();
            let viewer_version = headers
                .get(SLINT_VERSION_HEADER)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("an unknown version");
            let viewer_protocols =
                headers.get(SLINT_PROTOCOLS_HEADER).and_then(|value| value.to_str().ok());
            headers.contains_key(SLINT_VERSION_HEADER).then(|| {
                format!(
                    "Version mismatch: viewer runs Slint {viewer_version} (protocol {}), this client speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})",
                    viewer_protocols.unwrap_or("unknown"),
                )
            })
        }
        tokio_tungstenite_wasm::Error::Protocol(
            tokio_tungstenite_wasm::error::ProtocolError::SecWebSocketSubProtocolError(_),
        ) => Some(format!(
            "Version mismatch: viewer does not speak {PROTOCOL_SUBPROTOCOL} (this client uses Slint {SLINT_VERSION})",
        )),
        _ => None,
    }
}
