// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::Arc,
};

use dashmap::{DashMap, Entry};
use futures_util::{SinkExt as _, StreamExt as _, stream::SplitStream};
use lsp_protocol::{PreviewComponent, PreviewConfig, PreviewToLspMessage, SourceFileVersion};
use lsp_types::Url;
use mdns_sd::ServiceInfo;
use serde::Serialize;
use tokio::{
    net::TcpStream,
    sync::{self, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

#[derive(Clone, Debug)]
pub struct VersionedFileContent {
    pub version: SourceFileVersion,
    pub contents: Arc<[u8]>,
}

#[derive(Debug)]
enum CacheEntry {
    Loading(Vec<oneshot::Sender<std::io::Result<VersionedFileContent>>>),
    Ready(VersionedFileContent),
}

pub enum ConnectionMessage {
    SetConfiguration { config: PreviewConfig },
    ShowPreview { preview_component: PreviewComponent },
    HighlightFromEditor { url: Option<Url>, offset: u32 },
}

pub struct Connection {
    local_addr: SocketAddr,
    task_handle: JoinHandle<()>,
    message_sender: sync::mpsc::UnboundedSender<Message>,
    file_cache: Arc<DashMap<Url, CacheEntry>>,
}

impl Connection {
    pub async fn listen(
        message_handler: impl Fn(ConnectionMessage) + 'static + Send + Sync,
    ) -> anyhow::Result<Self> {
        let listener = tokio::net::TcpListener::bind(SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::UNSPECIFIED,
            0,
            0,
            0,
        )))
        .await?;
        let local_addr = listener.local_addr().map_err(Box::new)?;
        tracing::info!("Listening on {}", local_addr);
        let file_cache = Arc::new(DashMap::<Url, CacheEntry>::new());
        let (message_sender, mut message_receiver) = sync::mpsc::unbounded_channel();

        let inner_file_cache = file_cache.clone();
        let task_handle = tokio::spawn(async move {
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
                                match tokio_tungstenite::accept_async(stream).await {
                                    Err(err) => {
                                        tracing::error!("Failed to establish websocket connection: {err}")
                                    }
                                    Ok(stream) => {
                                        tracing::info!("Connected to {addr:?}");
                                        let (sink, receiver) = stream.split();
                                        tokio::spawn(Self::handle_connection(
                                            receiver,
                                            message_handler.clone(),
                                            inner_file_cache.clone(),
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
                    message = message_receiver.recv() => {
                        if let (Some(message), Some(current_sink)) = (message, &mut current_sink)
                            && let Err(err) = current_sink.send(message).await {
                            tracing::error!("Failed sending message to Websocket: {err}");
                        }
                    }
                }
            }
        });

        Ok(Self { local_addr, task_handle, message_sender, file_cache })
    }

    async fn handle_connection(
        mut receiver: SplitStream<WebSocketStream<TcpStream>>,
        message_handler: Arc<dyn Fn(ConnectionMessage) + 'static + Send + Sync>,
        file_cache: Arc<DashMap<Url, CacheEntry>>,
    ) {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle incoming text messages
                    eprintln!("Received text message: {text}");
                }
                Ok(Message::Binary(bin)) => {
                    // Handle incoming binary messages
                    match postcard::from_bytes::<lsp_protocol::LspToPreviewMessage>(&bin) {
                        Ok(message) => {
                            // Process the data
                            match message {
                                lsp_protocol::LspToPreviewMessage::InvalidateContents { url } => {
                                    file_cache.remove(&url);
                                }
                                lsp_protocol::LspToPreviewMessage::ForgetFile { url } => {
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
                                }
                                lsp_protocol::LspToPreviewMessage::SetContents {
                                    url,
                                    contents,
                                } => {
                                    let versioned_content = VersionedFileContent {
                                        version: *url.version(),
                                        contents: contents.into(),
                                    };
                                    file_cache
                                        .entry(url.url().clone())
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
                                lsp_protocol::LspToPreviewMessage::SetConfiguration { config } => {
                                    message_handler(ConnectionMessage::SetConfiguration { config });
                                }
                                lsp_protocol::LspToPreviewMessage::ShowPreview(
                                    preview_component,
                                ) => {
                                    message_handler(ConnectionMessage::ShowPreview {
                                        preview_component,
                                    });
                                }
                                lsp_protocol::LspToPreviewMessage::HighlightFromEditor {
                                    url,
                                    offset,
                                } => {
                                    message_handler(ConnectionMessage::HighlightFromEditor {
                                        url,
                                        offset,
                                    });
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("Failed to deserialize message: {err}");
                        }
                    }
                }
                Ok(Message::Ping(_data)) => {
                    todo!()
                }
                Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    // Handle connection close
                    break;
                }
                Ok(Message::Frame(_)) => unreachable!(),
                Err(err) => {
                    eprintln!("WebSocket error: {err}");
                    break;
                }
            }
        }
    }

    pub fn send(&self, data: impl Serialize) -> anyhow::Result<()> {
        let data: Vec<u8> = postcard::to_allocvec(&data)?;
        self.message_sender.send(Message::Binary(data.into()))?;

        Ok(())
    }

    pub async fn request_file(&self, file: Url) -> std::io::Result<VersionedFileContent> {
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
            self.send(PreviewToLspMessage::RequestFile { file }).map_err(std::io::Error::other)?;
        }
        receiver.await.map_err(std::io::Error::other)?
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn service(&self) -> anyhow::Result<ServiceInfo> {
        let local_addr = self.local_addr();
        ServiceInfo::new(
            crate::SERVICE_TYPE,
            "viewer",
            hostname::get()?.to_str().unwrap(),
            local_addr.ip(),
            local_addr.port(),
            None,
        )
        .map_err(Into::into)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}
