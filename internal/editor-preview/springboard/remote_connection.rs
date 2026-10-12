// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::Cell;

use futures_util::{
    SinkExt as _, StreamExt as _,
    stream::{SplitSink, SplitStream},
};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PreviewToLspMessage, session,
};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite_wasm::{CloseCode, Message, WebSocketStream};

use super::remote::DiscoveredViewer;
use crate::remote_authentication::{
    self, AuthenticationError, PairingAnswer, PairingCredentials, PairingPrompt,
};
use crate::remote_client::{self, CONNECT_TIMEOUT, PING_INTERVAL, PONG_TIMEOUT};

pub(super) enum ConnectionEvent {
    Pairing { prompt: PairingPrompt, answer: oneshot::Sender<PairingAnswer> },
    CredentialsChanged(Option<PairingCredentials>),
    Connected,
    Message(PreviewToLspMessage),
    Ended { error: Option<String> },
}

pub(super) struct RemoteConnection {
    sender: mpsc::UnboundedSender<LspToPreviewMessage>,
    receiver: mpsc::UnboundedReceiver<ConnectionEvent>,
    close_sender: Option<oneshot::Sender<()>>,
}

impl RemoteConnection {
    pub fn connect(viewer: DiscoveredViewer, credentials: Option<PairingCredentials>) -> Self {
        let (sender, messages) = mpsc::unbounded_channel();
        let (event_sender, receiver) = mpsc::unbounded_channel();
        let (close_sender, close_receiver) = oneshot::channel();
        crate::spawn_local(async move {
            let result = run(viewer, credentials, messages, &event_sender, close_receiver).await;
            let _ = event_sender.send(ConnectionEvent::Ended { error: result.err() });
        });
        Self { sender, receiver, close_sender: Some(close_sender) }
    }

    pub fn send(&self, message: LspToPreviewMessage) {
        let _ = self.sender.send(message);
    }

    pub async fn next_event(&mut self) -> Option<ConnectionEvent> {
        self.receiver.recv().await
    }
}

impl Drop for RemoteConnection {
    fn drop(&mut self) {
        if let Some(sender) = self.close_sender.take() {
            let _ = sender.send(());
        }
    }
}

async fn run(
    viewer: DiscoveredViewer,
    credentials: Option<PairingCredentials>,
    messages: mpsc::UnboundedReceiver<LspToPreviewMessage>,
    events: &mpsc::UnboundedSender<ConnectionEvent>,
    mut close_receiver: oneshot::Receiver<()>,
) -> Result<(), String> {
    let mut socket = tokio::select! {
        result = dial(&viewer) => result?,
        _ = &mut close_receiver => return Ok(()),
    };
    let authenticated = tokio::select! {
        result = remote_authentication::authenticate(
            &mut socket,
            credentials,
            |prompt| {
                let (answer, receiver) = oneshot::channel();
                let _ = events.send(ConnectionEvent::Pairing { prompt, answer });
                Box::pin(async move { receiver.await.unwrap_or(PairingAnswer::Cancel) })
            },
            |credentials| {
                let _ = events.send(ConnectionEvent::CredentialsChanged(credentials));
            },
        ) => result,
        _ = &mut close_receiver => Err(AuthenticationError::Cancelled),
    };
    let authenticated = match authenticated {
        Ok(authenticated) => authenticated,
        Err(error) => {
            remote_client::close(&mut socket).await;
            return Err(error.to_string());
        }
    };
    let (mut sender, receiver) = socket.split();
    let _ = events.send(ConnectionEvent::Connected);
    if let Some(request) = authenticated.initial_request {
        let _ = events.send(ConnectionEvent::Message(request));
    }
    let last_pong = Cell::new(tokio::time::Instant::now());
    let result = tokio::select! {
        result = receive(receiver, authenticated.opening, events, &last_pong) => result,
        result = send(&mut sender, authenticated.sealing, messages, &last_pong) => result,
        _ = &mut close_receiver => Ok(()),
    };
    remote_client::close(&mut sender).await;
    result
}

async fn dial(viewer: &DiscoveredViewer) -> Result<WebSocketStream, String> {
    let mut last_error = "Viewer has no usable address".to_string();
    let mut mismatch = None;
    for address in &viewer.addresses {
        match tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio_tungstenite_wasm::connect_with_protocols(
                &format!("ws://{address}:{}", viewer.port),
                &[PROTOCOL_SUBPROTOCOL],
            ),
        )
        .await
        {
            Ok(Ok(socket)) => return Ok(socket),
            Ok(Err(error)) => {
                mismatch = mismatch.or_else(|| {
                    remote_authentication::describe_version_mismatch(&error, "Springboard")
                });
                last_error = error.to_string();
            }
            Err(_) => last_error = format!("Connection attempt to {address} timed out"),
        }
    }
    Err(mismatch.unwrap_or(last_error))
}

async fn receive(
    mut socket: SplitStream<WebSocketStream>,
    mut opening: session::Opening,
    events: &mpsc::UnboundedSender<ConnectionEvent>,
    last_pong: &Cell<tokio::time::Instant>,
) -> Result<(), String> {
    while let Some(frame) = socket.next().await {
        match frame {
            Ok(Message::Binary(bytes)) => {
                let plain = opening
                    .open(&bytes)
                    .map_err(|_| "Viewer sent a frame that failed authentication".to_string())?;
                let message = match postcard::from_bytes(&plain) {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::error!("Invalid viewer message: {error}");
                        continue;
                    }
                };
                match message {
                    PreviewToLspMessage::Pong => last_pong.set(tokio::time::Instant::now()),
                    message if remote_client::is_allowed_message(&message) => {
                        let _ = events.send(ConnectionEvent::Message(message));
                    }
                    message => tracing::warn!(
                        "Ignoring message that a remote viewer may not send: {message:?}"
                    ),
                }
            }
            Ok(Message::Text(_)) => {}
            Ok(Message::Close(frame)) => {
                return match frame {
                    None => Ok(()),
                    Some(frame) if matches!(frame.code, CloseCode::Normal | CloseCode::Away) => {
                        Ok(())
                    }
                    Some(frame) => Err(format!("Viewer closed the connection: {frame}")),
                };
            }
            Err(tokio_tungstenite_wasm::Error::Protocol(
                tokio_tungstenite_wasm::error::ProtocolError::ResetWithoutClosingHandshake,
            )) => return Err("Connection to viewer was lost".into()),
            Err(error) => return Err(format!("Viewer connection ended: {error}")),
        }
    }
    Err("Connection to viewer was lost".into())
}

async fn send(
    socket: &mut SplitSink<WebSocketStream, Message>,
    mut sealing: session::Sealing,
    mut messages: mpsc::UnboundedReceiver<LspToPreviewMessage>,
    last_pong: &Cell<tokio::time::Instant>,
) -> Result<(), String> {
    let mut heartbeat =
        tokio::time::interval_at(tokio::time::Instant::now() + PING_INTERVAL, PING_INTERVAL);
    loop {
        let message = tokio::select! {
            message = messages.recv() => match message { Some(message) => message, None => return Ok(()) },
            _ = heartbeat.tick() => {
                if last_pong.get().elapsed() >= PONG_TIMEOUT { return Err("Viewer stopped responding to heartbeat".into()); }
                LspToPreviewMessage::Ping
            }
        };
        if matches!(message, LspToPreviewMessage::Quit) {
            continue;
        }
        let plain = postcard::to_allocvec(&message).map_err(|error| error.to_string())?;
        let sealed =
            sealing.seal(plain).map_err(|_| "Viewer session counter exhausted".to_string())?;
        tokio::time::timeout(PONG_TIMEOUT, socket.send(Message::binary(sealed)))
            .await
            .map_err(|_| "Sending to the viewer timed out".to_string())?
            .map_err(|error| error.to_string())?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_authentication::tests::{
        raw_peer, raw_peer_at, receive_handshake, send_handshake,
    };
    use std::time::Duration;

    #[tokio::test]
    async fn ipv6_scope_survives_websocket_dialing() {
        let (port, peer) =
            raw_peer_at((std::net::Ipv6Addr::LOCALHOST, 0).into(), |mut socket| async move {
                assert!(matches!(
                    socket.next().await.unwrap().unwrap(),
                    tokio_tungstenite::tungstenite::Message::Close(_)
                ));
            })
            .await;
        let mut socket = dial(&DiscoveredViewer {
            fullname: "ipv6".into(),
            name: "IPv6".into(),
            addresses: vec!["[::1%0]".into()],
            port,
            unavailable_reason: String::new(),
        })
        .await
        .unwrap();
        remote_client::close(&mut socket).await;
        tokio::time::timeout(Duration::from_secs(5), peer).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn dropping_connections_sends_close_during_pairing_consent_and_preview() {
        tokio::task::LocalSet::new()
            .run_until(async {
                for (generated, accept) in [(true, false), (false, false), (false, true)] {
                    let (port, peer) = raw_peer(move |mut socket| async move {
                        send_handshake(&mut socket, &PreviewToLspMessage::PairingReady).await;
                        assert!(matches!(
                            receive_handshake(&mut socket).await,
                            LspToPreviewMessage::PairingHello { .. }
                        ));
                        let response = if generated {
                            let handshake =
                                i_slint_live_preview::protocol::pairing::Handshake::with_code(
                                    i_slint_live_preview::protocol::pairing::Role::Viewer,
                                    "1234",
                                );
                            PreviewToLspMessage::PairingRequired {
                                attempts_left: 3,
                                expires_in_seconds: 60,
                                code_digits: 4,
                                element: handshake.element().clone(),
                            }
                        } else {
                            PreviewToLspMessage::PairingAccepted
                        };
                        send_handshake(&mut socket, &response).await;
                        assert!(matches!(
                            socket.next().await.unwrap().unwrap(),
                            tokio_tungstenite::tungstenite::Message::Close(_)
                        ));
                    })
                    .await;
                    let mut connection = RemoteConnection::connect(
                        DiscoveredViewer {
                            fullname: "closing-peer".into(),
                            name: "Closing".into(),
                            addresses: vec!["127.0.0.1".into()],
                            port,
                            unavailable_reason: String::new(),
                        },
                        None,
                    );
                    let ConnectionEvent::Pairing { answer, .. } =
                        connection.next_event().await.unwrap()
                    else {
                        panic!("Expected pairing input");
                    };
                    if accept {
                        assert!(answer.send(PairingAnswer::AcceptUnpaired).is_ok());
                        assert!(matches!(
                            connection.next_event().await,
                            Some(ConnectionEvent::Connected)
                        ));
                    }
                    drop(connection);
                    tokio::time::timeout(Duration::from_secs(5), peer).await.unwrap().unwrap();
                }
            })
            .await;
    }

    #[tokio::test]
    async fn connected_peer_without_pongs_times_out() {
        tokio::task::LocalSet::new().run_until(async {
            let (ping_seen, ping_wait) = oneshot::channel();
            let (port, peer) = raw_peer(move |mut socket| async move {
                send_handshake(&mut socket, &PreviewToLspMessage::PairingReady).await;
                assert!(matches!(receive_handshake(&mut socket).await, LspToPreviewMessage::PairingHello { .. }));
                send_handshake(&mut socket, &PreviewToLspMessage::PairingAccepted).await;
                let mut ping_seen = Some(ping_seen);
                while let Some(Ok(frame)) = socket.next().await {
                    if let tokio_tungstenite::tungstenite::Message::Binary(bytes) = frame {
                        assert!(matches!(postcard::from_bytes(&bytes), Ok(LspToPreviewMessage::Ping)));
                        if let Some(ping_seen) = ping_seen.take() { let _ = ping_seen.send(()); }
                    }
                }
            }).await;
            let mut connection = RemoteConnection::connect(DiscoveredViewer {
                fullname: "silent-peer".into(), name: "Silent".into(), addresses: vec!["127.0.0.1".into()], port, unavailable_reason: String::new(),
            }, None);
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    match connection.next_event().await.unwrap() {
                        ConnectionEvent::Pairing { prompt: PairingPrompt::Unpaired, answer } => { let _ = answer.send(PairingAnswer::AcceptUnpaired); }
                        ConnectionEvent::Connected => break,
                        _ => panic!("Unexpected event before connection"),
                    }
                }
            }).await.unwrap();
            tokio::time::pause();
            tokio::time::advance(PING_INTERVAL).await;
            tokio::time::timeout(Duration::from_secs(1), ping_wait).await.unwrap().unwrap();
            tokio::time::advance(PONG_TIMEOUT).await;
            let ended = tokio::time::timeout(Duration::from_secs(1), connection.next_event()).await.unwrap().unwrap();
            assert!(matches!(ended, ConnectionEvent::Ended { error: Some(message) } if message.contains("heartbeat")));
            drop(connection);
            peer.await.unwrap();
        }).await;
    }
}
