// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_live_preview::springboard_runtime::{
    ENDPOINT_ENVIRONMENT_VARIABLE, PROTOCOL_VERSION, RuntimeEvent, RuntimeEventAcknowledgement,
    RuntimeEventEnvelope, TOKEN_ENVIRONMENT_VARIABLE,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio::task::{JoinHandle, JoinSet};

pub struct RuntimeControlServer {
    endpoint: SocketAddr,
    token: String,
    events: mpsc::UnboundedReceiver<RuntimeEvent>,
    shutdown: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
}

impl RuntimeControlServer {
    pub async fn bind() -> std::io::Result<Self> {
        let listener =
            TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).await?;
        let endpoint = listener.local_addr()?;
        let token = generate_token()?;
        let (event_sender, events) = mpsc::unbounded_channel();
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let task =
            tokio::spawn(run_server(listener, token.clone(), event_sender, shutdown_receiver));
        Ok(Self { endpoint, token, events, shutdown, task: Some(task) })
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn environment(&self) -> [(&'static str, String); 2] {
        [
            (ENDPOINT_ENVIRONMENT_VARIABLE, self.endpoint.to_string()),
            (TOKEN_ENVIRONMENT_VARIABLE, self.token.clone()),
        ]
    }

    pub async fn next_event(&mut self) -> Option<RuntimeEvent> {
        self.events.recv().await
    }

    pub fn try_next_event(&mut self) -> Option<RuntimeEvent> {
        self.events.try_recv().ok()
    }

    pub async fn shutdown(mut self) {
        self.stop().await;
    }

    async fn stop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for RuntimeControlServer {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn run_server(
    listener: TcpListener,
    token: String,
    events: mpsc::UnboundedSender<RuntimeEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            result = shutdown.changed() => {
                if result.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            result = listener.accept() => {
                let Ok((stream, peer)) = result else { break };
                if peer.ip().is_loopback() {
                    connections.spawn(handle_connection(
                        stream,
                        token.clone(),
                        events.clone(),
                        shutdown.clone(),
                    ));
                }
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    connections.abort_all();
    while connections.join_next().await.is_some() {}
}

async fn handle_connection(
    stream: TcpStream,
    token: String,
    events: mpsc::UnboundedSender<RuntimeEvent>,
    mut shutdown: watch::Receiver<bool>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    loop {
        let line = tokio::select! {
            result = shutdown.changed() => {
                if result.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
            result = lines.next_line() => match result {
                Ok(Some(line)) => line,
                _ => break,
            }
        };
        let acknowledgement = match serde_json::from_str::<RuntimeEventEnvelope>(&line) {
            Ok(envelope) if envelope.protocol_version != PROTOCOL_VERSION => {
                rejected("incompatible runtime protocol version")
            }
            Ok(envelope) if envelope.token != token => rejected("invalid Springboard token"),
            Ok(envelope) => {
                if events.send(envelope.event).is_ok() {
                    accepted()
                } else {
                    rejected("Springboard session has stopped")
                }
            }
            Err(error) => rejected(format!("invalid runtime event: {error}")),
        };
        if write_acknowledgement(&mut writer, &acknowledgement).await.is_err() {
            break;
        }
    }
}

fn accepted() -> RuntimeEventAcknowledgement {
    RuntimeEventAcknowledgement { protocol_version: PROTOCOL_VERSION, accepted: true, error: None }
}

fn rejected(error: impl Into<String>) -> RuntimeEventAcknowledgement {
    RuntimeEventAcknowledgement {
        protocol_version: PROTOCOL_VERSION,
        accepted: false,
        error: Some(error.into()),
    }
}

async fn write_acknowledgement(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    acknowledgement: &RuntimeEventAcknowledgement,
) -> std::io::Result<()> {
    let mut response = serde_json::to_vec(acknowledgement)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    response.push(b'\n');
    writer.write_all(&response).await
}

fn generate_token() -> std::io::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
    let mut token = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        token.push(HEX[usize::from(byte >> 4)] as char);
        token.push(HEX[usize::from(byte & 0xf)] as char);
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_live_preview::springboard_runtime::SpringboardRuntimeReporter;
    use std::time::Duration;

    #[tokio::test]
    async fn accepts_authenticated_runtime_events() {
        let mut server = RuntimeControlServer::bind().await.unwrap();
        let environment = server.environment();
        assert_eq!(environment[0].0, ENDPOINT_ENVIRONMENT_VARIABLE);
        assert_eq!(environment[1].0, TOKEN_ENVIRONMENT_VARIABLE);
        let endpoint = server.endpoint();
        let token = server.token().to_owned();
        let report = tokio::task::spawn_blocking(move || {
            let mut reporter = SpringboardRuntimeReporter::connect(endpoint, token).unwrap();
            reporter.report(RuntimeEvent::Ready).unwrap();
        });

        assert_eq!(server.next_event().await, Some(RuntimeEvent::Ready));
        report.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_an_invalid_token_without_poisoning_the_server() {
        let mut server = RuntimeControlServer::bind().await.unwrap();
        let endpoint = server.endpoint();
        let bad_report = tokio::task::spawn_blocking(move || {
            let mut reporter = SpringboardRuntimeReporter::connect(endpoint, "invalid").unwrap();
            reporter.report(RuntimeEvent::Ready).unwrap_err().kind()
        });
        assert_eq!(bad_report.await.unwrap(), std::io::ErrorKind::PermissionDenied);

        let endpoint = server.endpoint();
        let token = server.token().to_owned();
        let good_report = tokio::task::spawn_blocking(move || {
            let mut reporter = SpringboardRuntimeReporter::connect(endpoint, token).unwrap();
            reporter.report(RuntimeEvent::Reloaded).unwrap();
        });
        assert_eq!(server.next_event().await, Some(RuntimeEvent::Reloaded));
        good_report.await.unwrap();
    }

    #[tokio::test]
    async fn manager_shutdown_closes_runtime_connections() {
        let server = RuntimeControlServer::bind().await.unwrap();
        let endpoint = server.endpoint();
        let token = server.token().to_owned();
        let mut reporter = tokio::task::spawn_blocking(move || {
            SpringboardRuntimeReporter::connect(endpoint, token).unwrap()
        })
        .await
        .unwrap();
        server.shutdown().await;

        let error = tokio::task::spawn_blocking(move || reporter.report(RuntimeEvent::Exiting))
            .await
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            error.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::WouldBlock
                | std::io::ErrorKind::TimedOut
        ));
    }

    #[tokio::test]
    async fn waiting_for_an_event_can_be_bounded() {
        let mut server = RuntimeControlServer::bind().await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(10), server.next_event()).await.is_err()
        );
    }
}
