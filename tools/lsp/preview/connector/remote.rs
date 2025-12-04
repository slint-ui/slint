use std::{
    cell::RefCell,
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
    str::FromStr,
    sync::Arc,
    thread::JoinHandle,
};

use futures_util::{stream::StreamExt, SinkExt};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::sync;
use tokio_tungstenite::tungstenite::Message;

use crate::common;

#[derive(Default)]
enum ServeTask {
    #[default]
    Stopped,
    Waiting {
        preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
        lsp_to_preview_channel: sync::broadcast::Receiver<common::LspToPreviewMessage>,
    },
    Running {
        join_handle: JoinHandle<()>,
        notify_stop: Arc<sync::Notify>,
    },
}

pub struct RemoteLspToPreview {
    to_show: sync::watch::Receiver<Option<common::PreviewComponent>>,
    sender: sync::broadcast::Sender<common::LspToPreviewMessage>,
    serve_task: RefCell<ServeTask>,
}

impl RemoteLspToPreview {
    pub fn new(
        to_show: sync::watch::Receiver<Option<common::PreviewComponent>>,
        preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
        _server_notifier: crate::ServerNotifier,
    ) -> Self {
        let (sender, lsp_to_preview_channel) = sync::broadcast::channel(128);
        Self {
            to_show,
            sender,
            serve_task: RefCell::new(ServeTask::Waiting {
                preview_to_lsp_channel,
                lsp_to_preview_channel,
            }),
        }
    }

    fn ensure_task(&self) {
        match self.serve_task.take() {
            ServeTask::Waiting { preview_to_lsp_channel, lsp_to_preview_channel } => {
                let notify_stop = Arc::new(sync::Notify::new());
                let inner_notify_stop = notify_stop.clone();
                let to_show = self.to_show.clone();

                self.serve_task.replace(ServeTask::Running {
                    join_handle: std::thread::spawn(move || {
                        if let Err(err) = Self::serve(
                            to_show,
                            preview_to_lsp_channel,
                            lsp_to_preview_channel,
                            inner_notify_stop,
                        ) {
                            eprintln!("WebSocket thread failed: {err}");
                        }
                    }),
                    notify_stop,
                });
            }
            other => {
                self.serve_task.replace(other);
            }
        }
    }

    fn serve(
        to_show: sync::watch::Receiver<Option<common::PreviewComponent>>,
        preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
        lsp_to_preview_channel: sync::broadcast::Receiver<common::LspToPreviewMessage>,
        notify_stop: Arc<sync::Notify>,
    ) -> common::Result<()> {
        let address: Option<String> = None;
        let announce = true;

        tokio::runtime::Builder::new_current_thread().enable_io().build().unwrap().block_on(async {
            eprintln!("Hello world");
            let listen_address = address
                .as_deref()
                .map(SocketAddr::from_str)
                .unwrap_or(
                    const { Ok(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0))) },
                )
                .map_err(Box::new)?;
            let listener = tokio::net::TcpListener::bind(listen_address).await.map_err(Box::new)?;
            let local_addr = listener.local_addr().map_err(Box::new)?;
            eprintln!("Listening on {}", local_addr);

            let mdns = if announce {
                let mdns = ServiceDaemon::new().map_err(Box::new)?;

                let service = ServiceInfo::new(
                    "_slint-preview._tcp.local.",
                    "lsp",
                    &format!(
                        "slint.{}.",
                        hostname::get().map_err(Box::new)?.to_str().unwrap_or_default()
                    ),
                    local_addr.ip(),
                    local_addr.port(),
                    None,
                )
                .map_err(Box::new)?
                .enable_addr_auto();

                mdns.register(service).map_err(Box::new)?;

                Some(mdns)
            } else {
                None
            };

            loop {
                tokio::select! {
                    conn = listener.accept() => {
                        match conn {
                            Ok((stream, addr)) => {
                                eprintln!("New connection from {addr}");
                                tokio::spawn(Self::handle_client(
                                    to_show.clone(),
                                    addr,
                                    stream,
                                    preview_to_lsp_channel.clone(),
                                    lsp_to_preview_channel.resubscribe(),
                                ));
                            }
                            Err(err) => {
                                return Err(Box::new(err) as Box<dyn std::error::Error>);
                            }
                        }
                    }
                    _ = notify_stop.notified() => {
                        if let Some(mdns) = mdns {
                            mdns.shutdown().map_err(Box::new)?;
                        }
                        return Ok(());
                    }
                }
            }
        })
    }

    async fn handle_client(
        to_show: sync::watch::Receiver<Option<common::PreviewComponent>>,
        addr: SocketAddr,
        stream: tokio::net::TcpStream,
        preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
        mut lsp_to_preview_channel: sync::broadcast::Receiver<common::LspToPreviewMessage>,
    ) {
        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .expect("Error during the websocket handshake occurred");
        eprintln!("WebSocket connection established: {addr}");
        let standard_config = bincode::config::standard();
        let (mut write, mut read) = ws_stream.split();

        // Initial setup: send the component to preview
        {
            let to_show = { to_show.borrow().as_ref().cloned() };
            if let Some(to_show) = to_show {
                if let Err(e) = write
                    .send(Message::binary(
                        bincode::serde::encode_to_vec(
                            &common::LspToPreviewMessage::ShowPreview(to_show),
                            standard_config,
                        )
                        .unwrap(),
                    ))
                    .await
                {
                    eprintln!("DISCONNECTING: Failed sending message to {addr}: {e}");
                    return;
                }
            }
        }

        loop {
            tokio::select! {
                message = read.next() => {
                    match message {
                        None => break,
                        Some(Ok(msg)) => {
                            if let tokio_tungstenite::tungstenite::Message::Binary(bytes) = msg {
                                match bincode::serde::decode_from_slice(&bytes, standard_config) {
                                    Ok((msg, _)) => {
                                        if let Err(e) = preview_to_lsp_channel.send(msg) {
                                            eprintln!("Error receiving message from {addr}: {e}");
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed decoding message from {addr}: {e}");
                                    }
                                }
                            } else {
                                eprintln!("Received non-binary message from {addr}: {msg:?}");
                            }
                        }
                        Some(Err(e)) => {
                            eprintln!("Error receiving message from {addr}: {e}");
                            break;
                        }
                    }
                }
                message = lsp_to_preview_channel.recv() => {
                    match message {
                        Ok(msg) => {
                            if let Err(err) = write.send(tokio_tungstenite::tungstenite::Message::binary(
                                bincode::serde::encode_to_vec(&msg, standard_config).unwrap(),
                            )).await {
                                eprintln!("Error sending message to {addr}: {err}");
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    }
}

impl Drop for RemoteLspToPreview {
    fn drop(&mut self) {
        if let ServeTask::Running { join_handle, notify_stop } = self.serve_task.take() {
            notify_stop.notify_waiters();
            let _ = join_handle.join();
        }
    }
}

impl common::LspToPreview for RemoteLspToPreview {
    fn send(&self, message: &common::LspToPreviewMessage) {
        self.ensure_task();
        eprintln!("Sending websocket message {message:?}");
        if let Err(err) = self.sender.send(message.clone()) {
            eprintln!("Failed sending message to WebSocket thread: {err}");
        }
    }

    fn set_preview_target(&self, _target: common::PreviewTarget) -> common::Result<()> {
        Err("Can not change the preview target".into())
    }

    fn preview_target(&self) -> common::PreviewTarget {
        common::PreviewTarget::Remote
    }
}

// pub struct WebSocketControlledPreviewToLsp {}

// impl Default for WebSocketControlledPreviewToLsp {
//     fn default() -> Self {
//         Self {}
//     }
// }

// impl common::PreviewToLsp for WebSocketControlledPreviewToLsp {
//     fn send(&self, message: &common::PreviewToLspMessage) -> common::Result<()> {
//         todo!()
//     }
// }
