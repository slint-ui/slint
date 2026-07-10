// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore alnum localdomain notlocalhost

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
    /// The viewer should register this font with the renderer.
    RegisterFont {
        url: Url,
        contents: Arc<[u8]>,
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
    /// Friendly device name shown to remote clients; also used as the mDNS instance name
    /// on non-Apple platforms. Always non-empty: an IP-derived label is substituted if no
    /// name source resolved. On Apple, the initial value is the system hostname; the
    /// viewer overwrites it with the Bonjour-reported name once the service is registered.
    device_name: Mutex<String>,
}

/// Serialize a message into the wire format and queue it on the write half.
fn encode_and_send(
    sender: &UnboundedSender<Message>,
    message: &impl Serialize,
) -> anyhow::Result<()> {
    let data: Vec<u8> = postcard::to_allocvec(message)?;
    sender.send(Message::Binary(data.into()))?;
    Ok(())
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
        device_name_override: Option<String>,
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
                                                // A finished handle is just a stale session left
                                                // behind by an earlier disconnect, not a takeover.
                                                if !old_handle.is_finished() {
                                                    tracing::warn!(
                                                        "Second connection while we were already connected, dropping old connection"
                                                    );
                                                    old_handle.abort();
                                                }
                                                // An aborted task can't run its end-of-loop
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

        let device_name = {
            let raw =
                device_name_override.filter(|n| !n.is_empty()).unwrap_or_else(default_device_name);
            if raw.is_empty() { ip_derived_device_name(&local_ips_for(local_addr)) } else { raw }
        };
        Ok(Self {
            local_addr,
            thread_handle: Some((thread_handle, quit_sender)),
            message_sender,
            file_cache,
            dependencies,
            device_name: Mutex::new(device_name),
        })
    }

    /// Friendly device name to advertise over mDNS and show in the viewer UI.
    /// Guaranteed non-empty: an IP-derived label is substituted when no user-set source
    /// is available. On Apple the value starts as the system hostname and is overwritten
    /// by [`Self::set_device_name`] once Bonjour reports the registered instance name.
    pub fn device_name(&self) -> String {
        self.device_name.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Replace the friendly device name. Empty or whitespace-only values are ignored so
    /// callers can pass the raw output of an mDNS registration without pre-checking.
    pub fn set_device_name(&self, name: String) {
        if !name.trim().is_empty() {
            *self.device_name.lock().unwrap_or_else(|e| e.into_inner()) = name;
        }
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
                                            // Fonts are registered with the renderer directly
                                            // and not consulted by the compiler, so they don't
                                            // go in the file cache.
                                            if i_slint_compiler::pathutils::is_font_file(
                                                url.url().path(),
                                            ) {
                                                message_handler(ConnectionMessage::RegisterFont {
                                                    url: url.url().clone(),
                                                    contents: contents.into(),
                                                });
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
                                        LspToPreviewMessage::Ping => {
                                            encode_and_send(
                                                &message_sender,
                                                &PreviewToLspMessage::Pong,
                                            )
                                            .ok();
                                        }
                                        // Internal LSP↔local-preview control message;
                                        // never legitimately reaches a remote viewer.
                                        LspToPreviewMessage::RemoteConnectionState { .. } => {
                                            tracing::warn!(
                                                "Ignoring unexpected RemoteConnectionState over WebSocket"
                                            );
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
                        Err(tokio_tungstenite::tungstenite::Error::Protocol(
                            tokio_tungstenite::tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                        )) => {
                            // The peer vanished without a close handshake (process killed,
                            // network drop) — a normal way for a session to end.
                            tracing::info!("Connection lost");
                            break;
                        }
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
        encode_and_send(&self.message_sender, &data)
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
        local_ips_for(self.local_addr)
    }
    pub fn local_port(&self) -> u16 {
        self.local_addr.port()
    }

    #[cfg(not(target_vendor = "apple"))]
    pub fn service(&self) -> anyhow::Result<ServiceInfo> {
        let local_ips = self.local_ips();
        let local_port = self.local_port();
        // The instance name is the user-visible label in editors and can contain spaces,
        // apostrophes, or non-ASCII characters. The SRV target ("host name") is consumed
        // by DNS resolvers and must be limited to LDH characters / RFC 1035 label limits.
        let device_name = self.device_name();
        let mdns_host = format!("{}.local.", sanitize_dns_label(&device_name));
        tracing::info!("Announcing service on {local_ips:?} as {device_name} ({mdns_host})");
        let properties = HashMap::from([
            (TXT_PROTOCOLS_KEY.to_owned(), PROTOCOL_SUBPROTOCOL.to_owned()),
            (TXT_SLINT_VERSION_KEY.to_owned(), SLINT_VERSION.to_owned()),
        ]);
        ServiceInfo::new(
            crate::protocol::SERVICE_TYPE,
            &device_name,
            &mdns_host,
            local_ips.as_slice(),
            local_port,
            Some(properties),
        )
        .map_err(Into::into)
    }
}

fn local_ips_for(local_addr: SocketAddr) -> Vec<IpAddr> {
    let unspecified = match local_addr {
        SocketAddr::V4(socket_addr_v4) => socket_addr_v4.ip().is_unspecified(),
        SocketAddr::V6(socket_addr_v6) => socket_addr_v6.ip().is_unspecified(),
    };
    if unspecified {
        let mut ips: Vec<IpAddr> = getifs::interface_addrs_by_filter(|addr| !addr.is_loopback())
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
        vec![local_addr.ip()]
    }
}

/// Compute the friendly device name to advertise.
///
/// On Linux, prefer the systemd "pretty hostname" the user sets in Settings → About →
/// Device Name (`PRETTY_HOSTNAME=` in `/etc/machine-info`). Everywhere else, fall back to
/// the system hostname. `localhost` and empty strings are treated as missing so the caller
/// can substitute an IP-derived label. On Apple, the viewer overwrites this with the
/// Bonjour-reported friendly name once the service is registered.
fn default_device_name() -> String {
    #[cfg(target_os = "linux")]
    if let Some(pretty) = read_pretty_hostname() {
        let cleaned = non_localhost(pretty);
        if !cleaned.is_empty() {
            return cleaned;
        }
    }
    let host = hostname::get().ok().and_then(|h| h.into_string().ok()).unwrap_or_default();
    non_localhost(host)
}

/// Treat any `localhost` variant as missing — bare `localhost`, the RHEL/CentOS default
/// `localhost.localdomain`, or case variants — so the caller falls through to the
/// IP-derived label rather than advertising a name that conflicts with every other host.
fn non_localhost(name: String) -> String {
    let lower = name.to_ascii_lowercase();
    if lower == "localhost" || lower.starts_with("localhost.") { String::new() } else { name }
}

/// Fallback when no user-set device name is available. Picks a label derived from the
/// first non-loopback local IP so the user can still tell two instances apart.
fn ip_derived_device_name(local_ips: &[IpAddr]) -> String {
    local_ips
        .first()
        .map(|ip| format!("slint-viewer-{ip}"))
        .unwrap_or_else(|| "slint-viewer".into())
}

/// Convert a friendly device name into a DNS label suitable for the SRV target. Replaces
/// non-LDH characters with `-`, collapses repeats, trims leading/trailing dashes, and
/// clamps to the RFC 1035 63-octet label limit. Returns a safe fallback for empty inputs.
#[cfg(not(target_vendor = "apple"))]
fn sanitize_dns_label(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > 63 {
        out.truncate(63);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() { "slint-viewer".to_owned() } else { out }
}

/// Parse the PRETTY_HOSTNAME entry from /etc/machine-info. Handles unquoted values and
/// single- or double-quoted values. Does not implement systemd's full shell-style escape
/// rules — values containing `\"` come through as-is. The systemd env-file format only
/// treats `#` as a comment at the start of a line, so `#` mid-value is preserved.
#[cfg(target_os = "linux")]
fn read_pretty_hostname() -> Option<String> {
    let contents = std::fs::read_to_string("/etc/machine-info").ok()?;
    parse_pretty_hostname(&contents)
}

#[cfg(any(target_os = "linux", test))]
fn parse_pretty_hostname(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let Some(rest) = line.trim_start().strip_prefix("PRETTY_HOSTNAME=") else { continue };
        let rest = rest.trim_start();
        let value = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let after_open = &rest[quote.len_utf8()..];
                // Malformed (unterminated) quote: skip this line, don't abandon the file.
                let Some(end) = after_open.find(quote) else { continue };
                after_open[..end].to_owned()
            }
            _ => rest.trim_end().to_owned(),
        };
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some((thread_handle, quit_sender)) = self.thread_handle.take() {
            quit_sender.send(()).ok();
            thread_handle.join().ok();
        }
    }
}

#[cfg(all(test, not(target_vendor = "apple")))]
mod tests {
    use super::sanitize_dns_label;

    #[test]
    fn sanitize_keeps_alnum() {
        assert_eq!(sanitize_dns_label("MyBox42"), "MyBox42");
    }

    #[test]
    fn sanitize_replaces_spaces_and_quotes() {
        assert_eq!(sanitize_dns_label("Simon's Laptop"), "Simon-s-Laptop");
    }

    #[test]
    fn sanitize_collapses_runs_and_trims_dashes() {
        assert_eq!(sanitize_dns_label("  hello   world  "), "hello-world");
        assert_eq!(sanitize_dns_label("---abc---"), "abc");
    }

    #[test]
    fn sanitize_truncates_to_63_octets() {
        let long = "a".repeat(200);
        assert_eq!(sanitize_dns_label(&long).len(), 63);
    }

    #[test]
    fn sanitize_falls_back_for_empty_or_all_invalid() {
        assert_eq!(sanitize_dns_label(""), "slint-viewer");
        assert_eq!(sanitize_dns_label("@@@"), "slint-viewer");
    }
}

#[cfg(test)]
mod keepalive_tests {
    use super::Connection;
    use crate::protocol::{LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PreviewToLspMessage};
    use futures_util::{SinkExt as _, StreamExt as _};
    use tokio_tungstenite::tungstenite::{
        client::IntoClientRequest as _,
        http::{HeaderValue, header::SEC_WEBSOCKET_PROTOCOL},
    };

    #[tokio::test]
    async fn ping_is_answered_with_pong() {
        let connection =
            Connection::listen(Some(std::net::SocketAddr::from(([127, 0, 0, 1], 0))), None, |_| {})
                .await
                .unwrap();

        let mut request =
            format!("ws://127.0.0.1:{}", connection.local_port()).into_client_request().unwrap();
        request
            .headers_mut()
            .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static(PROTOCOL_SUBPROTOCOL));
        let (mut stream, _) = tokio_tungstenite::connect_async(request).await.unwrap();

        let ping = postcard::to_allocvec(&LspToPreviewMessage::Ping).unwrap();
        stream.send(tokio_tungstenite::tungstenite::Message::Binary(ping.into())).await.unwrap();

        let pong = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let msg = stream.next().await.expect("stream ended without a pong").unwrap();
                if let tokio_tungstenite::tungstenite::Message::Binary(bytes) = msg {
                    return postcard::from_bytes::<PreviewToLspMessage>(&bytes).unwrap();
                }
            }
        })
        .await
        .expect("no pong within 5 seconds");
        assert!(matches!(pong, PreviewToLspMessage::Pong));
    }
}

#[cfg(test)]
mod parser_tests {
    use super::{non_localhost, parse_pretty_hostname};

    #[test]
    fn non_localhost_drops_variants() {
        assert!(non_localhost("localhost".into()).is_empty());
        assert!(non_localhost("LOCALHOST".into()).is_empty());
        assert!(non_localhost("localhost.localdomain".into()).is_empty());
        assert!(non_localhost("localhost.local".into()).is_empty());
    }

    #[test]
    fn non_localhost_keeps_others() {
        assert_eq!(non_localhost("notlocalhost".into()), "notlocalhost");
        assert_eq!(non_localhost("simon".into()), "simon");
    }

    #[test]
    fn parse_picks_quoted_value() {
        assert_eq!(
            parse_pretty_hostname("PRETTY_HOSTNAME=\"Simon's Laptop\"\n").as_deref(),
            Some("Simon's Laptop"),
        );
    }

    #[test]
    fn parse_preserves_inner_whitespace_in_quotes() {
        assert_eq!(
            parse_pretty_hostname("PRETTY_HOSTNAME=\"  My Box  \"\n").as_deref(),
            Some("  My Box  "),
        );
    }

    #[test]
    fn parse_keeps_hash_in_unquoted_value() {
        // systemd's env-file format treats `#` as a comment only at the start of a line.
        assert_eq!(
            parse_pretty_hostname("PRETTY_HOSTNAME=Build#42\n").as_deref(),
            Some("Build#42"),
        );
    }

    #[test]
    fn parse_unquoted_strips_trailing_whitespace() {
        assert_eq!(parse_pretty_hostname("PRETTY_HOSTNAME=hello   \n").as_deref(), Some("hello"),);
    }

    #[test]
    fn parse_skips_unterminated_quote_but_continues() {
        let input = "PRETTY_HOSTNAME=\"unterminated\nPRETTY_HOSTNAME=fallback\n";
        assert_eq!(parse_pretty_hostname(input).as_deref(), Some("fallback"));
    }

    #[test]
    fn parse_ignores_comment_lines() {
        let input = "# PRETTY_HOSTNAME=ignored\nPRETTY_HOSTNAME=real\n";
        assert_eq!(parse_pretty_hostname(input).as_deref(), Some("real"));
    }

    #[test]
    fn parse_returns_none_when_absent() {
        assert!(parse_pretty_hostname("ICON_NAME=computer\n").is_none());
    }

    #[test]
    fn parse_returns_none_for_empty_value() {
        assert!(parse_pretty_hostname("PRETTY_HOSTNAME=\n").is_none());
        assert!(parse_pretty_hostname("PRETTY_HOSTNAME=\"\"\n").is_none());
    }
}
