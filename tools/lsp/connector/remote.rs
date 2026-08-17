// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore backgrounded

//! LSP-server side of the remote-preview connection: owns the WebSocket.
//! Discovery and the dialog live in the preview process; see
//! [`crate::preview::remote`].

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures_util::{
    SinkExt as _,
    lock::Mutex,
    stream::{SplitSink, SplitStream, StreamExt as _},
};
use i_slint_live_preview::protocol::pairing::{self, MAX_ATTEMPTS, Token, TokenId};
use i_slint_live_preview::protocol::session;
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PairingRejection, PreviewToLspMessage,
    RemoteConnectionState, SLINT_PROTOCOLS_HEADER, SLINT_VERSION, SLINT_VERSION_HEADER,
};
use tokio::sync::mpsc;
use tokio_tungstenite_wasm::{Message, WebSocketStream};

use crate::editor_preview::LspToPreviews;

/// How often the keepalive probes the remote viewer.
const PING_INTERVAL: Duration = Duration::from_secs(5);
/// Without a pong for this long, the connection counts as dead.
/// Mobile devices abort a backgrounded app's connections without notifying
/// the peer, so the socket alone can't tell us.
const PONG_TIMEOUT: Duration = Duration::from_secs(15);
/// Pause between reconnect attempts.
const RECONNECT_DELAY: Duration = Duration::from_secs(3);
/// Cap on a single connection attempt.
/// A device that blocks network for a backgrounded viewer can swallow
/// packets; an uncapped dial would then hang for minutes on TCP retransmissions.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the editor gives one round of automatic handshake steps. The
/// viewer enforces its own deadlines and closes the socket, but a dropped
/// connection or a hostile peer delivers no close, so without a deadline
/// the editor's read would park forever and wedge the dialog on
/// "Connecting" until the LSP is restarted.
///
/// It bounds a whole round, not each read, so a peer can't dodge it by
/// dribbling one ignorable frame just under the limit. The clock is reset
/// only when the user makes progress -- typing a code, accepting a warning
/// -- which is the one thing that legitimately takes real time. Generous:
/// an automatic round is a couple of local computations and sends.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// What the user did with the pairing prompt.
enum PairingSubmission {
    Code(String),
    /// Connect to a pairing-disabled viewer even though the session will
    /// not be encrypted.
    AcceptUnpaired,
    Cancel,
}

/// One `PairingRequired` message's worth of prompt, as handed to
/// [`RemoteLspToPreview::answer_prompt`].
struct Prompt {
    attempts_left: u8,
    expires_in_seconds: u16,
    element: pairing::Element,
}

/// How one run of the editor's half of the exchange ended.
enum ExchangeVerdict {
    /// The viewer proved it derived the same key.
    Confirmed(pairing::Secrets),
    /// The viewer turned the secret down.
    Rejected(PairingRejection),
}

/// Why a dial attempt produced no session.
enum ConnectError {
    /// Worth retrying on a timer: nothing listening, socket died, superseded.
    Transient(String),
    /// Retrying won't help until the user acts, so the reconnect loop stops
    /// and hands back to the dialog.
    Fatal(String),
}

/// Classify a rejection the viewer sent. The rule itself lives on
/// [`PairingRejection::is_terminal`], next to the variants it judges.
fn from_rejection(reason: PairingRejection) -> ConnectError {
    let text = reason.to_string();
    if reason.is_terminal() { ConnectError::Fatal(text) } else { ConnectError::Transient(text) }
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transient(reason) | Self::Fatal(reason) => f.write_str(reason),
        }
    }
}

/// Clears the pairing input slot when the prompt is over, so a code typed
/// too late can't land in an unrelated attempt.
struct PairingInputGuard {
    slot: Rc<RefCell<Option<mpsc::UnboundedSender<PairingSubmission>>>>,
    /// The sender this guard installed, so it can tell whether the slot is
    /// still its own.
    mine: mpsc::UnboundedSender<PairingSubmission>,
}

impl PairingInputGuard {
    fn arm(
        slot: &Rc<RefCell<Option<mpsc::UnboundedSender<PairingSubmission>>>>,
        sender: mpsc::UnboundedSender<PairingSubmission>,
    ) -> Self {
        *slot.borrow_mut() = Some(sender.clone());
        Self { slot: slot.clone(), mine: sender }
    }
}

impl Drop for PairingInputGuard {
    fn drop(&mut self) {
        let mut slot = self.slot.borrow_mut();
        // Only ever clear our own. A newer prompt may have replaced it
        // already, and taking theirs would leave them waiting for a code
        // that can no longer be delivered.
        if slot.as_ref().is_some_and(|current| current.same_channel(&self.mine)) {
            slot.take();
        }
    }
}

struct RemoteLspConnection {
    sender: SplitSink<WebSocketStream, Message>,
    /// Seals outbound frames; a passthrough with `--no-pairing`, which
    /// establishes no shared secret.
    sealing: session::Sealing,
    task: tokio::task::JoinHandle<()>,
    /// Set when this connection is being replaced, so the old handle's
    /// `Drop` skips its `Disconnected` emission and the UI doesn't see
    /// Disconnected racing Connected for the new peer.
    replaced: Arc<AtomicBool>,
}

impl RemoteLspConnection {
    /// Seal `bytes` into a frame for this connection. `None` only on counter
    /// exhaustion, which is logged here so every send path handles it alike.
    fn seal(&mut self, bytes: Vec<u8>) -> Option<Message> {
        match self.sealing.seal(bytes) {
            Ok(sealed) => Some(Message::binary(sealed)),
            Err(err) => {
                tracing::error!("Cannot seal outbound frame: {err:?}");
                None
            }
        }
    }
}

/// State shared between the connector and its spawned tasks.
/// Everything runs on the LSP's `LocalSet` thread.
#[derive(Clone)]
struct SharedState {
    connection: Arc<Mutex<Option<RemoteLspConnection>>>,
    preview_to_lsp_sender: RemotePreviewSender,
    /// Back-reference to the owning [`LspToPreviews`]. Used to forward
    /// `RemoteConnectionState` updates to the dialog. `Weak` so it can
    /// be stored inside the owner without forming an `Rc` cycle.
    to_previews: Weak<LspToPreviews>,
    /// Bumped on every user-driven connect or disconnect.
    /// Tasks spawned for an older generation stand down.
    generation: Rc<Cell<u64>>,
    /// Pairing tokens and their public ids, stored under every
    /// `address:port` the viewer that issued them was known by. Held in
    /// memory only, mirroring the viewer: it forgets them when it restarts,
    /// and re-pairing is one prompt.
    tokens: Rc<RefCell<HashMap<String, (TokenId, Token)>>>,
    /// Set while a viewer is showing a code and we're waiting for the user
    /// to type it. The preview UI's submission arrives through here.
    pairing_input: Rc<RefCell<Option<mpsc::UnboundedSender<PairingSubmission>>>>,
    /// `address:port` of the session currently installed, so a failed
    /// attempt elsewhere can put the dialog back to the truth instead of
    /// leaving it on whatever it was saying.
    connected_target: Rc<RefCell<Option<String>>>,
}

impl SharedState {
    fn bump_generation(&self) -> u64 {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        generation
    }

    /// The token held for any of the viewer's addresses.
    fn held_token(&self, keys: &[String]) -> Option<(TokenId, Token)> {
        let tokens = self.tokens.borrow();
        keys.iter().find_map(|key| tokens.get(key).copied())
    }

    /// Store a token under every address the viewer is known by, so a
    /// reconnect that lands on another one doesn't ask for a code again.
    fn remember_token(&self, keys: &[String], issued: (TokenId, Token)) {
        let mut tokens = self.tokens.borrow_mut();
        for key in keys {
            tokens.insert(key.clone(), issued);
        }
    }

    fn forget_token(&self, keys: &[String]) {
        let mut tokens = self.tokens.borrow_mut();
        for key in keys {
            tokens.remove(key);
        }
    }

    /// Forward a connection-state transition to the local preview dialog.
    fn emit_state(&self, state: RemoteConnectionState, target: String, error: Option<String>) {
        RemoteLspToPreview::emit_state(&self.to_previews, state, target, error);
    }
}

pub struct RemoteLspToPreview {
    shared: SharedState,
}

impl RemoteLspToPreview {
    pub fn new(
        preview_to_lsp_sender: mpsc::UnboundedSender<PreviewToLspMessage>,
        to_previews: Weak<LspToPreviews>,
    ) -> Self {
        Self {
            shared: SharedState {
                connection: Arc::default(),
                preview_to_lsp_sender: RemotePreviewSender(preview_to_lsp_sender),
                to_previews,
                generation: Rc::default(),
                tokens: Rc::default(),
                pairing_input: Rc::default(),
                connected_target: Rc::default(),
            },
        }
    }

    /// Hand the code the user typed to the pairing attempt waiting for it.
    /// Does nothing if no prompt is up, which is what happens when the code
    /// arrives after the viewer's deadline passed.
    pub fn submit_pairing_code(&self, code: String) {
        self.deliver_pairing(PairingSubmission::Code(code));
    }

    /// Abandon the pairing attempt the user was prompted for.
    pub fn cancel_pairing(&self) {
        self.deliver_pairing(PairingSubmission::Cancel);
    }

    /// Connect to the pairing-disabled viewer the user was warned about.
    pub fn accept_unpaired_connection(&self) {
        self.deliver_pairing(PairingSubmission::AcceptUnpaired);
    }

    fn deliver_pairing(&self, submission: PairingSubmission) {
        match self.shared.pairing_input.borrow().as_ref() {
            Some(sender) => {
                let _ = sender.send(submission);
            }
            None => tracing::debug!("Ignoring pairing input: no prompt is waiting for one"),
        }
    }

    /// Forward a connection-state transition to the local preview dialog.
    fn emit_state(
        to_previews: &Weak<LspToPreviews>,
        state: RemoteConnectionState,
        target: String,
        error: Option<String>,
    ) {
        if let Some(to_previews) = to_previews.upgrade() {
            to_previews.send_to_local_preview(&LspToPreviewMessage::RemoteConnectionState {
                state,
                target,
                error,
            });
        }
    }

    /// Serialize and send a wire-format message. Silently drops if no
    /// peer is connected, and logs (without panicking) on a serialization
    /// failure — this is called from the LSP's hot send path.
    pub fn send(&self, message: &LspToPreviewMessage) {
        tracing::debug!("Sending websocket message {message:?}");
        let connection = Arc::downgrade(&self.shared.connection);
        let message = match postcard::to_allocvec(message) {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::error!("Failed to serialize message for remote preview server: {err}");
                return;
            }
        };
        crate::editor_preview::spawn_local(async move {
            let Some(connection) = connection.upgrade() else {
                return;
            };
            let mut connection = connection.lock().await;
            let Some(connection) = connection.as_mut() else {
                return;
            };
            let Some(frame) = connection.seal(message) else { return };
            if let Err(err) = connection.sender.send(frame).await {
                tracing::error!("Error sending message to remote preview server: {err}");
            }
        });
    }

    pub fn connect<S: Into<String>>(
        &self,
        addresses: impl IntoIterator<Item = S>,
        port: u16,
    ) -> impl Future<Output = crate::editor_preview::Result<()>> + 'static {
        tracing::debug!("RemoteLspToPreview::connect");
        let shared = self.shared.clone();
        let addresses = addresses.into_iter().map(Into::into).collect::<Vec<_>>();
        async move {
            // First address identifies the connection in the state notifications.
            let Some(first_address) = addresses.first() else {
                return Err("No address to connect to".into());
            };
            let target = format!("{first_address}:{port}");
            let generation = shared.bump_generation();
            shared.emit_state(RemoteConnectionState::Connecting, target.clone(), None);
            if let Err(reason) = Self::connect_impl(&shared, &addresses, port, generation).await {
                // A superseded attempt no longer owns the dialog state.
                if shared.generation.get() == generation {
                    let still_connected = shared.connected_target.borrow().clone();
                    match still_connected {
                        // `Failed` would contradict the peer that is still
                        // routing and hide its Disconnect button, but saying
                        // nothing leaves the dialog stuck on `Connecting`
                        // with no way back. Report what is actually true.
                        Some(live) => {
                            tracing::warn!(
                                "Connect attempt to {target} failed but {live} is still connected: {reason}"
                            );
                            shared.emit_state(RemoteConnectionState::Connected, live, None);
                        }
                        None => shared.emit_state(
                            RemoteConnectionState::Failed,
                            target,
                            Some(reason.to_string()),
                        ),
                    }
                }
                return Err(reason.to_string().into());
            }
            Ok(())
        }
    }

    /// Dial `addresses` in order and install the resulting session.
    /// The callers own the dialog state updates.
    async fn connect_impl(
        shared: &SharedState,
        addresses: &[String],
        port: u16,
        generation: u64,
    ) -> std::result::Result<(), ConnectError> {
        let mut last_error: Option<String> = None;
        let mut connected = None;
        for address in addresses {
            tracing::info!("Attempting to connect to remote preview server at {address}:{port}");
            let url = format!("ws://{address}:{port}");
            let connect_future =
                tokio_tungstenite_wasm::connect_with_protocols(&url, &[PROTOCOL_SUBPROTOCOL]);
            match tokio::time::timeout(CONNECT_TIMEOUT, connect_future).await {
                Ok(Ok(stream)) => {
                    tracing::info!("Connected to remote preview server at {address}:{port}");
                    connected = Some((stream, address.clone()));
                    break;
                }
                Ok(Err(err)) => {
                    let mismatch = describe_version_mismatch(&err);
                    tracing::debug!(
                        "Failed connecting to remote viewer, trying next address: {err}"
                    );
                    if mismatch.is_some() {
                        last_error = mismatch;
                    } else if last_error.is_none() {
                        last_error = Some(format!("{err}"));
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
        let Some((mut stream, address)) = connected else {
            return Err(ConnectError::Transient(
                last_error.unwrap_or_else(|| "Unable to connect to remote viewer".into()),
            ));
        };

        if shared.generation.get() != generation {
            // The user disconnected or connected elsewhere while we dialed.
            tracing::info!("Discarding connection to {address}:{port}: superseded");
            return Err(ConnectError::Transient("Connection superseded".into()));
        }

        // Nothing is pushed to the viewer until it has admitted us.
        let target = format!("{address}:{port}");
        // A viewer usually advertises several addresses, and which one wins
        // the dial varies between attempts. Remember its token under all of
        // them, or a reconnect that lands elsewhere asks for a code again.
        let keys: Vec<String> = addresses.iter().map(|a| format!("{a}:{port}")).collect();
        let (sealing, opening) = Self::authenticate(shared, &mut stream, &target, &keys).await?;

        if shared.generation.get() != generation {
            tracing::info!("Discarding connection to {target}: superseded during pairing");
            return Err(ConnectError::Transient("Connection superseded".into()));
        }

        let (socket_sender, socket_receiver) = stream.split();
        let replaced = Arc::new(AtomicBool::new(false));
        #[allow(clippy::disallowed_methods)]
        let task = tokio::task::spawn_local(Self::run_session(
            shared.clone(),
            socket_receiver,
            addresses.to_vec(),
            address,
            port,
            replaced.clone(),
            generation,
            opening,
        ));
        if let Some(mut old) = shared.connection.lock().await.replace(RemoteLspConnection {
            sender: socket_sender,
            sealing,
            task,
            replaced,
        }) {
            tracing::info!("Closing previous connection to remote preview server");
            old.replaced.store(true, Ordering::Relaxed);
            // Close handshake so the old viewer sees a clean end of session
            // instead of a connection reset.
            old.sender.close().await.ok();
            old.task.abort();
        }

        *shared.connected_target.borrow_mut() = Some(target.clone());

        // Have the LSP push configuration, file contents, and the previewed
        // component, so the viewer leaves its idle screen on its own.
        shared
            .preview_to_lsp_sender
            .send(PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() });

        Ok(())
    }

    /// Complete the pairing exchange on a freshly connected socket.
    ///
    /// See [`i_slint_live_preview::protocol::pairing`] for the shape of it.
    /// A reconnect where we still hold the viewer's token runs the exchange
    /// with the token as the secret and never reaches the user.
    async fn authenticate(
        shared: &SharedState,
        stream: &mut WebSocketStream,
        target: &str,
        keys: &[String],
    ) -> std::result::Result<(session::Sealing, session::Opening), ConnectError> {
        // One deadline for the automatic steps, reset whenever the user
        // makes progress. A peer that dribbles ignorable frames can't push
        // it back; only a human legitimately does.
        let mut deadline = Self::handshake_deadline();

        let PreviewToLspMessage::PairingReady =
            Self::next_handshake_message(stream, deadline).await?
        else {
            return Err(ConnectError::Transient(
                "The viewer did not start the pairing handshake".into(),
            ));
        };

        let held = shared.held_token(keys);
        Self::send_message(
            stream,
            &LspToPreviewMessage::PairingHello { token: held.map(|(id, _)| id) },
        )
        .await?;

        loop {
            match Self::next_handshake_message(stream, deadline).await? {
                PreviewToLspMessage::PairingAccepted => {
                    // Only ever legitimate as "pairing is disabled on this
                    // viewer". A viewer that accepted our token proves so in
                    // the exchange instead, so nothing gets to skip it.
                    if held.is_some() {
                        return Err(ConnectError::Transient(
                            "The viewer skipped the reconnect exchange".into(),
                        ));
                    }
                    // Anyone can claim this, so the user is asked every time
                    // before anything is sent over what would be a plaintext
                    // session. Not remembered: the endpoint at an address can
                    // change between reconnects, and there is no key to bind
                    // consent to, so each unencrypted connection is its own
                    // decision.
                    Self::confirm_unpaired(shared, stream, target).await?;
                    tracing::info!("Viewer at {target} has pairing disabled");
                    return Ok((session::Sealing::Plaintext, session::Opening::Plaintext));
                }
                PreviewToLspMessage::PairingTokenChallenge { element } => {
                    let Some((_, token)) = held else {
                        return Err(ConnectError::Transient(
                            "The viewer opened an exchange for a token we never announced".into(),
                        ));
                    };
                    // `None` is a refused token: the viewer has told us to
                    // forget it and follows up with a code prompt, which the
                    // next turn of this loop takes. Shares the round's
                    // deadline, so a peer can't reset it by re-challenging.
                    if let Some(session) = Self::token_exchange(
                        shared, stream, target, keys, &token, &element, deadline,
                    )
                    .await?
                    {
                        return Ok(session);
                    }
                }
                PreviewToLspMessage::PairingConfirm { .. } => {
                    // Only meaningful as the answer to a response we sent;
                    // the exchanges handle it inline.
                    tracing::warn!("Unexpected pairing confirmation from {target}");
                }
                PreviewToLspMessage::PairingRejected { reason } => {
                    tracing::info!("Viewer at {target} rejected us: {reason}");
                    match reason {
                        // Issued by an earlier run of that viewer. Forget it;
                        // the code prompt follows on the same connection.
                        PairingRejection::BadToken => shared.forget_token(keys),
                        // The retry count comes with the prompt that follows.
                        PairingRejection::BadCode => {}
                        // Everything else ends this attempt, one way or another.
                        _ => return Err(from_rejection(reason)),
                    }
                }
                PreviewToLspMessage::PairingRequired {
                    attempts_left,
                    expires_in_seconds,
                    element,
                } => {
                    let prompt = Prompt { attempts_left, expires_in_seconds, element };
                    // `None` is a wrong code: the viewer follows up with a
                    // fresh prompt, which the next turn of this loop takes.
                    if let Some(session) =
                        Self::answer_prompt(shared, stream, target, keys, prompt).await?
                    {
                        return Ok(session);
                    }
                    // The user just typed a code, so the next automatic round
                    // starts from a clean clock.
                    deadline = Self::handshake_deadline();
                }
                other => {
                    tracing::warn!("Ignoring {other:?} from {target} during pairing");
                }
            }
        }
    }

    /// Answer one code prompt: ask the user, then run the exchange with
    /// what they typed.
    ///
    /// `Ok(None)` is a wrong code with attempts left; the viewer sends a
    /// fresh prompt and the caller goes around again.
    async fn answer_prompt(
        shared: &SharedState,
        stream: &mut WebSocketStream,
        target: &str,
        keys: &[String],
        prompt: Prompt,
    ) -> std::result::Result<Option<(session::Sealing, session::Opening)>, ConnectError> {
        let code = Self::prompt_for_code(
            shared,
            stream,
            target,
            prompt.attempts_left,
            prompt.expires_in_seconds,
        )
        .await?;

        // The user just typed, so the viewer's confirmation is a fresh
        // automatic round with its own clock.
        let handshake = pairing::Handshake::with_code(pairing::Role::Editor, &code);
        let verdict =
            Self::run_exchange(stream, handshake, &prompt.element, Self::handshake_deadline())
                .await?;
        match verdict {
            ExchangeVerdict::Confirmed(secrets) => {
                tracing::info!("Paired with remote viewer at {target}");
                shared.remember_token(keys, (secrets.token_id, secrets.token));
                Ok(Some(secrets.session()))
            }
            // A wrong code with attempts left: the fresh prompt follows.
            ExchangeVerdict::Rejected(PairingRejection::BadCode) => Ok(None),
            ExchangeVerdict::Rejected(reason) => Err(from_rejection(reason)),
        }
    }

    /// The reconnect exchange: like a code prompt, with the token as the
    /// secret and nobody asked.
    ///
    /// `Ok(None)` is a refused token; the caller forgets it and falls
    /// through to the code prompt the viewer sends next.
    async fn token_exchange(
        shared: &SharedState,
        stream: &mut WebSocketStream,
        target: &str,
        keys: &[String],
        token: &Token,
        viewer_element: &pairing::Element,
        deadline: tokio::time::Instant,
    ) -> std::result::Result<Option<(session::Sealing, session::Opening)>, ConnectError> {
        let handshake = pairing::Handshake::with_token(pairing::Role::Editor, token);
        match Self::run_exchange(stream, handshake, viewer_element, deadline).await? {
            ExchangeVerdict::Confirmed(secrets) => {
                tracing::info!("Reconnected to remote viewer at {target} by token");
                Ok(Some(secrets.session()))
            }
            // Refused after all: forget it, like a token the viewer never
            // knew, and take the code prompt that follows.
            ExchangeVerdict::Rejected(PairingRejection::BadToken) => {
                shared.forget_token(keys);
                Ok(None)
            }
            ExchangeVerdict::Rejected(reason) => Err(from_rejection(reason)),
        }
    }

    /// The editor's half of one SPAKE2 exchange, whatever the secret:
    /// answer the viewer's element, then require its proof of having
    /// derived the same key.
    ///
    /// The secret never goes on the wire, and neither does anything an
    /// observer could test a guess against: the exchange only reveals
    /// whether both sides agreed.
    async fn run_exchange(
        stream: &mut WebSocketStream,
        handshake: pairing::Handshake,
        viewer_element: &pairing::Element,
        deadline: tokio::time::Instant,
    ) -> std::result::Result<ExchangeVerdict, ConnectError> {
        let our_element = handshake.element().clone();
        let secrets = handshake
            .finish(viewer_element)
            .map_err(|_| ConnectError::Transient("The viewer sent a malformed handshake".into()))?;

        Self::send_message(
            stream,
            &LspToPreviewMessage::PairingResponse {
                element: our_element,
                confirmation: secrets.confirmation(),
            },
        )
        .await?;

        // The viewer answers with its own confirmation only if it derived
        // the same key, so this is where a wrong secret surfaces. Anything
        // else means it refused us.
        match Self::next_handshake_message(stream, deadline).await? {
            PreviewToLspMessage::PairingConfirm { confirmation }
                if secrets.peer_confirms(&confirmation) =>
            {
                Ok(ExchangeVerdict::Confirmed(secrets))
            }
            PreviewToLspMessage::PairingConfirm { .. } => {
                // Same secret space, different key: a mistyped code, or
                // someone in the middle answering for a token it can't know.
                Err(ConnectError::Transient("The viewer could not confirm the pairing".into()))
            }
            PreviewToLspMessage::PairingRejected { reason } => {
                Ok(ExchangeVerdict::Rejected(reason))
            }
            other => Err(ConnectError::Transient(format!(
                "The viewer sent an unexpected {other:?} during pairing"
            ))),
        }
    }

    /// Put the code prompt in front of the user and wait for them.
    ///
    /// Waits on the socket at the same time: the viewer owns the deadline,
    /// and when it passes it closes the connection. Noticing that beats
    /// leaving a prompt up for a code that can no longer be used.
    async fn prompt_for_code(
        shared: &SharedState,
        stream: &mut WebSocketStream,
        target: &str,
        attempts_left: u8,
        expires_in_seconds: u16,
    ) -> std::result::Result<String, ConnectError> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let _guard = PairingInputGuard::arm(&shared.pairing_input, sender);

        let hint = if attempts_left < MAX_ATTEMPTS {
            format!(
                "Incorrect code. {attempts_left} attempts left, {expires_in_seconds}s remaining"
            )
        } else {
            format!("Enter the code shown on the viewer within {expires_in_seconds}s")
        };
        shared.emit_state(RemoteConnectionState::PairingRequired, target.to_owned(), Some(hint));

        let submission = tokio::select! {
            submission = receiver.recv() => submission,
            message = Self::next_message(stream) => {
                return Err(match message {
                    Some(PreviewToLspMessage::PairingRejected { reason }) => from_rejection(reason),
                    _ => ConnectError::Transient(
                        "The viewer closed the connection while waiting for the code".into(),
                    ),
                });
            }
        };

        match submission {
            Some(PairingSubmission::Code(code)) => Ok(code),
            // Cancelling is the user's decision, so don't reconnect behind
            // their back. An unpaired-acceptance is no answer to a code
            // prompt, so it is taken the conservative way.
            Some(PairingSubmission::Cancel | PairingSubmission::AcceptUnpaired) | None => {
                Err(ConnectError::Fatal("Pairing cancelled".into()))
            }
        }
    }

    /// Warn the user that the viewer has pairing disabled, and wait for
    /// them to accept the unencrypted connection or cancel.
    ///
    /// Waits on the socket at the same time, like the code prompt: the
    /// viewer hanging up takes the question off the screen.
    async fn confirm_unpaired(
        shared: &SharedState,
        stream: &mut WebSocketStream,
        target: &str,
    ) -> std::result::Result<(), ConnectError> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let _guard = PairingInputGuard::arm(&shared.pairing_input, sender);

        // The dialog owns the wording; this only says which question is up.
        shared.emit_state(RemoteConnectionState::UnpairedWarning, target.to_owned(), None);

        loop {
            tokio::select! {
                submission = receiver.recv() => {
                    return match submission {
                        Some(PairingSubmission::AcceptUnpaired) => {
                            tracing::info!("User accepted the unencrypted connection to {target}");
                            Ok(())
                        }
                        // Only a decision answers a warning; a stray code
                        // counts as not making one.
                        Some(PairingSubmission::Code(_) | PairingSubmission::Cancel) | None => {
                            Err(ConnectError::Fatal("Connection cancelled".into()))
                        }
                    };
                }
                message = Self::next_message(stream) => {
                    let Some(message) = message else {
                        return Err(ConnectError::Transient(
                            "The viewer closed the connection while waiting for the user".into(),
                        ));
                    };
                    // The session on the viewer's side is already running,
                    // so traffic may arrive; none of it is for us yet.
                    tracing::debug!("Ignoring {message:?} while the user decides");
                }
            }
        }
    }

    async fn send_message(
        stream: &mut WebSocketStream,
        message: &LspToPreviewMessage,
    ) -> std::result::Result<(), ConnectError> {
        let bytes = postcard::to_allocvec(message).map_err(|err| {
            ConnectError::Transient(format!("Failed encoding {message:?}: {err}"))
        })?;
        stream
            .send(Message::binary(bytes))
            .await
            .map_err(|err| ConnectError::Transient(format!("Failed sending to the viewer: {err}")))
    }

    /// The next handshake message, or a transient error once `deadline`
    /// passes. Every automatic step reads through this against a shared
    /// per-round deadline, so a dropped connection or a peer that stops
    /// answering -- or dribbles ignorable frames -- fails the connect
    /// instead of hanging it forever. The waits a human drives use
    /// [`Self::next_message`] directly: there the user's Cancel and the
    /// viewer's own deadline bound the wait.
    async fn next_handshake_message(
        stream: &mut WebSocketStream,
        deadline: tokio::time::Instant,
    ) -> std::result::Result<PreviewToLspMessage, ConnectError> {
        match tokio::time::timeout_at(deadline, Self::next_message(stream)).await {
            Ok(Some(message)) => Ok(message),
            Ok(None) => Err(ConnectError::Transient(
                "The viewer closed the connection during pairing".into(),
            )),
            Err(_) => {
                Err(ConnectError::Transient("The viewer stopped responding during pairing".into()))
            }
        }
    }

    /// A fresh deadline for the next round of automatic steps.
    fn handshake_deadline() -> tokio::time::Instant {
        tokio::time::Instant::now() + HANDSHAKE_TIMEOUT
    }

    /// Next protocol message, or `None` if the socket ended or carried
    /// something undecodable.
    async fn next_message(stream: &mut WebSocketStream) -> Option<PreviewToLspMessage> {
        loop {
            match stream.next().await? {
                Ok(Message::Binary(bytes)) => match postcard::from_bytes(&bytes) {
                    Ok(message) => return Some(message),
                    Err(err) => {
                        tracing::error!("Failed decoding message from remote viewer: {err}");
                        return None;
                    }
                },
                Ok(Message::Text(text)) => {
                    tracing::warn!("Ignoring text message from remote viewer: {text}");
                }
                Ok(Message::Close(_)) | Err(_) => return None,
            }
        }
    }

    /// Drive one established connection.
    /// When the session ends on its own (peer closed, socket error, missing
    /// pongs), reconnect; user-driven disconnects and replacements abort
    /// this task instead.
    async fn run_session(
        shared: SharedState,
        socket_receiver: SplitStream<WebSocketStream>,
        addresses: Vec<String>,
        connected_address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
        generation: u64,
        opening: session::Opening,
    ) {
        let last_pong = Cell::new(Instant::now());
        let receive = Self::receive_task(
            &shared,
            socket_receiver,
            connected_address,
            port,
            replaced,
            &last_pong,
            opening,
        );
        let keepalive = Self::keepalive_task(&shared, &last_pong);
        tokio::select! {
            _ = receive => {}
            _ = keepalive => {}
        }
        Self::reconnect_loop(&shared, &addresses, port, generation).await;
    }

    /// Ping the viewer every [`PING_INTERVAL`] and return — ending the
    /// session — when pongs stay out for [`PONG_TIMEOUT`] or sending fails.
    async fn keepalive_task(shared: &SharedState, last_pong: &Cell<Instant>) {
        let Ok(ping) = postcard::to_allocvec(&LspToPreviewMessage::Ping) else { return };
        loop {
            tokio::time::sleep(PING_INTERVAL).await;
            if last_pong.get().elapsed() > PONG_TIMEOUT {
                tracing::warn!(
                    "Remote viewer answered no ping for {PONG_TIMEOUT:?}; treating the connection as dead"
                );
                return;
            }
            let mut guard = shared.connection.lock().await;
            let Some(connection) = guard.as_mut() else { return };
            // Sealed like any other payload; a fresh nonce each time, so the
            // repeated plaintext doesn't repeat on the wire.
            let Some(frame) = connection.seal(ping.clone()) else { return };
            // Bound the send: it holds the connection lock, which would
            // otherwise block all LSP→viewer sends behind a stalled socket.
            match tokio::time::timeout(PONG_TIMEOUT, connection.sender.send(frame)).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    tracing::warn!("Failed sending keepalive ping to remote viewer: {err}");
                    return;
                }
                Err(_) => {
                    tracing::warn!("Keepalive ping send stalled; treating the connection as dead");
                    return;
                }
            }
        }
    }

    /// Redial a dropped connection every [`RECONNECT_DELAY`] until it
    /// succeeds or a generation bump tells us to stand down.
    async fn reconnect_loop(
        shared: &SharedState,
        addresses: &[String],
        port: u16,
        generation: u64,
    ) {
        if shared.generation.get() != generation {
            return;
        }
        // Drop the dead connection's write half; its task is this very task.
        drop(shared.connection.lock().await.take());
        shared.connected_target.borrow_mut().take();
        let target =
            format!("{}:{port}", addresses.first().map(String::as_str).unwrap_or_default());
        tracing::info!("Connection to remote viewer lost; reconnecting to {target}");
        shared.emit_state(RemoteConnectionState::Connecting, target.clone(), None);
        loop {
            match Self::connect_impl(shared, addresses, port, generation).await {
                Ok(()) => {
                    tracing::info!("Reconnected to remote viewer at {target}");
                    return;
                }
                Err(ConnectError::Fatal(reason)) => {
                    // Retrying would put the same prompt back up, or spin on a
                    // refusal. Hand it back to the user instead.
                    tracing::warn!("Giving up reconnecting to {target}: {reason}");
                    shared.emit_state(RemoteConnectionState::Failed, target, Some(reason));
                    return;
                }
                Err(ConnectError::Transient(err)) => {
                    tracing::debug!("Reconnect attempt to {target} failed: {err}");
                }
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
            if shared.generation.get() != generation {
                return;
            }
        }
    }

    async fn receive_task(
        shared: &SharedState,
        mut socket_receiver: SplitStream<WebSocketStream>,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
        last_pong: &Cell<Instant>,
        mut opening: session::Opening,
    ) {
        let mut connection_state_handle =
            ConnectionStateHandle::new(shared.to_previews.clone(), address, port, replaced);
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
                            let Ok(plain) = opening.open(&bytes) else {
                                // Tampered, reordered, or from a peer that
                                // derived a different key. The counters
                                // can't recover, so the session is over.
                                tracing::error!(
                                    "Dropping a frame from the remote viewer that failed to open"
                                );
                                return;
                            };
                            match postcard::from_bytes::<PreviewToLspMessage>(&plain) {
                                Ok(PreviewToLspMessage::Pong) => {
                                    last_pong.set(Instant::now());
                                }
                                Ok(msg) => {
                                    shared.preview_to_lsp_sender.send(msg);
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed decoding message from remote preview server: {e}"
                                    );
                                }
                            }
                        }
                        Message::Close(_) => {
                            return;
                        }
                    }
                }
                Err(tokio_tungstenite_wasm::Error::ConnectionClosed)
                | Err(tokio_tungstenite_wasm::Error::AlreadyClosed) => {
                    return;
                }
                Err(tokio_tungstenite_wasm::Error::Io(err))
                    if err.kind() != std::io::ErrorKind::WouldBlock =>
                {
                    tracing::error!("I/O error in WebSocket connection: {err}");
                    connection_state_handle.error = Some(format!("I/O error: {err}"));
                    return;
                }
                Err(tokio_tungstenite_wasm::Error::Protocol(
                    tokio_tungstenite_wasm::error::ProtocolError::ResetWithoutClosingHandshake,
                )) => {
                    // The viewer vanished without a close handshake (app killed,
                    // network drop) — a normal way for a session to end.
                    tracing::info!("Connection to remote viewer lost");
                    return;
                }
                Err(err) => {
                    tracing::error!("WebSocket error: {err}");
                }
            }
        }
    }

    pub fn disconnect(&self) -> impl Future<Output = ()> + 'static {
        let shared = self.shared.clone();
        async move {
            shared.bump_generation();
            shared.connected_target.borrow_mut().take();
            if let Some(mut connection) = shared.connection.lock().await.take() {
                // Close handshake so the viewer sees a clean end of session
                // instead of a connection reset.
                connection.sender.close().await.ok();
                connection.task.abort();
            }
        }
    }
}

/// The LSP's message channel, which the trusted local preview process also
/// feeds, seen from the remote connection. Everything a remote viewer sends
/// goes through here, so the check can't be forgotten at a call site.
#[derive(Clone)]
struct RemotePreviewSender(mpsc::UnboundedSender<PreviewToLspMessage>);

impl RemotePreviewSender {
    /// A remote viewer runs on someone else's machine: it may report what it
    /// compiled and ask for the files it needs, and nothing else. That, not
    /// "does it reach the editor", is where the line is — a viewer's whole job
    /// is to send back diagnostics.
    ///
    /// Listed positively on purpose: a new variant stays refused until it is
    /// deliberately allowed.
    ///
    /// Refused messages are dropped rather than taken as grounds to hang up:
    /// the reconnect loop would only dial it again, and by then
    /// the peer already has everything the connection was going to give it.
    fn send(&self, message: PreviewToLspMessage) {
        if !matches!(
            message,
            PreviewToLspMessage::Diagnostics { .. }
                | PreviewToLspMessage::DebugMessage { .. }
                | PreviewToLspMessage::RequestState { .. }
        ) {
            tracing::warn!(
                "Ignoring message that a remote preview server may not send: {message:?}"
            );
            return;
        }
        self.0.send(message).unwrap_or_else(|err| {
            tracing::error!(
                "Failed sending message from remote preview server to LSP server: {err}"
            );
        });
    }
}

// The forwards below name the inherent methods explicitly: were one of them
// removed, an unqualified call would bind to the trait method instead and
// recurse forever.
impl crate::editor_preview::RemoteTransport for RemoteLspToPreview {
    fn send(&self, message: &LspToPreviewMessage) {
        RemoteLspToPreview::send(self, message);
    }

    fn connect(
        &self,
        addresses: Vec<String>,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = crate::editor_preview::Result<()>>>> {
        Box::pin(RemoteLspToPreview::connect(self, addresses, port))
    }

    fn disconnect(&self) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(RemoteLspToPreview::disconnect(self))
    }

    fn submit_pairing_code(&self, code: String) {
        RemoteLspToPreview::submit_pairing_code(self, code);
    }

    fn cancel_pairing(&self) {
        RemoteLspToPreview::cancel_pairing(self);
    }

    fn accept_unpaired_connection(&self) {
        RemoteLspToPreview::accept_unpaired_connection(self);
    }
}

struct ConnectionStateHandle {
    to_previews: Weak<LspToPreviews>,
    error: Option<String>,
    address: String,
    port: u16,
    replaced: Arc<AtomicBool>,
}

impl ConnectionStateHandle {
    fn new(
        to_previews: Weak<LspToPreviews>,
        address: String,
        port: u16,
        replaced: Arc<AtomicBool>,
    ) -> Self {
        RemoteLspToPreview::emit_state(
            &to_previews,
            RemoteConnectionState::Connected,
            format!("{address}:{port}"),
            None,
        );
        Self { to_previews, error: None, address, port, replaced }
    }
}

impl Drop for ConnectionStateHandle {
    fn drop(&mut self) {
        if self.replaced.load(Ordering::Relaxed) {
            return;
        }
        RemoteLspToPreview::emit_state(
            &self.to_previews,
            RemoteConnectionState::Disconnected,
            format!("{}:{}", self.address, self.port),
            self.error.take(),
        );
    }
}

impl Drop for RemoteLspToPreview {
    fn drop(&mut self) {
        // Stop any reconnect loop that is between attempts.
        self.shared.bump_generation();
        // Best-effort: an in-flight future may hold the lock, in which case
        // LocalSet teardown aborts the receive task. Panicking here would
        // abort the LSP.
        if let Some(mut guard) = self.shared.connection.try_lock()
            && let Some(connection) = guard.take()
        {
            tracing::info!("Closing connection to remote preview server");
            connection.task.abort();
        }
    }
}

/// Human-readable explanation when the handshake was rejected for a Slint
/// version mismatch. The viewer sends `Slint-Version` / `Slint-Protocols`
/// headers; the browser hides them from WASM so we fall back to a generic
/// message there.
fn describe_version_mismatch(err: &tokio_tungstenite_wasm::Error) -> Option<String> {
    match err {
        tokio_tungstenite_wasm::Error::Http(response) => {
            let headers = response.headers();
            let viewer_version = headers
                .get(SLINT_VERSION_HEADER)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("an unknown version");
            let viewer_protocols =
                headers.get(SLINT_PROTOCOLS_HEADER).and_then(|v| v.to_str().ok());
            if headers.contains_key(SLINT_VERSION_HEADER) {
                Some(format!(
                    "Version mismatch: viewer runs Slint {viewer_version} (protocol {}), LSP speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})",
                    viewer_protocols.unwrap_or("unknown"),
                ))
            } else {
                None
            }
        }
        tokio_tungstenite_wasm::Error::Protocol(
            tokio_tungstenite_wasm::error::ProtocolError::SecWebSocketSubProtocolError(_),
        ) => Some(format!(
            "Version mismatch: viewer does not speak {PROTOCOL_SUBPROTOCOL} (this LSP is Slint {SLINT_VERSION})",
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_live_preview::protocol::PreviewComponent;
    use i_slint_live_preview::remote::{Connection, ConnectionMessage, PairingPolicy};
    use lsp_types::Url;

    async fn listen(
        port: u16,
        policy: PairingPolicy,
    ) -> (Connection, mpsc::UnboundedReceiver<ConnectionMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let connection = Connection::listen(
            Some(std::net::SocketAddr::from(([127, 0, 0, 1], port))),
            None,
            policy,
            move |msg| {
                let _ = tx.send(msg);
            },
        )
        .await
        .unwrap();
        (connection, rx)
    }

    /// The code from the viewer's next `PairingStarted`.
    async fn expect_code(rx: &mut mpsc::UnboundedReceiver<ConnectionMessage>) -> String {
        let event = expect_message(
            rx,
            |m| matches!(m, ConnectionMessage::PairingStarted { .. }),
            "the viewer to show a pairing code",
        )
        .await;
        let ConnectionMessage::PairingStarted { code, .. } = event else { unreachable!() };
        code
    }

    /// Spin until the connector is (or is no longer) waiting for a code.
    ///
    /// Everything runs on one thread, so the connect future only moves at its
    /// await points; yielding is what lets it get there.
    async fn wait_for_prompt(connector: &RemoteLspToPreview) {
        for _ in 0..2000 {
            if connector.shared.pairing_input.borrow().is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("connector never started waiting for a pairing code");
    }

    /// Wait until `rx` yields a message matching `pred`, and return it.
    async fn expect_message<T>(
        rx: &mut mpsc::UnboundedReceiver<T>,
        pred: impl Fn(&T) -> bool,
        what: &str,
    ) -> T {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let msg = rx.recv().await.expect("message channel closed");
                if pred(&msg) {
                    return msg;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
    }

    /// Connect and answer the code prompt, the way a user would.
    async fn pair(
        connector: &RemoteLspToPreview,
        viewer_rx: &mut mpsc::UnboundedReceiver<ConnectionMessage>,
        port: u16,
    ) -> crate::editor_preview::Result<()> {
        let connect = connector.connect(["127.0.0.1"], port);
        let enter_code = async {
            let code = expect_code(viewer_rx).await;
            wait_for_prompt(connector).await;
            connector.submit_pairing_code(code);
        };
        let (result, ()) = tokio::join!(connect, enter_code);
        result
    }

    /// A viewer on an arbitrary port with a connector attached to it, past the
    /// handshake and the initial state push both sides start a session with.
    async fn connected_viewer() -> (
        Connection,
        mpsc::UnboundedReceiver<ConnectionMessage>,
        RemoteLspToPreview,
        mpsc::UnboundedReceiver<PreviewToLspMessage>,
    ) {
        let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Disabled).await;

        let (to_lsp_tx, mut to_lsp_rx) = mpsc::unbounded_channel();
        let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());
        // A pairing-disabled viewer needs the user's ok before the first
        // connection goes through.
        let connect = connector.connect(["127.0.0.1"], viewer.local_port());
        let accept = async {
            wait_for_prompt(&connector).await;
            connector.accept_unpaired_connection();
        };
        let (result, ()) = tokio::join!(connect, accept);
        result.unwrap();
        expect_message(
            &mut viewer_rx,
            |m| matches!(m, ConnectionMessage::Connected { .. }),
            "viewer connection",
        )
        .await;
        expect_message(
            &mut to_lsp_rx,
            |m| matches!(m, PreviewToLspMessage::RequestState { .. }),
            "RequestState after connecting",
        )
        .await;

        (viewer, viewer_rx, connector, to_lsp_rx)
    }

    /// Two attempts can overlap: the device list stays clickable while a
    /// prompt is up. The older one standing down must not disarm the newer.
    #[test]
    fn a_finished_prompt_only_clears_its_own_input_slot() {
        let slot: Rc<RefCell<Option<mpsc::UnboundedSender<PairingSubmission>>>> = Rc::default();

        let (first, _first_rx) = mpsc::unbounded_channel();
        let first_guard = PairingInputGuard::arm(&slot, first);
        let (second, mut second_rx) = mpsc::unbounded_channel();
        let _second_guard = PairingInputGuard::arm(&slot, second);

        drop(first_guard);

        let armed = slot.borrow().clone().expect("the newer prompt was disarmed");
        armed.send(PairingSubmission::Code("4321".into())).expect("channel closed");
        assert!(
            matches!(second_rx.try_recv(), Ok(PairingSubmission::Code(code)) if code == "4321")
        );
    }

    #[tokio::test]
    async fn reconnects_after_connection_loss() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, viewer_rx, connector, mut to_lsp_rx) = connected_viewer().await;
                let port = viewer.local_port();

                // Replace the viewer on the same port, like an app whose
                // connection the OS cut while backgrounded.
                drop(viewer);
                drop(viewer_rx);
                let (_viewer, mut viewer_rx) = listen(port, PairingPolicy::Disabled).await;

                // The connector reconnects on its own, but an unpaired viewer
                // is never silently trusted, so the reconnect warns again and
                // the user accepts.
                let accept = async {
                    wait_for_prompt(&connector).await;
                    connector.accept_unpaired_connection();
                };
                let observe = async {
                    expect_message(
                        &mut viewer_rx,
                        |m| matches!(m, ConnectionMessage::Connected { .. }),
                        "viewer reconnection",
                    )
                    .await;
                    // ... and asks the LSP to re-push the preview state.
                    expect_message(
                        &mut to_lsp_rx,
                        |m| matches!(m, PreviewToLspMessage::RequestState { .. }),
                        "RequestState after reconnecting",
                    )
                    .await;
                };
                tokio::join!(accept, observe);

                connector.disconnect().await;
            })
            .await;
    }

    /// A viewer is on the far end of the network: it may not drive the editor.
    #[tokio::test]
    async fn a_code_read_off_the_viewer_completes_the_connection() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Generated).await;
                let port = viewer.local_port();

                let (to_lsp_tx, mut to_lsp_rx) = mpsc::unbounded_channel();
                let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());

                let result = pair(&connector, &mut viewer_rx, port).await;
                result.expect("pairing should have succeeded");

                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::Connected { .. }),
                    "viewer connection",
                )
                .await;
                expect_message(
                    &mut to_lsp_rx,
                    |m| matches!(m, PreviewToLspMessage::RequestState { .. }),
                    "RequestState after pairing",
                )
                .await;

                connector.disconnect().await;
            })
            .await;
    }

    /// A code exchange leaves both ends holding session keys, so from there on
    /// every frame has to survive the cipher. The `PairingPolicy::Disabled`
    /// tests can't cover this: no keys, no sealing, so a missing seal or open
    /// passes them.
    #[tokio::test]
    async fn traffic_over_a_paired_session_is_sealed_both_ways() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Generated).await;
                let port = viewer.local_port();

                let (to_lsp_tx, mut to_lsp_rx) = mpsc::unbounded_channel();
                let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());
                pair(&connector, &mut viewer_rx, port).await.expect("pairing failed");

                // The connector pushes this one locally on connect. Take it
                // out of the way so the one asserted below can only have come
                // off the wire.
                expect_message(
                    &mut to_lsp_rx,
                    |m| matches!(m, PreviewToLspMessage::RequestState { .. }),
                    "the local RequestState after pairing",
                )
                .await;

                let url = Url::parse("file:///sealed.slint").unwrap();
                connector.send(&LspToPreviewMessage::ShowPreview(PreviewComponent {
                    url: url.clone(),
                    component: None,
                }));

                // The viewer opened the sealed frame ...
                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::ShowPreview { .. }),
                    "ShowPreview to arrive at the viewer",
                )
                .await;
                // ... and its own request comes back the other way, which
                // the connector only sees if it opens the answer.
                // Nobody answers the request, so polling it is only how the
                // send happens.
                let asked = tokio::select! {
                    _ = viewer.request_file(url.clone()) => unreachable!("nobody sent the file"),
                    msg = expect_message(
                        &mut to_lsp_rx,
                        |m| matches!(m, PreviewToLspMessage::RequestState { files, .. } if !files.is_empty()),
                        "the viewer's request for a file",
                    ) => msg,
                };
                let PreviewToLspMessage::RequestState { files, .. } = asked else { unreachable!() };
                assert_eq!(files, vec![url]);

                connector.disconnect().await;
            })
            .await;
    }

    /// A second connect to a viewer we already paired with rides the token:
    /// the reconnect exchange runs on its own, no code reaches any screen,
    /// and the replacing session is sealed with fresh keys.
    #[tokio::test]
    async fn a_second_connect_needs_no_code() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Generated).await;
                let port = viewer.local_port();

                let (to_lsp_tx, mut to_lsp_rx) = mpsc::unbounded_channel();
                let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());
                pair(&connector, &mut viewer_rx, port).await.expect("pairing failed");
                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::Connected { .. }),
                    "the first connection",
                )
                .await;

                // Nobody is around to type a code: a reconnect that put up
                // a prompt would hang here until the timeout fails the test.
                tokio::time::timeout(
                    Duration::from_secs(15),
                    connector.connect(["127.0.0.1"], port),
                )
                .await
                .expect("the reconnect waited for a code")
                .expect("the token reconnect failed");

                // The viewer admitted it without touching the screen ...
                let event = expect_message(
                    &mut viewer_rx,
                    |m| {
                        matches!(
                            m,
                            ConnectionMessage::Connected { .. }
                                | ConnectionMessage::PairingStarted { .. }
                        )
                    },
                    "the second connection",
                )
                .await;
                assert!(
                    matches!(event, ConnectionMessage::Connected { .. }),
                    "the reconnect put a code on the screen"
                );

                // ... and both ends agree on the fresh keys: a frame crosses
                // the new session in each direction.
                connector.send(&LspToPreviewMessage::ShowPreview(PreviewComponent {
                    url: Url::parse("file:///fresh.slint").unwrap(),
                    component: None,
                }));
                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::ShowPreview { .. }),
                    "ShowPreview to arrive over the new session",
                )
                .await;
                viewer
                    .send(PreviewToLspMessage::Diagnostics {
                        uri: Url::parse("file:///fresh.slint").unwrap(),
                        version: None,
                        diagnostics: Vec::new(),
                    })
                    .unwrap();
                expect_message(
                    &mut to_lsp_rx,
                    |m| matches!(m, PreviewToLspMessage::Diagnostics { .. }),
                    "diagnostics to arrive over the new session",
                )
                .await;

                connector.disconnect().await;
            })
            .await;
    }

    #[tokio::test]
    async fn a_wrong_code_can_be_followed_by_the_right_one() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Generated).await;
                let port = viewer.local_port();

                let (to_lsp_tx, _to_lsp_rx) = mpsc::unbounded_channel();
                let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());

                // One attempt throughout: a second `connect` would be refused
                // by the viewer's rate limit on prompts.
                let connect = connector.connect(["127.0.0.1"], port);
                tokio::pin!(connect);
                let mut code: Option<String> = None;
                let mut wrong_code_sent = false;

                let result = tokio::time::timeout(Duration::from_secs(20), async {
                    loop {
                        tokio::select! {
                            result = &mut connect => break result,
                            event = viewer_rx.recv() => {
                                if let Some(ConnectionMessage::PairingStarted { code: shown, .. }) = event {
                                    code = Some(shown);
                                }
                            }
                            _ = tokio::time::sleep(Duration::from_millis(5)) => {
                                let Some(code) = code.clone() else { continue };
                                if connector.shared.pairing_input.borrow().is_none() {
                                    continue;
                                }
                                if wrong_code_sent {
                                    // Offered until a prompt takes it: the moment
                                    // the viewer re-arms isn't observable from here.
                                    connector.submit_pairing_code(code);
                                } else {
                                    let wrong = if code == "0000" { "1111" } else { "0000" };
                                    connector.submit_pairing_code(wrong.to_owned());
                                    wrong_code_sent = true;
                                }
                            }
                        }
                    }
                })
                .await
                .expect("timed out pairing");

                assert!(wrong_code_sent, "the wrong code was never offered");
                result.expect("the second code should have been accepted");

                connector.disconnect().await;
            })
            .await;
    }

    /// mDNS commonly offers a viewer under several addresses, and which one
    /// wins the dial varies. Landing on a different one must not cost the
    /// user another trip to the device.
    #[tokio::test]
    async fn a_token_survives_reconnecting_on_another_address() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Generated).await;
                let port = viewer.local_port();

                let (to_lsp_tx, _to_lsp_rx) = mpsc::unbounded_channel();
                let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());

                // Paired having dialled the first of two addresses.
                let connect = connector.connect(["127.0.0.1", "localhost"], port);
                let enter_code = async {
                    let code = expect_code(&mut viewer_rx).await;
                    wait_for_prompt(&connector).await;
                    connector.submit_pairing_code(code);
                };
                let (result, ()) = tokio::join!(connect, enter_code);
                result.expect("pairing should have succeeded");

                // The other address is the one that works next time.
                let tokens = connector.shared.tokens.borrow();
                assert_eq!(
                    tokens.get(&format!("127.0.0.1:{port}")),
                    tokens.get(&format!("localhost:{port}")),
                    "the token was only remembered for the address that happened to answer"
                );
                assert!(tokens.get(&format!("localhost:{port}")).is_some());
            })
            .await;
    }

    #[tokio::test]
    async fn a_restarted_viewer_asks_for_a_code_again() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Generated).await;
                let port = viewer.local_port();

                let (to_lsp_tx, _to_lsp_rx) = mpsc::unbounded_channel();
                let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());

                let result = pair(&connector, &mut viewer_rx, port).await;
                result.expect("pairing should have succeeded");
                let target = format!("127.0.0.1:{port}");
                assert!(
                    connector.shared.tokens.borrow().contains_key(&target),
                    "a successful pairing should leave a token behind"
                );

                // The viewer restarts: it has forgotten every token it issued,
                // so the one we hold is no longer worth anything.
                drop(viewer);
                drop(viewer_rx);
                let (_viewer, mut viewer_rx) = listen(port, PairingPolicy::Generated).await;

                // The reconnect offers the stale token, is turned away, and
                // ends up back at a code prompt.
                let code = expect_code(&mut viewer_rx).await;
                wait_for_prompt(&connector).await;
                // Reaching a code prompt means the rejection has been handled.
                assert!(
                    !connector.shared.tokens.borrow().contains_key(&target),
                    "a rejected token should have been forgotten"
                );
                connector.submit_pairing_code(code);

                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::Connected { .. }),
                    "viewer reconnection after re-pairing",
                )
                .await;

                connector.disconnect().await;
            })
            .await;
    }

    /// A local preview that records the connection-state updates the
    /// connector pushes to the dialog, which is the only place the client's
    /// own view of pairing is observable.
    struct RecordingPreview(mpsc::UnboundedSender<(RemoteConnectionState, Option<String>)>);

    impl i_slint_live_preview::protocol::LspToPreview for RecordingPreview {
        fn send(&self, message: &LspToPreviewMessage) {
            if let LspToPreviewMessage::RemoteConnectionState { state, error, .. } = message {
                let _ = self.0.send((*state, error.clone()));
            }
        }
        fn preview_target(&self) -> i_slint_live_preview::protocol::PreviewTarget {
            i_slint_live_preview::protocol::PreviewTarget::Dummy
        }
    }

    /// Wait for the connector to push `wanted` to the dialog, returning the
    /// error text that came with it.
    async fn expect_state(
        rx: &mut mpsc::UnboundedReceiver<(RemoteConnectionState, Option<String>)>,
        wanted: RemoteConnectionState,
    ) -> Option<String> {
        let what = format!("the connector to report {wanted:?}");
        expect_message(rx, |(state, _)| *state == wanted, &what).await.1
    }

    fn connector_with_states() -> (
        std::rc::Rc<LspToPreviews>,
        Rc<RemoteLspToPreview>,
        mpsc::UnboundedReceiver<(RemoteConnectionState, Option<String>)>,
        mpsc::UnboundedReceiver<PreviewToLspMessage>,
    ) {
        let (state_tx, state_rx) = mpsc::unbounded_channel();
        let (to_lsp_tx, to_lsp_rx) = mpsc::unbounded_channel();
        let locals = std::iter::once((
            i_slint_live_preview::protocol::PreviewTarget::Dummy,
            Box::new(RecordingPreview(state_tx))
                as Box<dyn i_slint_live_preview::protocol::LspToPreview>,
        ))
        .collect();
        // The transport the previews own is the one the tests drive, so
        // hold on to it as the concrete type on the way through.
        let mut connector = None;
        let previews = LspToPreviews::new(
            locals,
            i_slint_live_preview::protocol::PreviewTarget::Dummy,
            |to_previews| {
                let remote = Rc::new(RemoteLspToPreview::new(to_lsp_tx, to_previews));
                connector = Some(remote.clone());
                remote as Rc<dyn crate::editor_preview::RemoteTransport>
            },
        )
        .unwrap();
        (previews, connector.unwrap(), state_rx, to_lsp_rx)
    }

    /// Running out of attempts has to end the reconnect loop.
    ///
    /// Reproduces what a user hits after restarting the viewer: the stale
    /// token is refused, they fumble the code, and the loop must hand back
    /// to them rather than keep re-prompting the device.
    #[tokio::test]
    async fn running_out_of_attempts_stops_reconnecting() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Generated).await;
                let port = viewer.local_port();

                let (_previews, connector, mut state_rx, _to_lsp_rx) = connector_with_states();

                // Pair once, the way a user would.
                let result = pair(&connector, &mut viewer_rx, port).await;
                result.expect("first pairing should have succeeded");

                // Everything above is setup. Drop its states, or the burn
                // loop below matches the prompt we already answered.
                while state_rx.try_recv().is_ok() {}

                // The viewer restarts, so the token we hold is worthless and
                // the reconnect lands on a fresh code prompt.
                drop(viewer);
                drop(viewer_rx);
                let (_viewer, mut viewer_rx) = listen(port, PairingPolicy::Generated).await;
                let code = expect_code(&mut viewer_rx).await;
                let wrong = if code == "0000" { "1111" } else { "0000" };

                // Burn every attempt. `pairing_input` is armed before the
                // state goes out, so a prompt seen here is ready for a code.
                for _ in 0..MAX_ATTEMPTS {
                    expect_state(&mut state_rx, RemoteConnectionState::PairingRequired).await;
                    connector.submit_pairing_code(wrong.to_owned());
                }

                // The dialog has to be told, so the user can act.
                let failed = expect_state(&mut state_rx, RemoteConnectionState::Failed).await;
                assert_eq!(failed.as_deref(), Some("Too many incorrect pairing codes"));

                // And nothing may knock again: a retry is the user's call.
                //
                // Watching the *current* viewer would prove nothing, because
                // the failed prompt leaves a cool-down that turns knocks away
                // before they reach a prompt. A replacement viewer has no
                // such history, so a loop that is still running shows up as a
                // code on screen, and a loop that stopped shows up as silence.
                drop(_viewer);
                drop(viewer_rx);
                let (_fresh, mut fresh_rx) = listen(port, PairingPolicy::Generated).await;
                let retried =
                    tokio::time::timeout(RECONNECT_DELAY * 3, expect_code(&mut fresh_rx)).await;
                assert!(
                    retried.is_err(),
                    "the reconnect loop kept going and put another code on the viewer"
                );
            })
            .await;
    }

    #[tokio::test]
    async fn cancelling_the_prompt_gives_up() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Generated).await;
                let port = viewer.local_port();

                let (to_lsp_tx, _to_lsp_rx) = mpsc::unbounded_channel();
                let connector = RemoteLspToPreview::new(to_lsp_tx, Weak::new());

                let connect = connector.connect(["127.0.0.1"], port);
                let cancel = async {
                    let _code = expect_code(&mut viewer_rx).await;
                    wait_for_prompt(&connector).await;
                    connector.cancel_pairing();
                };
                let (result, ()) = tokio::join!(connect, cancel);
                result.expect_err("cancelling should not produce a connection");

                // The viewer takes the code back off its screen.
                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::PairingFinished { accepted: false, .. }),
                    "the prompt coming down",
                )
                .await;
                assert!(
                    connector.shared.tokens.borrow().is_empty(),
                    "cancelling should not leave a token behind"
                );
            })
            .await;
    }

    /// A viewer with pairing disabled cannot be connected to silently: the
    /// user is warned first. Consent is never remembered, so a fresh
    /// connection warns again -- there is no stored answer for a peer to
    /// slip past or for a reused address to inherit.
    #[tokio::test]
    async fn an_unpaired_viewer_needs_the_users_ok() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Disabled).await;
                let port = viewer.local_port();

                let (_previews, connector, mut state_rx, _to_lsp_rx) = connector_with_states();

                // Warned, accepted, connected.
                let connect = connector.connect(["127.0.0.1"], port);
                let accept = async {
                    expect_state(&mut state_rx, RemoteConnectionState::UnpairedWarning).await;
                    connector.accept_unpaired_connection();
                };
                let (result, ()) = tokio::join!(connect, accept);
                result.expect("accepting the warning should complete the connection");
                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::Connected { .. }),
                    "viewer connection",
                )
                .await;
                while state_rx.try_recv().is_ok() {}

                // A second connection to the same viewer must warn again: the
                // earlier acceptance was not stored.
                let connect = connector.connect(["127.0.0.1"], port);
                let accept = async {
                    expect_state(&mut state_rx, RemoteConnectionState::UnpairedWarning).await;
                    connector.accept_unpaired_connection();
                };
                let (result, ()) = tokio::join!(connect, accept);
                result.expect("the second connection should warn, then complete once accepted");

                connector.disconnect().await;
            })
            .await;
    }

    #[tokio::test]
    async fn declining_the_unpaired_warning_gives_up() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, mut viewer_rx) = listen(0, PairingPolicy::Disabled).await;
                let port = viewer.local_port();

                let (_previews, connector, mut state_rx, _to_lsp_rx) = connector_with_states();

                let connect = connector.connect(["127.0.0.1"], port);
                let decline = async {
                    expect_state(&mut state_rx, RemoteConnectionState::UnpairedWarning).await;
                    connector.cancel_pairing();
                };
                let (result, ()) = tokio::join!(connect, decline);
                result.expect_err("declining must not produce a connection");
                let failed = expect_state(&mut state_rx, RemoteConnectionState::Failed).await;
                assert_eq!(failed.as_deref(), Some("Connection cancelled"));

                // The viewer admits an unpaired session on its own side the
                // moment it accepts, so declining shows up there as the
                // editor hanging up before sending anything.
                expect_message(
                    &mut viewer_rx,
                    |m| matches!(m, ConnectionMessage::Disconnected { .. }),
                    "the editor to hang up",
                )
                .await;
            })
            .await;
    }

    /// A viewer is on the far end of the network: it may not drive the editor.
    #[tokio::test]
    async fn drops_messages_a_viewer_may_not_send() {
        use i_slint_live_preview::protocol::PreviewComponent;

        tokio::task::LocalSet::new()
            .run_until(async {
                let (viewer, _viewer_rx, connector, mut to_lsp_rx) = connected_viewer().await;

                // A message that drives the editor, followed by one a viewer is
                // allowed to send.
                viewer
                    .send(PreviewToLspMessage::RequestPreview {
                        component: PreviewComponent {
                            url: lsp_types::Url::parse("file:///test.slint").unwrap(),
                            component: None,
                        },
                    })
                    .unwrap();
                viewer
                    .send(PreviewToLspMessage::Diagnostics {
                        uri: lsp_types::Url::parse("file:///test.slint").unwrap(),
                        version: None,
                        diagnostics: Vec::new(),
                    })
                    .unwrap();

                // The channel keeps the order the viewer sent in, so the
                // diagnostics arriving first means the other one was dropped.
                let message = tokio::time::timeout(Duration::from_secs(15), to_lsp_rx.recv())
                    .await
                    .expect("timed out waiting for the diagnostics")
                    .expect("message channel closed");
                assert!(
                    matches!(message, PreviewToLspMessage::Diagnostics { .. }),
                    "a message the viewer may not send reached the LSP: {message:?}"
                );

                connector.disconnect().await;
            })
            .await;
    }

    type RawViewer = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

    /// Stand in for a peer that completes the WebSocket upgrade and then
    /// misbehaves in a way the real viewer never would, so its handshake
    /// can't be driven by the real `Connection`. `behavior` gets the
    /// upgraded socket to do as it likes; the spawned task holds it for the
    /// test's lifetime. Returns the port to dial.
    async fn raw_viewer<Fut>(behavior: impl FnOnce(RawViewer) -> Fut + 'static) -> u16
    where
        Fut: Future<Output = ()>,
    {
        use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
        use tokio_tungstenite::tungstenite::http::{HeaderValue, header::SEC_WEBSOCKET_PROTOCOL};

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            let (stream, _) = listener.accept().await.unwrap();
            // Echo the subprotocol back, or the client refuses the upgrade.
            let select_protocol = |_req: &Request, mut response: Response| {
                response
                    .headers_mut()
                    .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static(PROTOCOL_SUBPROTOCOL));
                Ok::<_, ErrorResponse>(response)
            };
            let ws = tokio_tungstenite::accept_hdr_async(stream, select_protocol).await.unwrap();
            behavior(ws).await;
        });
        port
    }

    async fn send_frame(ws: &mut RawViewer, message: &PreviewToLspMessage) {
        let bytes = postcard::to_allocvec(message).unwrap();
        ws.send(tokio_tungstenite::tungstenite::Message::Binary(bytes.into())).await.unwrap();
    }

    /// Fail every dialed connection and expect the dialog to be told, so the
    /// user is never stuck on "Connecting".
    async fn expect_connect_failure(port: u16) {
        let (_previews, connector, mut state_rx, _to_lsp_rx) = connector_with_states();
        connector
            .connect(["127.0.0.1"], port)
            .await
            .expect_err("a misbehaving viewer must not produce a connection");
        expect_state(&mut state_rx, RemoteConnectionState::Failed).await;
    }

    /// A viewer that answers the upgrade and then falls silent must not wedge
    /// the connector on "Connecting" forever: the handshake reads are bounded,
    /// so the attempt fails and the dialog can recover. Checked at two points
    /// in the handshake, before and after the first message.
    #[tokio::test(start_paused = true)]
    async fn a_silent_viewer_does_not_hang_the_connector() {
        tokio::task::LocalSet::new()
            .run_until(async {
                // Silent from the very first read.
                let port = raw_viewer(|_ws| std::future::pending()).await;
                expect_connect_failure(port).await;

                // Silent after one legitimate message, a step deeper in.
                let port = raw_viewer(|mut ws| async move {
                    send_frame(&mut ws, &PreviewToLspMessage::PairingReady).await;
                    std::future::pending::<()>().await;
                })
                .await;
                expect_connect_failure(port).await;
            })
            .await;
    }

    /// A per-read timeout alone would let a peer keep the handshake alive
    /// forever by dribbling one ignorable frame just under the limit. The
    /// deadline bounds the whole round, so this fails all the same.
    #[tokio::test(start_paused = true)]
    async fn a_dribbling_viewer_does_not_hang_the_connector() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let port = raw_viewer(|mut ws| async move {
                    send_frame(&mut ws, &PreviewToLspMessage::PairingReady).await;
                    // An ignorable frame, forever, each just inside the read
                    // budget a per-read timeout would have reset to.
                    loop {
                        send_frame(&mut ws, &PreviewToLspMessage::Pong).await;
                        tokio::time::sleep(HANDSHAKE_TIMEOUT - Duration::from_secs(1)).await;
                    }
                })
                .await;
                expect_connect_failure(port).await;
            })
            .await;
    }
}
