// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore alnum localdomain notlocalhost

use std::{
    collections::{HashSet, VecDeque},
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::REBUILD_DEBOUNCE;
use crate::protocol::pairing::{
    self, CODE_TIMEOUT, MAX_ATTEMPTS, PROMPT_RATE_LIMIT, PairingRejection, Token, TokenId,
};
use crate::protocol::session;
use crate::protocol::{
    LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PreviewComponent, PreviewConfig,
    PreviewToLspMessage, SLINT_PROTOCOLS_HEADER, SLINT_VERSION, SLINT_VERSION_HEADER,
    SourceFileVersion,
};
#[cfg(not(target_vendor = "apple"))]
use crate::protocol::{TXT_PROTOCOLS_KEY, TXT_SLINT_VERSION_KEY};
use dashmap::{DashMap, Entry};
use futures_util::{
    SinkExt as _, StreamExt as _,
    stream::{SplitSink, SplitStream},
};
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
        protocol::{CloseFrame, frame::coding::CloseCode},
    },
};

/// Write half of an accepted connection.
type Sink = SplitSink<WebSocketStream<TcpStream>, Message>;
/// Read half of an accepted connection.
type Source = SplitStream<WebSocketStream<TcpStream>>;

/// A connection that made it through pairing, on its way from the admitting
/// task to the accept loop.
struct Admitted {
    sink: Sink,
    source: Source,
    remote_addr: SocketAddr,
    sealing: session::Sealing,
    opening: session::Opening,
}

/// How long a peer has to complete the WebSocket upgrade. A peer that opens
/// a socket and then says nothing would otherwise hold a task and a file
/// descriptor for as long as the viewer runs.
const UPGRADE_TIMEOUT: Duration = Duration::from_secs(10);

/// How many connections may be part-way through admission at once. Beyond
/// this the listener drops new sockets: accepting them without bound is how
/// a flood reaches the process file-descriptor limit, and `accept` failing
/// takes the whole listener down with it.
const MAX_PENDING_ADMISSIONS: usize = 16;

/// How long a client has to answer the challenge before we give up on it.
/// Only covers the automatic first exchange, not the part where a human is
/// reading a code off the screen; that gets [`CODE_TIMEOUT`].
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest a cool-down can grow to. Each failed prompt doubles the wait,
/// which is what bounds guessing against a code that never changes: a
/// generated code is a fresh secret every prompt, but one pinned with
/// `--pairing-code` is not, so per-prompt attempt limits alone would let
/// an attacker walk the whole space given enough hours.
const MAX_COOL_DOWN: Duration = Duration::from_secs(15 * 60);

/// How many pairing tokens the viewer remembers. Reconnects reuse a token
/// rather than minting one, so this only fills up if several editors pair
/// with the same viewer, and the oldest falling out just means that editor
/// pairs again.
const MAX_REMEMBERED_TOKENS: usize = 8;

/// How the viewer decides whether to admit a client.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PairingPolicy {
    /// Show a freshly generated code on screen and require the client to
    /// echo it back. The default, and what the mobile viewers always use.
    #[default]
    Generated,
    /// Require this code instead of a generated one, for devices with no
    /// usable display and for scripted clients that know it up front.
    Fixed(String),
    /// Admit anyone who completes the WebSocket handshake. Restores the
    /// behavior from before pairing existed, for trusted networks only.
    Disabled,
}

/// Pairing bookkeeping shared by every incoming connection.
///
/// The screen is a single resource: two clients can't be shown a code at the
/// same time, and a client that keeps knocking mustn't be able to keep the
/// prompt up, which on a viewer that's mid-preview means interrupting it
/// over and over.
#[derive(Default)]
struct PairingState {
    screen: Screen,
    /// Failed prompts since the last successful one, so the cool-down can
    /// escalate against a code that doesn't rotate. Outlives any one
    /// phase, which is why it isn't part of [`Screen`].
    consecutive_failures: u32,
    /// Tokens issued during this viewer run. Deliberately not persisted:
    /// restarting the viewer makes every editor pair again.
    tokens: VecDeque<(TokenId, Token)>,
}

/// What the screen is doing about pairing.
///
/// The three are exclusive, and each carries exactly the instant it needs,
/// so there is no way to be mid-prompt without a start time or to be
/// cooling down while a code is up.
#[derive(Default)]
enum Screen {
    /// Nothing on screen: a knocker can raise a prompt right now.
    #[default]
    Idle,
    /// A code is up. How long it stays up is what its cool-down will be.
    Prompting { since: tokio::time::Instant },
    /// A prompt ended with nobody paired, so nothing may go up before this.
    CoolingDown { until: tokio::time::Instant },
}

impl PairingState {
    /// The token the client announced, if it is one we issued. A plain
    /// lookup, because the id is public; possession of the token itself is
    /// proven by the exchange that follows.
    fn issued_token(&self, id: &TokenId) -> Option<Token> {
        self.tokens.iter().find(|(known, _)| known == id).map(|(_, token)| *token)
    }

    /// Accept `token` on future connections from whoever just paired.
    fn remember_token(&mut self, id: TokenId, token: Token) {
        self.tokens.push_back((id, token));
        while self.tokens.len() > MAX_REMEMBERED_TOKENS {
            self.tokens.pop_front();
        }
    }

    /// Claim the screen for a pairing prompt.
    fn begin_prompt(&mut self) -> Result<(), PairingRejection> {
        let now = tokio::time::Instant::now();
        match self.screen {
            Screen::Prompting { .. } => return Err(PairingRejection::Busy),
            Screen::CoolingDown { until } if now < until => {
                // Round up, so the last fraction of a second never reports
                // as "try again in 0 seconds".
                let left = (until - now).as_millis().div_ceil(1000);
                return Err(PairingRejection::TooSoon { retry_after_seconds: left as u16 });
            }
            // Idle, or a cool-down that has run out.
            Screen::Idle | Screen::CoolingDown { .. } => {}
        }
        self.screen = Screen::Prompting { since: now };
        Ok(())
    }

    /// Release the screen.
    ///
    /// Only a prompt that *failed* starts a cool-down: the limit exists to
    /// stop an unauthenticated peer throwing prompts at the device, and
    /// someone who just typed the right code is plainly not that.
    ///
    /// The cool-down is at least as long as the prompt held the screen. A
    /// flat delay was not enough: a peer that knocks and then says nothing
    /// occupies the device for [`CODE_TIMEOUT`], so a shorter gap let it
    /// own the screen for most of every cycle, forever.
    fn end_prompt(&mut self, accepted: bool) {
        let Screen::Prompting { since } = self.screen else {
            // Nothing was up, so there is nothing to end and no cool-down
            // to earn. Only reachable if a guard outlives its prompt.
            return;
        };
        if accepted {
            self.consecutive_failures = 0;
            self.screen = Screen::Idle;
            return;
        }
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        // Doubling per failure is what bounds guessing against a pinned
        // code: it never rotates, so three tries per prompt would otherwise
        // add up to the whole space over a long enough night.
        let factor = 1u32.checked_shl(self.consecutive_failures - 1).unwrap_or(u32::MAX);
        let held = since.elapsed();
        let cool_down = PROMPT_RATE_LIMIT.max(held).checked_mul(factor).unwrap_or(MAX_COOL_DOWN);
        self.screen = Screen::CoolingDown {
            until: tokio::time::Instant::now() + cool_down.min(MAX_COOL_DOWN),
        };
    }
}

/// Releases the prompt slot and takes the code off the screen however the
/// pairing exchange ends, including when the task is aborted mid-way.
struct PromptGuard {
    state: Arc<Mutex<PairingState>>,
    message_handler: Arc<dyn Fn(ConnectionMessage) + 'static + Send + Sync>,
    remote_addr: SocketAddr,
    /// Set once the code has been entered correctly. Tells the viewer whether
    /// it has to put back whatever the prompt displaced, or whether the
    /// incoming session is about to drive the screen anyway.
    accepted: bool,
}

impl PromptGuard {
    /// Claim the screen and put `code` on it, or report why not.
    ///
    /// Claiming, displaying and tearing down are one thing rather than three
    /// steps a caller has to sequence correctly.
    fn begin(
        state: &Arc<Mutex<PairingState>>,
        message_handler: Arc<dyn Fn(ConnectionMessage) + 'static + Send + Sync>,
        remote_addr: SocketAddr,
        code: &str,
    ) -> Result<Self, PairingRejection> {
        state.lock().unwrap().begin_prompt()?;
        tracing::info!("Showing a pairing code for {remote_addr:?}, valid for {CODE_TIMEOUT:?}");
        message_handler(ConnectionMessage::PairingStarted {
            remote_addr,
            code: code.to_owned(),
            expires_in: CODE_TIMEOUT,
        });
        Ok(Self { state: state.clone(), message_handler, remote_addr, accepted: false })
    }
}

impl Drop for PromptGuard {
    fn drop(&mut self) {
        self.state.lock().unwrap().end_prompt(self.accepted);
        (self.message_handler)(ConnectionMessage::PairingFinished {
            remote_addr: self.remote_addr,
            accepted: self.accepted,
        });
    }
}

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
    SetUserSettings {
        name: String,
        contents: String,
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
    /// Put `code` on screen: someone is trying to connect and needs to read
    /// it off the device. Any preview currently shown is displaced until the
    /// matching [`Self::PairingFinished`].
    PairingStarted {
        remote_addr: SocketAddr,
        code: String,
        expires_in: Duration,
    },
    /// Take the code back off the screen. When `accepted` is false nobody
    /// connected, so the viewer restores what the prompt displaced; when it's
    /// true the newly admitted session takes over from here.
    PairingFinished {
        remote_addr: SocketAddr,
        accepted: bool,
    },
}

pub struct Connection {
    local_addr: SocketAddr,
    thread_handle: Option<(std::thread::JoinHandle<()>, sync::oneshot::Sender<()>)>,
    message_sender: sync::mpsc::UnboundedSender<Outbound>,
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

/// Something queued for the write half.
enum Outbound {
    /// A protocol message, sealed if the session has keys.
    Payload(Vec<u8>),
    /// A WebSocket control frame. Never sealed: it belongs to the transport,
    /// not to the protocol running on top of it.
    Control(Message),
}

/// Serialize a message into the wire format and queue it on the write half.
fn encode_and_send(
    sender: &UnboundedSender<Outbound>,
    message: &impl Serialize,
) -> anyhow::Result<()> {
    let data: Vec<u8> = postcard::to_allocvec(message)?;
    sender.send(Outbound::Payload(data))?;
    Ok(())
}

/// Why a connection didn't make it past pairing.
enum Denied {
    /// Tell the client, then close.
    Pairing(PairingRejection),
    /// The socket died or the peer sent something that isn't part of the
    /// handshake. Nobody to report to.
    Gone,
}

/// Write one message straight to a socket, bypassing the shared outbound
/// channel. Used during pairing, when the connection isn't the current
/// session yet and its sink still belongs to the admitting task.
async fn send_on(sink: &mut Sink, message: &impl Serialize) -> Result<(), Denied> {
    let data: Vec<u8> = postcard::to_allocvec(message).map_err(|_| Denied::Gone)?;
    sink.send(Message::Binary(data.into())).await.map_err(|_| Denied::Gone)
}

/// Read the next protocol message, or `None` if `timeout` elapsed, the peer
/// hung up, or it sent something undecodable.
async fn next_message(receiver: &mut Source, timeout: Duration) -> Option<LspToPreviewMessage> {
    let read = async {
        loop {
            match receiver.next().await? {
                Ok(Message::Binary(bin)) => match postcard::from_bytes(&bin) {
                    Ok(message) => return Some(message),
                    Err(err) => {
                        tracing::warn!("Undecodable message during pairing: {err}");
                        return None;
                    }
                },
                // Keepalive traffic is fine in the middle of the exchange.
                Ok(Message::Ping(_) | Message::Pong(_)) => continue,
                Ok(Message::Text(text)) => {
                    tracing::warn!("Ignoring text message during pairing: {text}");
                    continue;
                }
                Ok(Message::Close(_)) | Err(_) => return None,
                Ok(Message::Frame(_)) => unreachable!("raw frames are never yielded by read"),
            }
        }
    };
    tokio::time::timeout(timeout, read).await.ok().flatten()
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
        pairing_policy: PairingPolicy,
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
                let pairing_state = Arc::new(Mutex::new(PairingState::default()));
                let admissions = Arc::new(sync::Semaphore::new(MAX_PENDING_ADMISSIONS));
                // Connections that finished the handshake and proved themselves. The
                // work leading up to that happens off the accept loop, so a peer that
                // stalls mid-handshake or never enters a code can't hold the listener
                // up, and the running session survives until someone replaces it.
                let (admitted_sender, mut admitted_receiver) =
                    sync::mpsc::unbounded_channel::<Admitted>();
                // The sink is the write half; the JoinHandle is the read-half task. We keep
                // both so an admitted connection can abort the in-flight task and reset shared
                // state before its messages race with the new client's.
                let mut current_session: Option<(Sink, tokio::task::JoinHandle<()>, session::Sealing)> = None;
                loop {
                    tokio::select! {
                        accept = listener.accept() => {
                            match accept {
                                Err(err) => {
                                    tracing::error!("Failed listening for Websocket connections: {err}");
                                    return;
                                }
                                Ok((stream, addr)) => {
                                    let Ok(permit) = admissions.clone().try_acquire_owned() else {
                                        // Dropping `stream` closes it, which is
                                        // the point: better to refuse than to
                                        // let a flood exhaust our descriptors.
                                        tracing::warn!(
                                            "Refusing {addr:?}: {MAX_PENDING_ADMISSIONS} connections are already mid-admission"
                                        );
                                        continue;
                                    };
                                    tracing::info!("Connected to {addr:?}");
                                    tokio::spawn(Self::admit(
                                        stream,
                                        addr,
                                        pairing_policy.clone(),
                                        pairing_state.clone(),
                                        message_handler.clone(),
                                        admitted_sender.clone(),
                                        permit,
                                    ));
                                }
                            }
                        }
                        admitted = admitted_receiver.recv() => {
                            // `admitted_sender` is held by this loop, so the channel
                            // never closes while we're running.
                            let Some(Admitted { sink, source, remote_addr, sealing, opening }) = admitted else { continue };
                            if let Some((_old_sink, old_handle, _)) = current_session.take() {
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
                            let handle = tokio::spawn(Self::handle_connection(
                                source,
                                message_handler.clone(),
                                inner_file_cache.clone(),
                                inner_dependencies.clone(),
                                inner_message_sender.clone(),
                                remote_addr,
                                opening,
                            ));
                            current_session = Some((sink, handle, sealing));
                        }
                        _ = &mut quit_receiver => {
                            tracing::info!("Quit signal received, shutting down connection thread.");
                            break;
                        }
                        message = message_receiver.recv() => {
                            if let (Some(outbound), Some((sink, _, sealing))) = (message, current_session.as_mut()) {
                                let frame = match outbound {
                                    Outbound::Control(frame) => Some(frame),
                                    Outbound::Payload(bytes) => match sealing.seal(bytes) {
                                        Ok(sealed) => Some(Message::Binary(sealed.into())),
                                        Err(err) => {
                                            tracing::error!("Cannot seal outbound frame: {err:?}");
                                            None
                                        }
                                    },
                                };
                                if let Some(frame) = frame
                                    && let Err(err) = sink.send(frame).await {
                                    tracing::error!("Failed sending message to Websocket: {err}");
                                }
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

    /// Take one incoming socket through the WebSocket handshake and the
    /// pairing exchange, and hand it to the accept loop if it gets through.
    ///
    /// Runs off the accept loop on purpose: everything here waits on a peer
    /// we have no reason to trust yet, and one of the waits is for a human to
    /// read a code off a screen.
    async fn admit(
        stream: TcpStream,
        remote_addr: SocketAddr,
        policy: PairingPolicy,
        pairing_state: Arc<Mutex<PairingState>>,
        message_handler: Arc<dyn Fn(ConnectionMessage) + 'static + Send + Sync>,
        admitted: UnboundedSender<Admitted>,
        // Held for as long as this connection is being admitted, so the
        // listener can bound how many are in flight.
        _permit: sync::OwnedSemaphorePermit,
    ) {
        let upgrade = tokio_tungstenite::accept_hdr_async(stream, handshake_callback);
        let stream = match tokio::time::timeout(UPGRADE_TIMEOUT, upgrade).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(err)) => {
                tracing::error!("Failed to establish websocket connection: {err}");
                return;
            }
            Err(_) => {
                tracing::warn!("{remote_addr:?} never completed the WebSocket upgrade");
                return;
            }
        };
        tracing::info!("Websocket established with {remote_addr:?}");
        let (mut sink, mut receiver) = stream.split();

        match Self::authenticate(
            &mut sink,
            &mut receiver,
            remote_addr,
            &policy,
            &pairing_state,
            message_handler,
        )
        .await
        {
            Ok((sealing, opening)) => {
                tracing::info!("Paired with {remote_addr:?}");
                admitted
                    .send(Admitted { sink, source: receiver, remote_addr, sealing, opening })
                    .ok();
            }
            Err(Denied::Gone) => {
                tracing::info!("{remote_addr:?} gave up before pairing completed");
            }
            Err(Denied::Pairing(reason)) => {
                tracing::warn!("Refused {remote_addr:?}: {reason}");
                send_on(&mut sink, &PreviewToLspMessage::PairingRejected { reason }).await.ok();
                sink.send(Message::Close(Some(CloseFrame {
                    code: CloseCode::Policy,
                    reason: reason.to_string().as_str().into(),
                })))
                .await
                .ok();
                sink.close().await.ok();
            }
        }
    }

    /// Run the pairing exchange described in [`crate::protocol::pairing`].
    ///
    /// Deliberately touches none of the session state: until this returns
    /// `Ok` the caller is just a peer on a socket, and the editor that is
    /// already connected keeps its session and its file cache.
    async fn authenticate(
        sink: &mut Sink,
        receiver: &mut Source,
        remote_addr: SocketAddr,
        policy: &PairingPolicy,
        pairing_state: &Arc<Mutex<PairingState>>,
        message_handler: Arc<dyn Fn(ConnectionMessage) + 'static + Send + Sync>,
    ) -> Result<(session::Sealing, session::Opening), Denied> {
        send_on(sink, &PreviewToLspMessage::PairingReady).await?;

        // The client answers this much on its own; no human is involved yet.
        let announced = match next_message(receiver, HELLO_TIMEOUT).await {
            Some(LspToPreviewMessage::PairingHello { token }) => token,
            Some(other) => {
                tracing::warn!("Expected PairingHello from {remote_addr:?}, got {other:?}");
                return Err(Denied::Gone);
            }
            None => return Err(Denied::Gone),
        };

        if *policy == PairingPolicy::Disabled {
            // --no-pairing: everyone is admitted, so there is no shared
            // secret, and nothing to key a sealed session with either.
            send_on(sink, &PreviewToLspMessage::PairingAccepted).await?;
            return Ok((session::Sealing::Plaintext, session::Opening::Plaintext));
        }

        if let Some(id) = announced {
            // Bound first: holding the guard across the awaits below would
            // make this future non-Send.
            let issued = pairing_state.lock().unwrap().issued_token(&id);
            if let Some(token) = issued {
                if let Some(session) =
                    Self::token_exchange(sink, receiver, remote_addr, &token).await?
                {
                    return Ok(session);
                }
                // Announced one of our ids but couldn't prove the token:
                // treated exactly like a token we never issued.
            } else {
                tracing::info!("{remote_addr:?} announced a token we did not issue");
            }
            // Almost always a token from a previous run of this viewer. Say
            // so, so the client forgets it, then fall through to the code.
            let reason = PairingRejection::BadToken;
            send_on(sink, &PreviewToLspMessage::PairingRejected { reason }).await?;
        }

        let code = match policy {
            PairingPolicy::Fixed(code) => code.clone(),
            _ => pairing::generate::code(),
        };

        // From here on the prompt is up, so every exit has to take it down.
        let mut guard = PromptGuard::begin(pairing_state, message_handler, remote_addr, &code)
            .map_err(Denied::Pairing)?;

        let deadline = tokio::time::Instant::now() + CODE_TIMEOUT;
        let remaining = || deadline.saturating_duration_since(tokio::time::Instant::now());
        let mut attempts_left = MAX_ATTEMPTS;

        loop {
            // A fresh exchange per attempt: the state is consumed by
            // finishing, and reusing an element across guesses would leak
            // more than one guess's worth.
            let handshake = pairing::Handshake::with_code(pairing::Role::Viewer, &code);
            send_on(
                sink,
                &PreviewToLspMessage::PairingRequired {
                    attempts_left,
                    expires_in_seconds: remaining().as_secs() as u16,
                    element: handshake.element().clone(),
                },
            )
            .await?;

            let Some(message) = next_message(receiver, remaining()).await else {
                // End of stream covers both a deadline that lapsed and a peer
                // that vanished. The clock is what tells them apart, and
                // calling a disconnect a timeout makes logs read wrong.
                return Err(if remaining().is_zero() {
                    Denied::Pairing(PairingRejection::Expired)
                } else {
                    Denied::Gone
                });
            };
            let LspToPreviewMessage::PairingResponse { element, confirmation } = message else {
                tracing::warn!("Ignoring {message:?} from {remote_addr:?} during pairing");
                continue;
            };

            // A wrong code doesn't fail here: it produces different keys,
            // and only the confirmation tells us so. That is the point --
            // the wire carries nothing to test a guess against offline.
            if let Ok(secrets) = handshake.finish_confirmed(&element, &confirmation) {
                tracing::info!("{remote_addr:?} entered the pairing code correctly");
                pairing_state.lock().unwrap().remember_token(secrets.token_id, secrets.token);
                send_on(
                    sink,
                    &PreviewToLspMessage::PairingConfirm { confirmation: secrets.confirmation() },
                )
                .await?;
                // Tells the guard the incoming session is about to drive the
                // screen, so the viewer shouldn't restore what we displaced.
                guard.accepted = true;
                return Ok(secrets.session());
            }

            attempts_left -= 1;
            if attempts_left == 0 {
                return Err(Denied::Pairing(PairingRejection::TooManyAttempts));
            }
            send_on(
                sink,
                &PreviewToLspMessage::PairingRejected { reason: PairingRejection::BadCode },
            )
            .await?;
        }
    }

    /// Run the reconnect exchange, with `token` as the secret.
    ///
    /// The same exchange as for a code, minus the human: the token never
    /// crosses the wire, and the session keys are fresh, so recording this
    /// connection is worthless even to someone who steals the token later.
    ///
    /// `Ok(None)` means the peer announced the token but could not prove
    /// holding it; the caller treats that like a token we never issued.
    async fn token_exchange(
        sink: &mut Sink,
        receiver: &mut Source,
        remote_addr: SocketAddr,
        token: &Token,
    ) -> Result<Option<(session::Sealing, session::Opening)>, Denied> {
        let handshake = pairing::Handshake::with_token(pairing::Role::Viewer, token);
        send_on(
            sink,
            &PreviewToLspMessage::PairingTokenChallenge { element: handshake.element().clone() },
        )
        .await?;

        let Some(message) = next_message(receiver, HELLO_TIMEOUT).await else {
            return Err(Denied::Gone);
        };
        let LspToPreviewMessage::PairingResponse { element, confirmation } = message else {
            tracing::warn!("Expected the reconnect response from {remote_addr:?}, got {message:?}");
            return Err(Denied::Gone);
        };

        let Ok(secrets) = handshake.finish_confirmed(&element, &confirmation) else {
            tracing::info!("{remote_addr:?} announced a token it could not prove");
            return Ok(None);
        };

        // An editor we already paired with, reconnecting. Keep the screen
        // alone: this is the common case after a network blip or the viewer
        // coming back to the foreground.
        tracing::info!("{remote_addr:?} proved its token; no code needed");
        send_on(
            sink,
            &PreviewToLspMessage::PairingConfirm { confirmation: secrets.confirmation() },
        )
        .await?;
        Ok(Some(secrets.session()))
    }

    async fn handle_connection(
        mut receiver: SplitStream<WebSocketStream<TcpStream>>,
        message_handler: Arc<dyn Fn(ConnectionMessage) + 'static + Send + Sync>,
        file_cache: FileCache,
        dependencies: Arc<Mutex<HashSet<Url>>>,
        message_sender: UnboundedSender<Outbound>,
        remote_addr: SocketAddr,
        mut opening: session::Opening,
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
                            let Ok(plain) = opening.open(&bin) else {
                                // Tampered, reordered, or from a peer that
                                // derived a different key.
                                tracing::error!("Dropping a frame that failed to open");
                                break;
                            };
                            match postcard::from_bytes::<LspToPreviewMessage>(&plain) {
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
                                        LspToPreviewMessage::SetUserSettings { name, contents } => {
                                            message_handler(ConnectionMessage::SetUserSettings {
                                                name,
                                                contents,
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
                                        LspToPreviewMessage::OpenProject { .. } => {}
                                        // Pairing is settled before the session starts, so
                                        // these can only be a confused or malicious peer.
                                        LspToPreviewMessage::PairingHello { .. }
                                        | LspToPreviewMessage::PairingResponse { .. } => {
                                            tracing::warn!(
                                                "Ignoring pairing message on an established session"
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
                            message_sender.send(Outbound::Control(Message::Pong(data))).ok();
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
            && let Err(err) = self.send(PreviewToLspMessage::RequestState {
                files: vec![url.clone()],
                settings: vec![],
            })
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
mod cool_down_tests {
    use super::{PairingRejection, PairingState, Screen};
    use crate::protocol::pairing::{CODE_TIMEOUT, PROMPT_RATE_LIMIT};
    use std::time::Duration;

    /// A peer that knocks and then says nothing holds the screen for the
    /// whole deadline, so a flat gap shorter than that let it own the
    /// device for most of every cycle.
    #[tokio::test(start_paused = true)]
    async fn an_abandoned_prompt_costs_as_long_as_it_held_the_screen() {
        let mut state = PairingState::default();
        state.begin_prompt().unwrap();
        tokio::time::advance(CODE_TIMEOUT).await;
        state.end_prompt(false);

        tokio::time::advance(CODE_TIMEOUT / 2).await;
        assert!(
            matches!(state.begin_prompt(), Err(PairingRejection::TooSoon { .. })),
            "the screen was free again before the prompt's own length had passed"
        );
        tokio::time::advance(CODE_TIMEOUT / 2).await;
        assert!(state.begin_prompt().is_ok());
    }

    /// A user who fumbles their codes quickly should not be charged for
    /// time nobody spent occupying the screen.
    #[tokio::test(start_paused = true)]
    async fn a_quick_failure_costs_only_the_base_limit() {
        let mut state = PairingState::default();
        state.begin_prompt().unwrap();
        tokio::time::advance(Duration::from_secs(5)).await;
        state.end_prompt(false);

        tokio::time::advance(PROMPT_RATE_LIMIT - Duration::from_secs(1)).await;
        assert!(matches!(state.begin_prompt(), Err(PairingRejection::TooSoon { .. })));
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(state.begin_prompt().is_ok());
    }

    /// A code pinned with `--pairing-code` never rotates, so the per-prompt
    /// attempt limit does not bound total guesses on its own.
    #[tokio::test(start_paused = true)]
    async fn repeated_failures_escalate_the_wait() {
        let mut state = PairingState::default();
        let mut previous = Duration::ZERO;
        for _ in 0..4 {
            state.begin_prompt().unwrap();
            state.end_prompt(false);
            let waited = wait_out_cool_down(&mut state).await;
            assert!(waited > previous, "the wait did not grow: {waited:?} after {previous:?}");
            previous = waited;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn pairing_successfully_clears_the_escalation() {
        let mut state = PairingState::default();
        for _ in 0..3 {
            state.begin_prompt().unwrap();
            state.end_prompt(false);
            wait_out_cool_down(&mut state).await;
        }
        state.begin_prompt().unwrap();
        state.end_prompt(true);

        state.begin_prompt().unwrap();
        state.end_prompt(false);
        assert_eq!(
            wait_out_cool_down(&mut state).await,
            PROMPT_RATE_LIMIT,
            "a successful pairing should put the escalation back to the start"
        );
    }

    /// Advance until the screen is free again, returning how long that took.
    async fn wait_out_cool_down(state: &mut PairingState) -> Duration {
        let mut waited = Duration::ZERO;
        loop {
            match state.begin_prompt() {
                Ok(()) => {
                    state.screen = Screen::Idle;
                    return waited;
                }
                Err(_) => {
                    tokio::time::advance(Duration::from_secs(1)).await;
                    waited += Duration::from_secs(1);
                }
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn pairing_successfully_costs_nothing() {
        let mut state = PairingState::default();
        state.begin_prompt().unwrap();
        tokio::time::advance(CODE_TIMEOUT).await;
        state.end_prompt(true);
        assert!(state.begin_prompt().is_ok());
    }
}

#[cfg(test)]
mod session_tests {
    use super::{Connection, ConnectionMessage, MAX_PENDING_ADMISSIONS, PairingPolicy};
    use crate::protocol::pairing::{self, MAX_ATTEMPTS, Token, TokenId};
    use crate::protocol::session;
    use crate::protocol::{
        LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PairingRejection, PreviewToLspMessage,
    };
    use futures_util::{SinkExt as _, StreamExt as _};
    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
    use tokio_tungstenite::tungstenite::{
        Message,
        client::IntoClientRequest as _,
        http::{HeaderValue, header::SEC_WEBSOCKET_PROTOCOL},
    };

    /// Fails the test rather than hanging it if the viewer never answers.
    const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

    type Client = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    /// A viewer under test, plus the messages it pushed to its UI.
    struct Viewer {
        connection: Connection,
        events: UnboundedReceiver<ConnectionMessage>,
    }

    impl Viewer {
        async fn start(policy: PairingPolicy) -> Self {
            let (sender, events) = unbounded_channel();
            let connection = Connection::listen(
                Some(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
                None,
                policy,
                move |message| {
                    let _ = sender.send(message);
                },
            )
            .await
            .unwrap();
            Self { connection, events }
        }

        /// Open a socket and complete the WebSocket handshake, stopping short
        /// of pairing.
        async fn dial(&self) -> Client {
            let mut request = format!("ws://127.0.0.1:{}", self.connection.local_port())
                .into_client_request()
                .unwrap();
            request
                .headers_mut()
                .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static(PROTOCOL_SUBPROTOCOL));
            tokio_tungstenite::connect_async(request).await.unwrap().0
        }

        /// Wait for the next event the viewer would have shown on screen.
        async fn next_event(&mut self) -> ConnectionMessage {
            tokio::time::timeout(REPLY_TIMEOUT, self.events.recv())
                .await
                .expect("viewer produced no event")
                .expect("viewer event channel closed")
        }

        /// Wait for a prompt to come down with the given outcome.
        async fn expect_prompt_finished(&mut self, accepted: bool) {
            loop {
                if let ConnectionMessage::PairingFinished { accepted: got, .. } =
                    self.next_event().await
                {
                    assert_eq!(got, accepted, "prompt ended with the wrong outcome");
                    return;
                }
            }
        }

        /// The code from the next `PairingStarted`, skipping anything else.
        async fn next_code(&mut self) -> String {
            loop {
                if let ConnectionMessage::PairingStarted { code, .. } = self.next_event().await {
                    return code;
                }
            }
        }
    }

    async fn send(client: &mut Client, message: &LspToPreviewMessage) {
        let bytes = postcard::to_allocvec(message).unwrap();
        client.send(Message::Binary(bytes.into())).await.unwrap();
    }

    /// Raw bytes of the next frame from the viewer, before decoding.
    async fn recv_raw(client: &mut Client) -> Option<Vec<u8>> {
        tokio::time::timeout(REPLY_TIMEOUT, async {
            loop {
                match client.next().await? {
                    Ok(Message::Binary(bytes)) => return Some(bytes.to_vec()),
                    Ok(Message::Close(_)) | Err(_) => return None,
                    Ok(_) => continue,
                }
            }
        })
        .await
        .expect("viewer sent no reply")
    }

    /// Next protocol message from the viewer, or `None` if it closed the socket.
    async fn recv(client: &mut Client) -> Option<PreviewToLspMessage> {
        tokio::time::timeout(REPLY_TIMEOUT, async {
            loop {
                match client.next().await? {
                    Ok(Message::Binary(bytes)) => {
                        return Some(postcard::from_bytes(&bytes).unwrap());
                    }
                    Ok(Message::Close(_)) | Err(_) => return None,
                    Ok(_) => continue,
                }
            }
        })
        .await
        .expect("viewer sent no reply")
    }

    /// Read the viewer's opener and answer it, announcing `token`.
    async fn hello(client: &mut Client, token: Option<TokenId>) {
        assert!(
            matches!(recv(client).await, Some(PreviewToLspMessage::PairingReady)),
            "expected the viewer to open pairing"
        );
        send(client, &LspToPreviewMessage::PairingHello { token }).await;
    }

    /// Complete the editor's half of the exchange for `code`.
    ///
    /// Returns the derived secrets, or the viewer's rejection. A wrong code
    /// gets all the way to the confirmation exchange -- that is the property
    /// under test.
    async fn answer_code(
        client: &mut Client,
        code: &str,
        element: &pairing::Element,
    ) -> Result<pairing::Secrets, PairingRejection> {
        let handshake = pairing::Handshake::with_code(pairing::Role::Editor, code);
        answer_with(client, handshake, element).await
    }

    /// Send the response for a started exchange and read the verdict.
    async fn answer_with(
        client: &mut Client,
        handshake: pairing::Handshake,
        element: &pairing::Element,
    ) -> Result<pairing::Secrets, PairingRejection> {
        let ours = handshake.element().clone();
        let secrets = handshake.finish(element).expect("the viewer's element is well formed");
        send(
            client,
            &LspToPreviewMessage::PairingResponse {
                element: ours,
                confirmation: secrets.confirmation(),
            },
        )
        .await;
        match recv(client).await {
            Some(PreviewToLspMessage::PairingConfirm { confirmation }) => {
                assert!(
                    secrets.peer_confirms(&confirmation),
                    "the viewer confirmed with a key we did not derive"
                );
                Ok(secrets)
            }
            Some(PreviewToLspMessage::PairingRejected { reason }) => Err(reason),
            other => panic!("unexpected answer to the pairing response: {other:?}"),
        }
    }

    /// Dial as a stranger and wait for the viewer to put a code on screen.
    async fn knock(viewer: &Viewer) -> (Client, pairing::Element) {
        let mut client = viewer.dial().await;
        hello(&mut client, None).await;
        let Some(PreviewToLspMessage::PairingRequired { element, .. }) = recv(&mut client).await
        else {
            panic!("expected a code prompt");
        };
        (client, element)
    }

    /// A client past the handshake, sealing and opening like the real one.
    struct Paired {
        client: Client,
        sealing: session::Sealing,
        opening: session::Opening,
    }

    impl Paired {
        async fn send(&mut self, message: &LspToPreviewMessage) {
            let bytes = postcard::to_allocvec(message).unwrap();
            let sealed = self.sealing.seal(bytes).unwrap();
            self.client.send(Message::Binary(sealed.into())).await.unwrap();
        }

        async fn recv(&mut self) -> Option<PreviewToLspMessage> {
            let raw = recv_raw(&mut self.client).await?;
            let plain = self.opening.open(&raw).expect("the viewer sealed with another key");
            Some(postcard::from_bytes(&plain).unwrap())
        }
    }

    /// Dial, pair with the code the viewer displays, and return a sealed
    /// session plus everything the exchange derived.
    async fn pair(viewer: &mut Viewer) -> (Paired, pairing::Secrets) {
        let (mut client, element) = knock(viewer).await;
        let code = viewer.next_code().await;
        let secrets = answer_code(&mut client, &code, &element).await.expect("accepted");
        let (sealing, opening) = secrets.session();
        (Paired { client, sealing, opening }, secrets)
    }

    /// Dial again and run the token exchange, as a reconnecting editor would.
    async fn reconnect(viewer: &Viewer, secrets: &pairing::Secrets) -> Paired {
        let mut client = viewer.dial().await;
        hello(&mut client, Some(secrets.token_id)).await;
        let Some(PreviewToLspMessage::PairingTokenChallenge { element }) = recv(&mut client).await
        else {
            panic!("expected the reconnect exchange to open");
        };
        let handshake = pairing::Handshake::with_token(pairing::Role::Editor, &secrets.token);
        let fresh = answer_with(&mut client, handshake, &element).await.expect("accepted");
        let (sealing, opening) = fresh.session();
        Paired { client, sealing, opening }
    }

    #[tokio::test]
    async fn correct_code_is_accepted_and_session_works() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let (mut client, _secrets) = pair(&mut viewer).await;

        // The prompt comes down, and the viewer is told the newcomer took over.
        assert!(matches!(
            viewer.next_event().await,
            ConnectionMessage::PairingFinished { accepted: true, .. }
        ));
        assert!(matches!(viewer.next_event().await, ConnectionMessage::Connected { .. }));

        // The session is live, and sealed: the keepalive round trip works
        // only because both sides derived the same keys.
        client.send(&LspToPreviewMessage::Ping).await;
        assert!(matches!(client.recv().await, Some(PreviewToLspMessage::Pong)));
    }

    #[tokio::test]
    async fn wrong_code_burns_attempts_then_closes() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let mut client = viewer.dial().await;
        hello(&mut client, None).await;

        let Some(PreviewToLspMessage::PairingRequired { attempts_left, mut element, .. }) =
            recv(&mut client).await
        else {
            panic!("expected a code prompt");
        };
        assert_eq!(attempts_left, MAX_ATTEMPTS);
        let code = viewer.next_code().await;
        assert_eq!(code.len(), pairing::CODE_DIGITS as usize);
        assert!(code.chars().all(|c| c.is_ascii_digit()), "generated code {code} is not digits");
        let wrong = if code == "0000" { "1111" } else { "0000" };

        for expected_left in (1..MAX_ATTEMPTS).rev() {
            assert!(matches!(
                answer_code(&mut client, wrong, &element).await,
                Err(PairingRejection::BadCode)
            ));
            let Some(PreviewToLspMessage::PairingRequired { attempts_left, element: next, .. }) =
                recv(&mut client).await
            else {
                panic!("expected another code prompt");
            };
            assert_eq!(attempts_left, expected_left);
            // A fresh exchange per attempt, so one guess never helps the next.
            element = next;
        }

        assert!(matches!(
            answer_code(&mut client, wrong, &element).await,
            Err(PairingRejection::TooManyAttempts)
        ));
        assert!(recv(&mut client).await.is_none(), "viewer should close the socket");
    }

    /// Peers that open a socket and say nothing used to accumulate a task
    /// and a descriptor each, without limit, until `accept` failed and took
    /// the listener down. They are bounded now, and the listener keeps
    /// serving real clients underneath them.
    #[tokio::test]
    async fn silent_peers_do_not_stop_the_listener() {
        let viewer = Viewer::start(PairingPolicy::Generated).await;
        let addr = format!("127.0.0.1:{}", viewer.connection.local_port());

        let mut silent = Vec::new();
        for _ in 0..MAX_PENDING_ADMISSIONS - 1 {
            silent.push(tokio::net::TcpStream::connect(&addr).await.unwrap());
        }

        let mut client = viewer.dial().await;
        hello(&mut client, None).await;
        assert!(
            matches!(recv(&mut client).await, Some(PreviewToLspMessage::PairingRequired { .. })),
            "the listener stopped serving while silent peers were connected"
        );
        drop(silent);
    }

    /// Nothing an observer can use may reach the wire: not the code, not the
    /// token, not the session keys.
    #[tokio::test]
    async fn nothing_secret_reaches_the_wire() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let mut client = viewer.dial().await;

        let mut frames: Vec<Vec<u8>> = Vec::new();
        frames.push(recv_raw(&mut client).await.expect("opener"));
        send(&mut client, &LspToPreviewMessage::PairingHello { token: None }).await;

        let prompt = recv_raw(&mut client).await.expect("prompt");
        frames.push(prompt.clone());
        let PreviewToLspMessage::PairingRequired { element, .. } =
            postcard::from_bytes(&prompt).unwrap()
        else {
            panic!("expected a code prompt");
        };

        let code = viewer.next_code().await;
        let handshake = pairing::Handshake::with_code(pairing::Role::Editor, &code);
        let ours = handshake.element().clone();
        let secrets = handshake.finish(&element).unwrap();
        send(
            &mut client,
            &LspToPreviewMessage::PairingResponse {
                element: ours,
                confirmation: secrets.confirmation(),
            },
        )
        .await;
        frames.push(recv_raw(&mut client).await.expect("confirmation"));

        let [key_down, key_up] = secrets.key_material();
        for frame in &frames {
            for (what, needle) in [
                ("the pairing code", code.as_bytes()),
                ("the reconnect token", secrets.token.as_bytes()),
                ("a session key", key_down),
                ("a session key", key_up),
            ] {
                assert!(
                    !frame.windows(needle.len()).any(|w| w == needle),
                    "{what} appeared on the wire"
                );
            }
        }

        // ... and the token both sides derived is the one that reconnects.
        drop(client);
        let mut again = reconnect(&viewer, &secrets).await;
        again.send(&LspToPreviewMessage::Ping).await;
        assert!(matches!(again.recv().await, Some(PreviewToLspMessage::Pong)));
    }

    #[tokio::test]
    async fn a_token_reconnects_without_a_prompt() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let (first, secrets) = pair(&mut viewer).await;
        drop(first);

        let mut client = reconnect(&viewer, &secrets).await;
        // The session is sealed with keys from the fresh exchange.
        client.send(&LspToPreviewMessage::Ping).await;
        assert!(matches!(client.recv().await, Some(PreviewToLspMessage::Pong)));
        // No code was ever generated for the second connection.
        assert!(
            !viewer
                .events
                .try_recv()
                .is_ok_and(|event| matches!(event, ConnectionMessage::PairingStarted { .. }))
        );
    }

    /// Two reconnects with one token must not share keys: recording a
    /// session and later stealing the token opens nothing.
    #[tokio::test]
    async fn every_reconnect_has_fresh_keys() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let (first, secrets) = pair(&mut viewer).await;
        drop(first);

        let mut one = reconnect(&viewer, &secrets).await;
        let mut two = reconnect(&viewer, &secrets).await;
        let a = one.sealing.seal(postcard::to_allocvec(&LspToPreviewMessage::Ping).unwrap());
        let b = two.sealing.seal(postcard::to_allocvec(&LspToPreviewMessage::Ping).unwrap());
        assert_ne!(a.unwrap(), b.unwrap(), "two sessions sealed alike");
    }

    #[tokio::test]
    async fn an_unknown_token_falls_back_to_the_code() {
        let viewer = Viewer::start(PairingPolicy::Generated).await;
        let mut client = viewer.dial().await;
        // A token from some other viewer, or from a previous run of this one.
        hello(&mut client, Some(TokenId::for_test(9))).await;

        assert!(matches!(
            recv(&mut client).await,
            Some(PreviewToLspMessage::PairingRejected { reason: PairingRejection::BadToken })
        ));
        assert!(matches!(
            recv(&mut client).await,
            Some(PreviewToLspMessage::PairingRequired { .. })
        ));
    }

    /// A token id crosses the wire in the clear, so anyone can announce it.
    /// Without the token behind it, the exchange must fail and leave the
    /// announcer no better off than any stranger.
    #[tokio::test]
    async fn an_id_without_the_token_is_refused() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let (first, secrets) = pair(&mut viewer).await;
        drop(first);

        let mut sniffer = viewer.dial().await;
        hello(&mut sniffer, Some(secrets.token_id)).await;
        let Some(PreviewToLspMessage::PairingTokenChallenge { element }) = recv(&mut sniffer).await
        else {
            panic!("expected the reconnect exchange to open");
        };
        let handshake = pairing::Handshake::with_token(pairing::Role::Editor, &Token::for_test(9));
        assert!(matches!(
            answer_with(&mut sniffer, handshake, &element).await,
            Err(PairingRejection::BadToken)
        ));
        // ... and ends up at the code prompt, like any stranger.
        assert!(matches!(
            recv(&mut sniffer).await,
            Some(PreviewToLspMessage::PairingRequired { .. })
        ));
    }

    #[tokio::test]
    async fn a_knocker_cannot_displace_a_live_session() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let (mut established, _secrets) = pair(&mut viewer).await;
        // Clear the setup pairing's own prompt, so the wait below can only
        // match the intruder's.
        viewer.expect_prompt_finished(true).await;

        // Someone else turns up and gets as far as a code prompt, which is
        // the furthest an unauthenticated peer can go.
        let mut intruder = viewer.dial().await;
        hello(&mut intruder, None).await;
        assert!(matches!(
            recv(&mut intruder).await,
            Some(PreviewToLspMessage::PairingRequired { .. })
        ));
        // ... and gives up without ever entering the code.
        drop(intruder);
        viewer.expect_prompt_finished(false).await;

        // The original session is untouched.
        established.send(&LspToPreviewMessage::Ping).await;
        assert!(matches!(established.recv().await, Some(PreviewToLspMessage::Pong)));
    }

    #[tokio::test]
    async fn a_second_knocker_is_turned_away_while_a_prompt_is_up() {
        let viewer = Viewer::start(PairingPolicy::Generated).await;
        let mut first = viewer.dial().await;
        hello(&mut first, None).await;
        assert!(matches!(
            recv(&mut first).await,
            Some(PreviewToLspMessage::PairingRequired { .. })
        ));

        let mut second = viewer.dial().await;
        hello(&mut second, None).await;
        assert!(matches!(
            recv(&mut second).await,
            Some(PreviewToLspMessage::PairingRejected { reason: PairingRejection::Busy })
        ));
        assert!(recv(&mut second).await.is_none(), "viewer should close the second socket");
    }

    /// A prompt nobody completed starts the cool-down, and retrying inside
    /// it is told so specifically rather than blamed on another computer
    /// that isn't there.
    #[tokio::test]
    async fn a_failed_prompt_holds_off_the_next_one() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;

        let mut abandoned = viewer.dial().await;
        hello(&mut abandoned, None).await;
        assert!(matches!(
            recv(&mut abandoned).await,
            Some(PreviewToLspMessage::PairingRequired { .. })
        ));
        // Walk away from it: the prompt comes down unanswered.
        drop(abandoned);
        viewer.expect_prompt_finished(false).await;

        let mut knocker = viewer.dial().await;
        hello(&mut knocker, None).await;
        assert!(matches!(
            recv(&mut knocker).await,
            Some(PreviewToLspMessage::PairingRejected { reason: PairingRejection::TooSoon { .. } })
        ));
    }

    /// Pairing correctly must not cost the user a cool-down. Reloading an
    /// editor drops its token, so this is the path straight back to a code.
    #[tokio::test]
    async fn a_successful_pairing_does_not_hold_off_the_next_one() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let (client, _secrets) = pair(&mut viewer).await;
        drop(client);

        // A brand new editor, with no token, pairing straight afterwards.
        let mut fresh = viewer.dial().await;
        hello(&mut fresh, None).await;
        let Some(PreviewToLspMessage::PairingRequired { element, .. }) = recv(&mut fresh).await
        else {
            panic!("a successful pairing must not lock out the next one");
        };
        let code = viewer.next_code().await;
        answer_code(&mut fresh, &code, &element).await.expect("accepted");
    }

    #[tokio::test]
    async fn a_fixed_code_is_accepted_without_reading_the_screen() {
        let viewer = Viewer::start(PairingPolicy::Fixed("1357".into())).await;
        let (mut client, element) = knock(&viewer).await;

        answer_code(&mut client, "1357", &element).await.expect("accepted");
    }

    #[tokio::test]
    async fn pairing_disabled_admits_without_a_code() {
        let mut viewer = Viewer::start(PairingPolicy::Disabled).await;
        let mut client = viewer.dial().await;
        hello(&mut client, None).await;

        assert!(matches!(recv(&mut client).await, Some(PreviewToLspMessage::PairingAccepted)));
        assert!(matches!(viewer.next_event().await, ConnectionMessage::Connected { .. }));
    }

    /// An element harvested from some other exchange must not be usable
    /// here, even with the right code: the transcript binds both halves.
    #[tokio::test]
    async fn an_element_from_another_exchange_is_refused() {
        let mut viewer = Viewer::start(PairingPolicy::Generated).await;
        let (mut client, _element) = knock(&viewer).await;
        let code = viewer.next_code().await;

        // A well-formed element, but from an exchange this viewer never ran.
        let elsewhere = pairing::Handshake::with_code(pairing::Role::Viewer, &code);
        assert!(matches!(
            answer_code(&mut client, &code, elsewhere.element()).await,
            Err(PairingRejection::BadCode)
        ));
    }

    #[tokio::test]
    async fn a_garbage_element_is_refused() {
        let viewer = Viewer::start(PairingPolicy::Generated).await;
        let mut client = viewer.dial().await;
        hello(&mut client, None).await;
        let Some(PreviewToLspMessage::PairingRequired { element, .. }) = recv(&mut client).await
        else {
            panic!("expected a code prompt");
        };

        // Nonsense in place of a group element, with a confirmation that
        // cannot possibly match.
        let handshake = pairing::Handshake::with_code(pairing::Role::Editor, "0000");
        let secrets = handshake.finish(&element).unwrap();
        send(
            &mut client,
            &LspToPreviewMessage::PairingResponse {
                element: pairing::Element::for_test(vec![0u8; 33]),
                confirmation: secrets.confirmation(),
            },
        )
        .await;
        assert!(matches!(
            recv(&mut client).await,
            Some(PreviewToLspMessage::PairingRejected { reason: PairingRejection::BadCode })
        ));
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
