// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::Arc,
};

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
        file_cache: Arc<DashMap<String, CacheEntry>>,
    },
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
    file_cache: Arc<DashMap<String, CacheEntry>>,
}

impl Connection {
    pub async fn listen(
        address: Option<SocketAddr>,
        message_handler: impl Fn(ConnectionMessage) + 'static + Send + Sync,
    ) -> anyhow::Result<Self> {
        let file_cache = Arc::new(DashMap::<String, CacheEntry>::new());
        let (message_sender, mut message_receiver) = sync::mpsc::unbounded_channel();

        let inner_file_cache = file_cache.clone();
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
                let mut current_sink = None;
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
                                            let (sink, receiver) = stream.split();
                                            tokio::spawn(Self::handle_connection(
                                                receiver,
                                                message_handler.clone(),
                                                inner_file_cache.clone(),
                                                inner_message_sender.clone(),
                                                addr,
                                            ));
                                            if let Some(_old_sink) = current_sink.replace(sink) {
                                                tracing::error!(
                                                    "Second connection while we were already connected, dropping old connection"
                                                );
                                            }
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
                            if let (Some(message), Some(current_sink)) = (message, &mut current_sink)
                                && let Err(err) = current_sink.send(message).await {
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
        })
    }

    async fn handle_connection(
        mut receiver: SplitStream<WebSocketStream<TcpStream>>,
        message_handler: Arc<dyn Fn(ConnectionMessage) + 'static + Send + Sync>,
        file_cache: Arc<DashMap<String, CacheEntry>>,
        message_sender: UnboundedSender<Message>,
        remote_addr: SocketAddr,
    ) {
        message_handler(ConnectionMessage::Connected { remote_addr });
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle incoming text messages
                    tracing::warn!("Received text message: {text}");
                }
                Ok(Message::Binary(bin)) => {
                    // Handle incoming binary messages
                    match postcard::from_bytes::<LspToPreviewMessage>(&bin) {
                        Ok(message) => {
                            tracing::debug!("Received message {message:?}");
                            // Process the data
                            match message {
                                LspToPreviewMessage::InvalidateContents { url } => {
                                    file_cache.remove(
                                        url.to_file_path().unwrap().as_os_str().to_str().unwrap(),
                                    );
                                }
                                LspToPreviewMessage::ForgetFile { url } => {
                                    if let Some((_, CacheEntry::Loading(senders))) = file_cache
                                        .remove(
                                            url.to_file_path()
                                                .unwrap()
                                                .as_os_str()
                                                .to_str()
                                                .unwrap(),
                                        )
                                    {
                                        for sender in senders {
                                            let _ = sender.send(Err(std::io::Error::new(
                                                std::io::ErrorKind::NotFound,
                                                "File not found",
                                            )));
                                        }
                                    }
                                }
                                LspToPreviewMessage::SetContents { url, contents } => {
                                    tracing::debug!(
                                        "Inserting file {} with {} bytes.",
                                        url.url(),
                                        contents.len()
                                    );
                                    let versioned_content = VersionedFileContent {
                                        version: *url.version(),
                                        contents: contents.into(),
                                    };
                                    file_cache
                                        .entry(
                                            url.url()
                                                .to_file_path()
                                                .unwrap()
                                                .to_str()
                                                .unwrap()
                                                .to_owned(),
                                        )
                                        .and_modify(|entry| {
                                            if let CacheEntry::Loading(senders) = entry {
                                                for sender in senders.drain(..) {
                                                    let _ =
                                                        sender.send(Ok(versioned_content.clone()));
                                                }
                                            }
                                        })
                                        .insert(CacheEntry::Ready(versioned_content));
                                }
                                LspToPreviewMessage::SetConfiguration { config } => {
                                    message_handler(ConnectionMessage::SetConfiguration { config });
                                }
                                LspToPreviewMessage::ShowPreview(preview_component) => {
                                    message_handler(ConnectionMessage::ShowPreview {
                                        preview_component,
                                        file_cache: file_cache.clone(),
                                    });
                                }
                                LspToPreviewMessage::HighlightFromEditor { url, offset } => {
                                    message_handler(ConnectionMessage::HighlightFromEditor {
                                        url,
                                        offset,
                                    });
                                }
                                LspToPreviewMessage::Quit => {
                                    break;
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
                    // Handle connection close
                    break;
                }
                Ok(Message::Frame(_)) => unreachable!(),
                Err(err) => {
                    tracing::error!("WebSocket error: {err}");
                    break;
                }
            }
        }
        message_handler(ConnectionMessage::Disconnected { remote_addr });
    }

    pub fn send(&self, data: impl Serialize) -> anyhow::Result<()> {
        let data: Vec<u8> = postcard::to_allocvec(&data)?;
        self.message_sender.send(Message::Binary(data.into()))?;

        Ok(())
    }

    pub async fn request_file(&self, file: String) -> std::io::Result<VersionedFileContent> {
        if let Some(entry) = self.file_cache.get(&file)
            && let CacheEntry::Ready(entry) = entry.value()
        {
            return Ok(entry.clone());
        }
        let (sender, receiver) = oneshot::channel();
        let request_file; // do not hold the lock on requested_files across await
        match self.file_cache.entry(file.clone()) {
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
        if request_file {
            self.send(PreviewToLspMessage::RequestState {
                files: vec![Url::from_file_path(&file).map_err(|()| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Invalid file path: {file}"),
                    )
                })?],
            })
            .map_err(std::io::Error::other)?;
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
        let mdns_host = if host == "localhost" || host.is_empty() {
            let ip = local_ips.first().map(|ip| ip.to_string()).unwrap_or("unknown".into());
            format!("slint-viewer-{ip}.local.")
        } else {
            format!("{host}.local.")
        };
        tracing::info!("Announcing service on {local_ips:?} as {mdns_host}");
        let properties = HashMap::from([
            (TXT_PROTOCOLS_KEY.to_owned(), PROTOCOL_SUBPROTOCOL.to_owned()),
            (TXT_SLINT_VERSION_KEY.to_owned(), SLINT_VERSION.to_owned()),
        ]);
        ServiceInfo::new(
            crate::protocol::SERVICE_TYPE,
            "viewer",
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
