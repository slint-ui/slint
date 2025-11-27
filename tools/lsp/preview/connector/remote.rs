use std::{
    cell::RefCell,
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
    str::FromStr,
    sync::{Arc, Mutex},
};

use futures_util::stream::StreamExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::common;

struct RemoteLspToPreviewInner {
    communication_handle: std::thread::JoinHandle<std::result::Result<(), String>>,
    to_child: std::process::ChildStdin,
    child: Arc<Mutex<std::process::Child>>,
}

pub struct RemoteLspToPreview {
    inner: RefCell<Option<RemoteLspToPreviewInner>>,
    preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
}

impl RemoteLspToPreview {
    pub fn new(
        preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
        _server_notifier: crate::ServerNotifier,
    ) -> Self {
        Self { inner: RefCell::new(None), preview_to_lsp_channel }
    }
}

impl common::LspToPreview for RemoteLspToPreview {
    fn send(&self, message: &common::LspToPreviewMessage) {
        todo!()
    }

    fn set_preview_target(&self, target: common::PreviewTarget) -> common::Result<()> {
        todo!()
    }

    fn preview_target(&self) -> common::PreviewTarget {
        todo!()
    }
}

pub struct WebSocketControlledPreviewToLsp {}

impl Default for WebSocketControlledPreviewToLsp {
    fn default() -> Self {
        Self {}
    }
}

impl common::PreviewToLsp for WebSocketControlledPreviewToLsp {
    fn send(&self, message: &common::PreviewToLspMessage) -> common::Result<()> {
        todo!()
    }
}

fn wrap_err(err: impl std::error::Error + Send + Sync + 'static) -> slint::PlatformError {
    slint::PlatformError::OtherError(Box::new(err))
}

pub fn serve(args: &crate::Serve) -> Result<(), slint::PlatformError> {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        eprintln!("Hello world");
        let listen_address = args
            .address
            .as_deref()
            .map(SocketAddr::from_str)
            .unwrap_or(
                const { Ok(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0))) },
            )
            .map_err(wrap_err)?;
        let listener = tokio::net::TcpListener::bind(listen_address).await.map_err(wrap_err)?;
        let local_addr = listener.local_addr().map_err(wrap_err)?;
        eprintln!("Listening on {}", local_addr);

        let mdns = if args.announce {
            let mdns = ServiceDaemon::new().map_err(wrap_err)?;

            let service = ServiceInfo::new(
                "_slint-preview._tcp.local.",
                "lsp",
                hostname::get().map_err(wrap_err)?.to_str().unwrap_or_default(),
                local_addr.ip(),
                local_addr.port(),
                None,
            )
            .map_err(wrap_err)?
            .enable_addr_auto();

            mdns.register(service).map_err(wrap_err)?;

            Some(mdns)
        } else {
            None
        };

        loop {
            let (stream, addr) = listener.accept().await.map_err(wrap_err)?;
            eprintln!("New connection from {addr}");
            tokio::spawn(async move {
                let ws_stream = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("Error during the websocket handshake occurred");
                eprintln!("WebSocket connection established: {addr}");
                let (write, mut read) = ws_stream.split();
                while let Some(message) = read.next().await {
                    match message {
                        Ok(msg) => eprintln!(
                            "Received a message from {addr}: {}",
                            msg.into_text().unwrap_or_default()
                        ),
                        Err(e) => {
                            eprintln!("Error receiving message from {addr}: {e}");
                            break;
                        }
                    }
                }
            });
        }

        if let Some(mdns) = mdns {
            mdns.shutdown().map_err(wrap_err)?;
        }

        Ok(())
    })
}
