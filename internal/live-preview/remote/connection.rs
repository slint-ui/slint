// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    collections::HashSet,
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{Arc, Mutex},
};

use crate::REBUILD_DEBOUNCE;
use crate::protocol::{
    LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PreviewComponent, PreviewConfig,
    PreviewToLspMessage, SLINT_PROTOCOLS_HEADER, SLINT_VERSION, SLINT_VERSION_HEADER,
    SourceFileVersion,
};
#[cfg(not(target_vendor = "apple"))]
use crate::protocol::{TXT_PROTOCOLS_KEY, TXT_SLINT_VERSION_KEY};
use dashmap::{DashMap, Entry};
use futures_util::{SinkExt as _, StreamExt as _, stream::SplitStream};
use lsp_types::Url;
use serde::Serialize;
#[cfg(not(target_vendor = "apple"))]
use std::collections::HashMap;
use tokio::{
    net::TcpStream,
    sync::{self, mpsc::UnboundedSender, oneshot},
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        Message,
        handshake::server::{ErrorResponse, Request, Response},
        http::{HeaderValue, StatusCode, header::SEC_WEBSOCKET_PROTOCOL},
    },
};

#[cfg(not(target_vendor = "apple"))]
use mdns_sd::ServiceInfo;

/// WebSocket handshake callback used on the viewer (server) side.
///
/// Always attaches the `Slint-Version` and `Slint-Protocols` response
/// headers so the LSP can report the viewer's actual version when the
/// handshake is rejected. Accepts the connection only when the client
/// offered our [`PROTOCOL_SUBPROTOCOL`]; otherwise returns 426 Upgrade
/// Required with the same informational headers attached.
#[allow(clippy::result_large_err)] // signature is dictated by tungstenite's Callback trait
fn handshake_callback(
    request: &Request,
    mut response: Response,
) -> Result<Response, ErrorResponse> {
    let headers = response.headers_mut();
    headers.insert(SLINT_VERSION_HEADER, HeaderValue::from_static(SLINT_VERSION));
    headers.insert(SLINT_PROTOCOLS_HEADER, HeaderValue::from_static(PROTOCOL_SUBPROTOCOL));

    let offered: Vec<&str> = request
        .headers()
        .get_all(SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(','))
        .map(str::trim)
        .collect();

    if offered.contains(&PROTOCOL_SUBPROTOCOL) {
        response
            .headers_mut()
            .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static(PROTOCOL_SUBPROTOCOL));
        Ok(response)
    } else {
        tracing::warn!(
            "Rejecting handshake: client offered {offered:?}, we support {PROTOCOL_SUBPROTOCOL:?}"
        );
        let mut err = ErrorResponse::new(None);
        *err.status_mut() = StatusCode::UPGRADE_REQUIRED;
        let err_headers = err.headers_mut();
        err_headers.insert(SLINT_VERSION_HEADER, HeaderValue::from_static(SLINT_VERSION));
        err_headers.insert(SLINT_PROTOCOLS_HEADER, HeaderValue::from_static(PROTOCOL_SUBPROTOCOL));
        Err(err)
    }
}

#[derive(Clone, Debug)]
pub struct VersionedFileContent {
    #[allow(dead_code)]
    pub version: SourceFileVersion,
    pub contents: Arc<[u8]>,
}

#[derive(Debug)]
pub enum CacheEntry {
    Loading(Vec<oneshot::Sender<std::io::Result<VersionedFileContent>>>),
    Ready(VersionedFileContent),
}

/// Shared cache of file contents pushed by the LSP, keyed by the `Url` the LSP sent. Using
/// the URL verbatim avoids platform-dependent path normalization (Windows backslashes,
/// percent-encoding) — equality is structural.
pub type FileCache = Arc<DashMap<Url, CacheEntry>>;

#[derive(Debug)]
pub enum ConnectionMessage {
    Connected {
        remote_addr: SocketAddr,
    },
    Disconnected {
        remote_addr: SocketAddr,
    },
    SetConfiguration {
        config: PreviewConfig,
    },
    ShowPreview {
        preview_component: PreviewComponent,
    },
    /// A dependency of the currently shown component changed. The viewer should rebuild.
    /// The connection has already filtered unrelated edits and debounced bursts of keystrokes.
    ContentsChanged,
    #[allow(dead_code)]
    HighlightFromEditor {
        url: Option<Url>,
        offset: u32,
    },
}

pub struct Connection {
    local_addr: SocketAddr,
    thread_handle: Option<(std::thread::JoinHandle<()>, sync::oneshot::Sender<()>)>,
    message_sender: sync::mpsc::UnboundedSender<Message>,
    file_cache: FileCache,
    /// Files the currently shown component depends on. `SetContents` notifications for URLs
    /// outside this set are ignored, so unrelated edits in the user's editor don't trigger a
    /// rebuild. Updated by the viewer after each compile.
    dependencies: Arc<Mutex<HashSet<Url>>>,
}

/// Whether the connection can act on this URL. The remote preview protocol only handles
/// `file://` URLs; the LSP can legitimately produce others (e.g. `vscode-remote://`), but
/// they're silently ignored on this side.
fn is_supported(url: &Url) -> bool {
    if url.scheme() != "file" {
        tracing::warn!("Ignoring message for unsupported URL scheme: {url}");
        return false;
    }
    true
}

impl Connection {
    pub async fn listen(
        address: Option<SocketAddr>,
        message_handler: impl Fn(ConnectionMessage) + 'static + Send + Sync,
    ) -> anyhow::Result<Self> {
        let file_cache = Arc::new(DashMap::<Url, CacheEntry>::new());
        let dependencies = Arc::new(Mutex::new(HashSet::<Url>::new()));
        let (message_sender, mut message_receiver) = sync::mpsc::unbounded_channel();

        let inner_file_cache = file_cache.clone();
        let inner_dependencies = dependencies.clone();
        let inner_message_sender = message_sender.clone();

        let (local_addr_sender, local_addr_receiver) =
            sync::oneshot::channel::<std::io::Result<SocketAddr>>();
        let (quit_sender, mut quit_receiver) = tokio::sync::oneshot::channel::<()>();

        let thread_handle = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async move {
                let listener = match tokio::net::TcpListener::bind(
                    address.unwrap_or(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0))),
                )
                .await {
                    Ok(listener) => listener,
                    Err(err) => {
                        tracing::error!("Failed to bind to address: {err}");
                        local_addr_sender.send(Err(err)).ok();
                        return;
                    }
                };
                local_addr_sender.send(listener.local_addr()).ok();

                let message_handler = Arc::new(message_handler);
                // The sink is the write half; the JoinHandle is the read-half task. We keep
                // both so the next accept can abort the in-flight task and reset shared state
                // before its messages race with the new client's.
                let mut current_session: Option<(_, tokio::task::JoinHandle<()>)> = None;
                loop {
                    tokio::select! {
                        accept = listener.accept() => {
                            match accept {
                                Err(err) => {
                                    tracing::error!("Failed listening for Websocket connections: {err}");
                                    return;
                                }
                                Ok((stream, addr)) => {
                                    tracing::info!("Connected to {addr:?}");
                                    match tokio_tungstenite::accept_hdr_async(stream, handshake_callback).await {
                                        Err(err) => {
                                            tracing::error!("Failed to establish websocket connection: {err}")
                                        }
                                        Ok(stream) => {
                                            tracing::info!("Websocket established with {addr:?}");
                                            if let Some((_old_sink, old_handle)) = current_session.take() {
                                                tracing::error!(
                                                    "Second connection while we were already connected, dropping old connection"
                                                );
                                                old_handle.abort();
                                                // The aborted task can't run its end-of-loop
                                                // cleanup, so reset the shared state here so
                                                // the new client starts from a clean cache.
                                                inner_file_cache.clear();
                                                inner_dependencies.lock().unwrap().clear();
                                            }
                                            let (sink, receiver) = stream.split();
                                            let handle = tokio::spawn(Self::handle_connection(
                                                receiver,
                                                message_handler.clone(),
                                                inner_file_cache.clone(),
                                                inner_dependencies.clone(),
                                                inner_message_sender.clone(),
                                                addr,
                                            ));
                                            current_session = Some((sink, handle));
                                        }
                                    }
                                }
                            }
                        }
                        _ = &mut quit_receiver => {
                            tracing::info!("Quit signal received, shutting down connection thread.");
                            break;
                        }
                        message = message_receiver.recv() => {
                            if let (Some(message), Some((sink, _))) = (message, current_session.as_mut())
                                && let Err(err) = sink.send(message).await {
                                tracing::error!("Failed sending message to Websocket: {err}");
                            }
                        }
                    }
                }
            });
        });

        let local_addr = local_addr_receiver.await??;
        tracing::info!("Listening on {}", local_addr);

        Ok(Self {
            local_addr,
            thread_handle: Some((thread_handle, quit_sender)),
            message_sender,
            file_cache,
            dependencies,
        })
    }

    /// Replace the set of URLs the connection treats as relevant. A subsequent `SetContents`
    /// for a URL in `urls` produces a `ContentsChanged` message; anything outside is dropped.
    pub fn set_dependencies(&self, urls: Vec<Url>) {
        *self.dependencies.lock().unwrap() = urls.into_iter().collect();
    }

    /// Shared cache of files pushed by the LSP. The viewer reads this to feed
    /// `Compiler::build_from_source`.
    pub fn file_cache(&self) -> FileCache {
        self.file_cache.clone()
    }

    async fn handle_connection(
        mut receiver: SplitStream<WebSocketStream<TcpStream>>,
        message_handler: Arc<dyn Fn(ConnectionMessage) + 'static + Send + Sync>,
        file_cache: FileCache,
        dependencies: Arc<Mutex<HashSet<Url>>>,
        message_sender: UnboundedSender<Message>,
        remote_addr: SocketAddr,
    ) {
        message_handler(ConnectionMessage::Connected { remote_addr });
        // `Some(deadline)` while a `SetContents`-driven rebuild is pending. The sleep_until
        // arm of the select fires `ContentsChanged` once the burst of keystrokes settles.
        let mut debounce_deadline: Option<tokio::time::Instant> = None;
        'outer: loop {
            let debounce_fut = async {
                match debounce_deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                biased;
                _ = debounce_fut => {
                    debounce_deadline = None;
                    message_handler(ConnectionMessage::ContentsChanged);
                }
                msg = receiver.next() => {
                    let Some(msg) = msg else { break };
                    match msg {
                        Ok(Message::Text(text)) => {
                            tracing::warn!("Received text message: {text}");
                        }
                        Ok(Message::Binary(bin)) => {
                            match postcard::from_bytes::<LspToPreviewMessage>(&bin) {
                                Ok(message) => {
                                    tracing::debug!("Received message {message:?}");
                                    match message {
                                        LspToPreviewMessage::InvalidateContents { url } => {
                                            if !is_supported(&url) {
                                                continue;
                                            }
                                            file_cache.remove(&url);
                                            if dependencies.lock().unwrap().contains(&url) {
                                                debounce_deadline = Some(
                                                    tokio::time::Instant::now() + REBUILD_DEBOUNCE,
                                                );
                                            }
                                        }
                                        LspToPreviewMessage::ForgetFile { url } => {
                                            if !is_supported(&url) {
                                                continue;
                                            }
                                            if let Some((_, CacheEntry::Loading(senders))) =
                                                file_cache.remove(&url)
                                            {
                                                for sender in senders {
                                                    let _ = sender.send(Err(std::io::Error::new(
                                                        std::io::ErrorKind::NotFound,
                                                        "File not found",
                                                    )));
                                                }
                                            }
                                            if dependencies.lock().unwrap().contains(&url) {
                                                debounce_deadline = Some(
                                                    tokio::time::Instant::now() + REBUILD_DEBOUNCE,
                                                );
                                            }
                                        }
                                        LspToPreviewMessage::SetContents { url, contents } => {
                                            tracing::debug!(
                                                "Inserting file {} with {} bytes.",
                                                url.url(),
                                                contents.len()
                                            );
                                            if !is_supported(url.url()) {
                                                continue;
                                            }
                                            let versioned_content = VersionedFileContent {
                                                version: *url.version(),
                                                contents: contents.into(),
                                            };
                                            let triggers_rebuild = dependencies
                                                .lock()
                                                .unwrap()
                                                .contains(url.url());
                                            file_cache
                                                .entry(url.url().clone())
                                                .and_modify(|entry| {
                                                    if let CacheEntry::Loading(senders) = entry {
                                                        for sender in senders.drain(..) {
                                                            let _ = sender.send(Ok(
                                                                versioned_content.clone(),
                                                            ));
                                                        }
                                                    }
                                                })
                                                .insert(CacheEntry::Ready(versioned_content));
                                            if triggers_rebuild {
                                                debounce_deadline = Some(
                                                    tokio::time::Instant::now() + REBUILD_DEBOUNCE,
                                                );
                                            }
                                        }
                                        LspToPreviewMessage::SetConfiguration { config } => {
                                            message_handler(ConnectionMessage::SetConfiguration {
                                                config,
                                            });
                                        }
                                        LspToPreviewMessage::ShowPreview(preview_component) => {
                                            // ShowPreview rebuilds unconditionally; cancel any
                                            // queued debounce so the viewer only rebuilds once.
                                            debounce_deadline = None;
                                            message_handler(ConnectionMessage::ShowPreview {
                                                preview_component,
                                            });
                                        }
                                        LspToPreviewMessage::HighlightFromEditor { url, offset } => {
                                            message_handler(ConnectionMessage::HighlightFromEditor {
                                                url,
                                                offset,
                                            });
                                        }
                                        LspToPreviewMessage::Quit => {
                                            break 'outer;
                                        }
                                    }
                                }
                                Err(err) => {
                                    tracing::error!("Failed to deserialize message: {err}");
                                }
                            }
                        }
                        Ok(Message::Ping(data)) => {
                            message_sender.send(Message::Pong(data)).ok();
                        }
                        Ok(Message::Pong(_)) => {}
                        Ok(Message::Close(_)) => {
                            break;
                        }
                        Ok(Message::Frame(_)) => unreachable!(),
                        Err(err) => {
                            tracing::error!("WebSocket error: {err}");
                            break;
                        }
                    }
                }
            }
        }
        // Drop cached contents so a reconnecting peer doesn't see stale buffers from the prior
        // session (the next peer only pushes files currently dirty in its editor and would
        // otherwise inherit our cache for everything else).
        file_cache.clear();
        message_handler(ConnectionMessage::Disconnected { remote_addr });
    }

    pub fn send(&self, data: impl Serialize) -> anyhow::Result<()> {
        let data: Vec<u8> = postcard::to_allocvec(&data)?;
        self.message_sender.send(Message::Binary(data.into()))?;

        Ok(())
    }

    pub async fn request_file(&self, url: Url) -> std::io::Result<VersionedFileContent> {
        if let Some(entry) = self.file_cache.get(&url)
            && let CacheEntry::Ready(entry) = entry.value()
        {
            return Ok(entry.clone());
        }
        let (sender, receiver) = oneshot::channel();
        let request_file; // do not hold the lock across await
        match self.file_cache.entry(url.clone()) {
            Entry::Occupied(mut occupied) => match occupied.get_mut() {
                CacheEntry::Ready(entry) => {
                    return Ok(entry.clone());
                }
                CacheEntry::Loading(senders) => {
                    senders.push(sender);
                    request_file = false;
                }
            },
            Entry::Vacant(vacant) => {
                vacant.insert(CacheEntry::Loading(vec![sender]));
                request_file = true;
            }
        }
        if request_file
            && let Err(err) =
                self.send(PreviewToLspMessage::RequestState { files: vec![url.clone()] })
        {
            // The Loading entry we just inserted will never be resolved by the
            // websocket task — remove it so the senders inside (including ours)
            // drop and a later request_file for the same key doesn't deadlock.
            self.file_cache.remove(&url);
            return Err(std::io::Error::other(err));
        }
        receiver.await.map_err(std::io::Error::other)?
    }

    pub fn local_ips(&self) -> Vec<IpAddr> {
        let unspecified = match self.local_addr {
            SocketAddr::V4(socket_addr_v4) => socket_addr_v4.ip().is_unspecified(),
            SocketAddr::V6(socket_addr_v6) => socket_addr_v6.ip().is_unspecified(),
        };
        if unspecified {
            let mut ips: Vec<IpAddr> =
                getifs::interface_addrs_by_filter(|addr| !addr.is_loopback())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|net| net.addr())
                    .collect();
            if ips.is_empty() {
                // Fallback: open a UDP socket to a public address (nothing is
                // sent) and read back the local IP the OS picked.
                if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0")
                    && sock.connect("8.8.8.8:80").is_ok()
                    && let Ok(addr) = sock.local_addr()
                {
                    ips.push(addr.ip());
                }
            }
            ips
        } else {
            vec![self.local_addr.ip()]
        }
    }
    pub fn local_port(&self) -> u16 {
        self.local_addr.port()
    }

    #[cfg(not(target_vendor = "apple"))]
    pub fn service(&self) -> anyhow::Result<ServiceInfo> {
        let local_ips = self.local_ips();
        let local_port = self.local_port();
        let host = hostname::get()?;
        let host = host.to_str().unwrap_or("unknown");
        // "localhost" is useless for mDNS — derive a name from the first IP instead.
        // The instance name is what the editor shows to the user, so prefer the
        // machine's hostname (these platforms have no separate "device name" the way
        // iOS/macOS do); fall back to an IP-derived name when it's missing.
        let instance_name = if host == "localhost" || host.is_empty() {
            local_ips
                .first()
                .map(|ip| format!("slint-viewer-{ip}"))
                .unwrap_or_else(|| "slint-viewer".into())
        } else {
            host.to_owned()
        };
        let mdns_host = format!("{instance_name}.local.");
        tracing::info!("Announcing service on {local_ips:?} as {mdns_host}");
        let properties = HashMap::from([
            (TXT_PROTOCOLS_KEY.to_owned(), PROTOCOL_SUBPROTOCOL.to_owned()),
            (TXT_SLINT_VERSION_KEY.to_owned(), SLINT_VERSION.to_owned()),
        ]);
        ServiceInfo::new(
            crate::protocol::SERVICE_TYPE,
            &instance_name,
            &mdns_host,
            local_ips.as_slice(),
            local_port,
            Some(properties),
        )
        .map_err(Into::into)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some((thread_handle, quit_sender)) = self.thread_handle.take() {
            quit_sender.send(()).ok();
            thread_handle.join().ok();
        }
    }
}
