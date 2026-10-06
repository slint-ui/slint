// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{future::Future, pin::Pin, time::Duration};

use futures_util::{SinkExt as _, StreamExt as _};
use tokio_tungstenite_wasm::{Message, WebSocketStream};

use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PROTOCOL_SUBPROTOCOL, PairingRejection, PreviewToLspMessage,
    SLINT_PROTOCOLS_HEADER, SLINT_VERSION, SLINT_VERSION_HEADER, pairing, session,
};

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairingCredentials {
    pub token_id: pairing::TokenId,
    pub token: pairing::Token,
}

pub enum PairingPrompt {
    Code { attempts_left: u8, expires_in_seconds: u16 },
    Unpaired,
}

pub enum PairingAnswer {
    Code(String),
    AcceptUnpaired,
    Cancel,
}

pub type PairingInputFuture = Pin<Box<dyn Future<Output = PairingAnswer>>>;

pub struct AuthenticatedSession {
    pub sealing: session::Sealing,
    pub opening: session::Opening,
    pub initial_request: Option<PreviewToLspMessage>,
}

#[derive(Debug)]
pub enum AuthenticationError {
    Cancelled,
    UnpairedDeclined,
    Rejected(PairingRejection),
    Failed(String),
}

impl std::fmt::Display for AuthenticationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("Pairing cancelled"),
            Self::UnpairedDeclined => formatter.write_str("Connection cancelled"),
            Self::Rejected(reason) => write!(formatter, "{reason}"),
            Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AuthenticationError {}

enum ExchangeVerdict {
    Confirmed(pairing::Secrets),
    Rejected(PairingRejection),
}

pub async fn authenticate(
    socket: &mut WebSocketStream,
    mut credentials: Option<PairingCredentials>,
    mut request_input: impl FnMut(PairingPrompt) -> PairingInputFuture,
    mut credentials_changed: impl FnMut(Option<PairingCredentials>),
) -> Result<AuthenticatedSession, AuthenticationError> {
    let announced_token = credentials.is_some();
    let mut deadline = handshake_deadline();
    if !matches!(next_handshake_message(socket, deadline).await?, PreviewToLspMessage::PairingReady)
    {
        return Err(AuthenticationError::Failed(
            "The viewer did not start the pairing handshake".into(),
        ));
    }
    send_message(
        socket,
        &LspToPreviewMessage::PairingHello {
            token: credentials.map(|credentials| credentials.token_id),
        },
        deadline,
    )
    .await?;
    loop {
        match next_handshake_message(socket, deadline).await? {
            PreviewToLspMessage::PairingAccepted => {
                if announced_token {
                    return Err(AuthenticationError::Failed(
                        "The viewer skipped the reconnect exchange".into(),
                    ));
                }
                let initial_request =
                    confirm_unpaired(socket, request_input(PairingPrompt::Unpaired)).await?;
                return Ok(AuthenticatedSession {
                    sealing: session::Sealing::Plaintext,
                    opening: session::Opening::Plaintext,
                    initial_request,
                });
            }
            PreviewToLspMessage::PairingTokenChallenge { element } => {
                let Some(held) = credentials else {
                    return Err(AuthenticationError::Failed(
                        "The viewer opened an exchange for a token we never announced".into(),
                    ));
                };
                let handshake = pairing::Handshake::with_token(pairing::Role::Editor, &held.token);
                match run_exchange(socket, handshake, &element, deadline).await? {
                    ExchangeVerdict::Confirmed(secrets) => {
                        return Ok(authenticated_session(secrets));
                    }
                    ExchangeVerdict::Rejected(PairingRejection::BadToken) => {
                        credentials = None;
                        credentials_changed(None);
                    }
                    ExchangeVerdict::Rejected(reason) => {
                        return Err(AuthenticationError::Rejected(reason));
                    }
                }
            }
            PreviewToLspMessage::PairingRequired { attempts_left, expires_in_seconds, element } => {
                let input =
                    request_input(PairingPrompt::Code { attempts_left, expires_in_seconds });
                let code = prompt_for_code(socket, input).await?;
                let handshake = pairing::Handshake::with_code(pairing::Role::Editor, &code);
                match run_exchange(socket, handshake, &element, handshake_deadline()).await? {
                    ExchangeVerdict::Confirmed(secrets) => {
                        credentials_changed(Some(PairingCredentials {
                            token_id: secrets.token_id,
                            token: secrets.token,
                        }));
                        return Ok(authenticated_session(secrets));
                    }
                    ExchangeVerdict::Rejected(PairingRejection::BadCode) => {}
                    ExchangeVerdict::Rejected(reason) => {
                        return Err(AuthenticationError::Rejected(reason));
                    }
                }
                deadline = handshake_deadline();
            }
            PreviewToLspMessage::PairingRejected { reason: PairingRejection::BadToken } => {
                credentials = None;
                credentials_changed(None);
            }
            PreviewToLspMessage::PairingRejected { reason: PairingRejection::BadCode } => {}
            PreviewToLspMessage::PairingRejected { reason } => {
                return Err(AuthenticationError::Rejected(reason));
            }
            message => tracing::warn!("Ignoring {message:?} during pairing"),
        }
    }
}

fn authenticated_session(secrets: pairing::Secrets) -> AuthenticatedSession {
    let (sealing, opening) = secrets.session();
    AuthenticatedSession { sealing, opening, initial_request: None }
}

async fn run_exchange(
    socket: &mut WebSocketStream,
    handshake: pairing::Handshake,
    viewer_element: &pairing::Element,
    deadline: tokio::time::Instant,
) -> Result<ExchangeVerdict, AuthenticationError> {
    let element = handshake.element().clone();
    let secrets = handshake
        .finish(viewer_element)
        .map_err(|_| AuthenticationError::Failed("The viewer sent a malformed handshake".into()))?;
    send_message(
        socket,
        &LspToPreviewMessage::PairingResponse { element, confirmation: secrets.confirmation() },
        deadline,
    )
    .await?;
    match next_handshake_message(socket, deadline).await? {
        PreviewToLspMessage::PairingConfirm { confirmation }
            if secrets.peer_confirms(&confirmation) =>
        {
            Ok(ExchangeVerdict::Confirmed(secrets))
        }
        PreviewToLspMessage::PairingConfirm { .. } => {
            Err(AuthenticationError::Failed("The viewer could not confirm the pairing".into()))
        }
        PreviewToLspMessage::PairingRejected { reason } => Ok(ExchangeVerdict::Rejected(reason)),
        message => Err(AuthenticationError::Failed(format!(
            "The viewer sent an unexpected {message:?} during pairing"
        ))),
    }
}

async fn prompt_for_code(
    socket: &mut WebSocketStream,
    input: PairingInputFuture,
) -> Result<String, AuthenticationError> {
    tokio::select! {
        answer = input => match answer {
            PairingAnswer::Code(code) => Ok(code),
            PairingAnswer::AcceptUnpaired | PairingAnswer::Cancel => Err(AuthenticationError::Cancelled),
        },
        message = next_message(socket) => Err(match message {
            Some(PreviewToLspMessage::PairingRejected { reason }) => AuthenticationError::Rejected(reason),
            _ => AuthenticationError::Failed("The viewer closed the connection while waiting for the code".into()),
        }),
    }
}

async fn confirm_unpaired(
    socket: &mut WebSocketStream,
    mut input: PairingInputFuture,
) -> Result<Option<PreviewToLspMessage>, AuthenticationError> {
    let mut initial_request = None;
    loop {
        tokio::select! {
            answer = &mut input => return match answer {
                PairingAnswer::AcceptUnpaired => Ok(initial_request),
                PairingAnswer::Code(_) | PairingAnswer::Cancel => Err(AuthenticationError::UnpairedDeclined),
            },
            message = next_message(socket) => {
                let Some(message) = message else {
                    return Err(AuthenticationError::Failed("The viewer closed the connection while waiting for the user".into()));
                };
                if initial_request.is_none() && matches!(&message, PreviewToLspMessage::RequestState { files, .. } if files.is_empty()) {
                    initial_request = Some(message);
                } else {
                    tracing::debug!("Ignoring {message:?} while the user decides");
                }
            }
        }
    }
}

async fn send_message(
    socket: &mut WebSocketStream,
    message: &LspToPreviewMessage,
    deadline: tokio::time::Instant,
) -> Result<(), AuthenticationError> {
    let bytes = postcard::to_allocvec(message).map_err(|error| {
        AuthenticationError::Failed(format!("Failed encoding {message:?}: {error}"))
    })?;
    tokio::time::timeout_at(deadline, socket.send(Message::binary(bytes)))
        .await
        .map_err(|_| {
            AuthenticationError::Failed("Sending to the viewer timed out during pairing".into())
        })?
        .map_err(|error| {
            AuthenticationError::Failed(format!("Failed sending to the viewer: {error}"))
        })
}

async fn next_handshake_message(
    socket: &mut WebSocketStream,
    deadline: tokio::time::Instant,
) -> Result<PreviewToLspMessage, AuthenticationError> {
    match tokio::time::timeout_at(deadline, next_message(socket)).await {
        Ok(Some(message)) => Ok(message),
        Ok(None) => Err(AuthenticationError::Failed(
            "The viewer closed the connection during pairing".into(),
        )),
        Err(_) => {
            Err(AuthenticationError::Failed("The viewer stopped responding during pairing".into()))
        }
    }
}

fn handshake_deadline() -> tokio::time::Instant {
    tokio::time::Instant::now() + HANDSHAKE_TIMEOUT
}

async fn next_message(socket: &mut WebSocketStream) -> Option<PreviewToLspMessage> {
    loop {
        match socket.next().await? {
            Ok(Message::Binary(bytes)) => match postcard::from_bytes(&bytes) {
                Ok(message) => return Some(message),
                Err(error) => {
                    tracing::error!("Failed decoding message from remote viewer: {error}");
                    return None;
                }
            },
            Ok(Message::Text(text)) => {
                tracing::warn!("Ignoring text message from remote viewer: {text}")
            }
            Ok(Message::Close(_)) | Err(_) => return None,
        }
    }
}

pub fn describe_version_mismatch(
    err: &tokio_tungstenite_wasm::Error,
    client: &str,
) -> Option<String> {
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
                    "Version mismatch: viewer runs Slint {viewer_version} (protocol {}), {client} speaks {PROTOCOL_SUBPROTOCOL} (Slint {SLINT_VERSION})",
                    viewer_protocols.unwrap_or("unknown"),
                ))
            } else {
                None
            }
        }
        tokio_tungstenite_wasm::Error::Protocol(
            tokio_tungstenite_wasm::error::ProtocolError::SecWebSocketSubProtocolError(_),
        ) => Some(format!(
            "Version mismatch: viewer does not speak {PROTOCOL_SUBPROTOCOL} (this {client} is Slint {SLINT_VERSION})",
        )),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use i_slint_live_preview::protocol::PROTOCOL_SUBPROTOCOL;
    use tokio_tungstenite::tungstenite::{
        Message as ServerMessage,
        handshake::server::{ErrorResponse, Request, Response},
        http::{HeaderValue, header::SEC_WEBSOCKET_PROTOCOL},
    };

    pub(crate) type RawSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

    pub(crate) async fn raw_peer<Future: std::future::Future<Output = ()> + Send + 'static>(
        behavior: impl FnOnce(RawSocket) -> Future + Send + 'static,
    ) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let socket = tokio_tungstenite::accept_hdr_async(
                socket,
                |_request: &Request, mut response: Response| {
                    response.headers_mut().insert(
                        SEC_WEBSOCKET_PROTOCOL,
                        HeaderValue::from_static(PROTOCOL_SUBPROTOCOL),
                    );
                    Ok::<_, ErrorResponse>(response)
                },
            )
            .await
            .unwrap();
            behavior(socket).await;
        });
        (port, task)
    }

    pub(crate) async fn send_handshake(socket: &mut RawSocket, message: &PreviewToLspMessage) {
        socket
            .send(ServerMessage::Binary(postcard::to_allocvec(message).unwrap().into()))
            .await
            .unwrap();
    }

    pub(crate) async fn receive_handshake(socket: &mut RawSocket) -> LspToPreviewMessage {
        let ServerMessage::Binary(message) = socket.next().await.unwrap().unwrap() else {
            panic!("Expected protocol message");
        };
        postcard::from_bytes(&message).unwrap()
    }

    #[tokio::test]
    async fn initial_request_is_retained_until_plaintext_consent() {
        for answer in [PairingAnswer::AcceptUnpaired, PairingAnswer::Cancel] {
            tokio::time::timeout(Duration::from_secs(5), async {
                let accepted = matches!(answer, PairingAnswer::AcceptUnpaired);
                let (consent_ready, consent_wait) = tokio::sync::oneshot::channel();
                let (finished, finish_wait) = tokio::sync::oneshot::channel();
                let (port, server) = raw_peer(move |mut socket| async move {
                    socket.send(ServerMessage::Binary(postcard::to_allocvec(&PreviewToLspMessage::PairingReady).unwrap().into())).await.unwrap();
                    let ServerMessage::Binary(hello) = socket.next().await.unwrap().unwrap() else { panic!("Expected pairing hello"); };
                    assert!(matches!(postcard::from_bytes(&hello), Ok(LspToPreviewMessage::PairingHello { token: None })));
                    for message in [PreviewToLspMessage::PairingAccepted, PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() }] {
                        socket.send(ServerMessage::Binary(postcard::to_allocvec(&message).unwrap().into())).await.unwrap();
                    }
                    socket.send(ServerMessage::Ping(vec![1].into())).await.unwrap();
                    assert!(matches!(socket.next().await.unwrap().unwrap(), ServerMessage::Pong(_)));
                    consent_ready.send(()).unwrap();
                    finish_wait.await.unwrap();
                }).await;
                let mut socket = tokio_tungstenite_wasm::connect_with_protocols(&format!("ws://127.0.0.1:{port}"), &[PROTOCOL_SUBPROTOCOL]).await.unwrap();
                let mut input = Some((consent_wait, answer));
                let result = authenticate(&mut socket, None, |prompt| {
                    assert!(matches!(prompt, PairingPrompt::Unpaired));
                    let (ready, answer) = input.take().unwrap();
                    Box::pin(async move { ready.await.unwrap(); answer })
                }, |_| panic!("Plaintext consent must not change credentials")).await;
                if accepted {
                    let session = result.unwrap();
                    assert!(matches!(session.initial_request, Some(PreviewToLspMessage::RequestState { files, settings }) if files.is_empty() && settings.is_empty()));
                } else {
                    assert!(matches!(result, Err(AuthenticationError::UnpairedDeclined)));
                }
                finished.send(()).unwrap();
                server.await.unwrap();
            }).await.unwrap();
        }
    }
}
