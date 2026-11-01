// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{ffi::OsString, pin::Pin};

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewToLspMessage, pairing};
use tokio::sync::{mpsc, oneshot};

use super::LocalPreviewConfig;
use super::discovery::RemoteViewer;
use super::remote_connection::{self, ConnectionEvent, DialError, RemoteConnection};
use crate::{
    child_process::ChildProcessLspToPreview,
    remote_authentication::{PairingAnswer, PairingCredentials, PairingPrompt},
    serial::{SerialConnection, SerialEvent, is_serial_port_name},
    springboard_ui as ui,
};

enum ActivePreview {
    Local {
        preview: ChildProcessLspToPreview,
        receiver: mpsc::UnboundedReceiver<PreviewToLspMessage>,
    },
    Remote {
        connection: RemoteConnection,
        pairing_answer: Option<oneshot::Sender<PairingAnswer>>,
    },
    Serial {
        connection: SerialConnection,
        events: mpsc::UnboundedReceiver<SerialEvent>,
    },
}

#[derive(Default)]
pub(super) struct Connection {
    pub(super) state: ui::ConnectionState,
    /// Set while the viewer waits for its code.
    pub(super) pairing_code_length: Option<u8>,
    pub(super) error: String,
    preview: Option<ActivePreview>,
}

impl Connection {
    pub(super) fn local(config: &LocalPreviewConfig, arguments: Vec<OsString>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let preview = ChildProcessLspToPreview::new(config.executable.clone(), arguments, sender)
            .with_started_callback(config.on_started.clone());
        match preview.start_preview() {
            Ok(()) => Self {
                state: ui::ConnectionState::Connected,
                preview: Some(ActivePreview::Local { preview, receiver }),
                ..Default::default()
            },
            Err(error) => Self {
                state: ui::ConnectionState::Failed,
                error: format!("Failed to launch Local preview: {error}"),
                ..Default::default()
            },
        }
    }

    pub(super) fn remote(viewer: RemoteViewer, credentials: Option<PairingCredentials>) -> Self {
        Self {
            state: ui::ConnectionState::Connecting,
            preview: Some(ActivePreview::Remote {
                connection: RemoteConnection::connect(viewer, credentials),
                pairing_answer: None,
            }),
            ..Default::default()
        }
    }

    pub(super) fn serial(port: String) -> Self {
        let (connection, events) = SerialConnection::connect(port, None);
        Self {
            state: ui::ConnectionState::Connecting,
            preview: Some(ActivePreview::Serial { connection, events }),
            ..Default::default()
        }
    }

    pub(super) fn is_started(&self) -> bool {
        self.preview.is_some()
    }

    /// Pending while nothing is started. `None` when the preview went away.
    pub(super) async fn next_event(&mut self) -> Option<ConnectionEvent> {
        match &mut self.preview {
            Some(ActivePreview::Remote { connection, .. }) => connection.next_event().await,
            Some(ActivePreview::Local { receiver, .. }) => {
                receiver.recv().await.map(ConnectionEvent::Message)
            }
            Some(ActivePreview::Serial { events, .. }) => Some(match events.recv().await? {
                SerialEvent::Connected { problem } => ConnectionEvent::Connected { problem },
                SerialEvent::Failed(error) => ConnectionEvent::Ended { error: Some(error) },
                SerialEvent::Message(message) => ConnectionEvent::Message(message),
            }),
            None => std::future::pending().await,
        }
    }

    pub(super) fn fail(&mut self, error: String) {
        self.preview = None;
        self.pairing_code_length = None;
        self.state = ui::ConnectionState::Failed;
        self.error = error;
    }

    /// The viewer asks to pair: show its prompt, and keep `answer` for [`Self::answer`].
    pub(super) fn prompt(&mut self, prompt: PairingPrompt, answer: oneshot::Sender<PairingAnswer>) {
        let Some(ActivePreview::Remote { pairing_answer, .. }) = &mut self.preview else {
            return;
        };
        *pairing_answer = Some(answer);
        match prompt {
            PairingPrompt::Code { attempts_left, code_digits, .. } => {
                self.state = ui::ConnectionState::PairingRequired;
                self.pairing_code_length = Some(code_digits);
                self.error = if attempts_left < pairing::MAX_ATTEMPTS {
                    format!("Incorrect code. {attempts_left} attempts left")
                } else {
                    String::new()
                };
            }
            PairingPrompt::Unpaired => {
                self.state = ui::ConnectionState::UnpairedConfirmationRequired;
                self.pairing_code_length = None;
                self.error.clear();
            }
        }
    }

    pub(super) fn connected(&mut self) {
        self.state = ui::ConnectionState::Connected;
        if let Some(ActivePreview::Remote { pairing_answer, .. }) = &mut self.preview {
            *pairing_answer = None;
        }
        self.error.clear();
    }

    /// Answer the pairing prompt, if `answer` fits it.
    pub(super) fn answer(&mut self, answer: PairingAnswer) {
        match &answer {
            PairingAnswer::Code(code) if self.state == ui::ConnectionState::PairingRequired => {
                let length = self.pairing_code_length.unwrap_or(pairing::CODE_DIGITS as u8);
                if !pairing::is_valid_code(code) || code.len() != usize::from(length) {
                    self.error = format!("Enter {length} numeric digits");
                    return;
                }
            }
            PairingAnswer::AcceptUnpaired
                if self.state == ui::ConnectionState::UnpairedConfirmationRequired => {}
            _ => return,
        }
        let Some(ActivePreview::Remote { pairing_answer, .. }) = &mut self.preview else {
            return;
        };
        if let Some(sender) = pairing_answer.take() {
            let _ = sender.send(answer);
            self.state = ui::ConnectionState::Connecting;
            self.error.clear();
        }
    }

    #[cfg(test)]
    pub(super) fn awaits_answer(&self) -> bool {
        matches!(&self.preview, Some(ActivePreview::Remote { pairing_answer: Some(_), .. }))
    }

    #[cfg(test)]
    pub(super) fn replace_local_receiver(
        &mut self,
        receiver: mpsc::UnboundedReceiver<PreviewToLspMessage>,
    ) {
        let Some(ActivePreview::Local { receiver: current, .. }) = &mut self.preview else {
            panic!("Expected Local preview");
        };
        *current = receiver;
    }

    /// Forward `message` to the preview, once connected.
    pub(super) fn send(&self, message: LspToPreviewMessage) {
        match &self.preview {
            Some(ActivePreview::Local { preview, .. }) => preview.send_running(&message),
            Some(ActivePreview::Remote { connection, .. })
                if self.state == ui::ConnectionState::Connected =>
            {
                connection.send(message)
            }
            Some(ActivePreview::Serial { connection, .. }) => connection.send(&message),
            _ => {}
        }
    }
}

/// Whether a viewer answers at the address the user is typing.
///
/// Unlike [`RemoteConnection`], which runs in a spawned task, it holds the dial
/// itself: replacing it on the next keystroke cancels the previous dial.
pub(super) struct ManualProbe {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) state: ui::ManualEndpointState,
    /// The viewer's Slint version, when it's `Incompatible`.
    pub(super) version: String,
    dial: Pin<Box<dyn std::future::Future<Output = Result<(), DialError>>>>,
}

impl ManualProbe {
    /// `None` for an empty host.
    pub(super) fn start(host: String, port: i32) -> Option<Self> {
        if host.is_empty() {
            return None;
        }
        let port = u16::try_from(port).ok().filter(|port| *port != 0);
        let viewer = port.map(|port| RemoteViewer::manual(&host, port));
        Some(Self {
            state: if is_serial_port_name(&host) {
                ui::ManualEndpointState::SerialPort
            } else {
                ui::ManualEndpointState::Probing
            },
            host,
            port: port.unwrap_or_default(),
            version: String::new(),
            dial: Box::pin(async move {
                let viewer = viewer.ok_or_else(|| DialError::Unreachable("Invalid port".into()))?;
                crate::remote_client::close(&mut remote_connection::dial(&viewer).await?).await;
                Ok(())
            }),
        })
    }

    /// Pending once the probe has finished.
    pub(super) async fn finished(&mut self) {
        if self.state != ui::ManualEndpointState::Probing {
            return std::future::pending().await;
        }
        let result = self.dial.as_mut().await;
        self.state = match result {
            Ok(()) => ui::ManualEndpointState::Reachable,
            Err(DialError::Incompatible { version, .. }) => {
                self.version = version;
                ui::ManualEndpointState::Incompatible
            }
            Err(DialError::Unreachable(error)) => {
                tracing::debug!("Probing {}:{} failed: {error}", self.host, self.port);
                ui::ManualEndpointState::Unreachable
            }
        };
    }
}
