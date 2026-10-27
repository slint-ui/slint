// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::pin::Pin;

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewToLspMessage, pairing};
use tokio::sync::{mpsc, oneshot};

use super::LocalPreviewConfig;
use super::remote::RemoteViewer;
use super::remote_connection::{self, ConnectionEvent, DialError, RemoteConnection};
use crate::{
    child_process::ChildProcessLspToPreview,
    remote_authentication::{PairingAnswer, PairingCredentials, PairingPrompt},
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
}

#[derive(Default)]
pub(super) struct Connection {
    pub(super) state: ui::ConnectionState,
    pub(super) pairing_code_digits: u8,
    pub(super) error: String,
    preview: Option<ActivePreview>,
}

impl Connection {
    pub(super) fn local(config: &LocalPreviewConfig) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let preview = ChildProcessLspToPreview::new(
            config.executable.clone(),
            config.arguments.clone(),
            sender,
        );
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
            None => std::future::pending().await,
        }
    }

    pub(super) fn fail(&mut self, error: String) {
        self.preview = None;
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
                self.pairing_code_digits = code_digits;
                self.error = if attempts_left < pairing::MAX_ATTEMPTS {
                    format!("Incorrect code. {attempts_left} attempts left")
                } else {
                    String::new()
                };
            }
            PairingPrompt::Unpaired => {
                self.state = ui::ConnectionState::UnpairedConfirmationRequired;
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
                if !pairing::is_valid_code(code)
                    || code.len() != usize::from(self.pairing_code_digits)
                {
                    self.error = format!("Enter {} numeric digits", self.pairing_code_digits);
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
            _ => {}
        }
    }
}

/// Whether a viewer answers at the address the user is typing.
///
/// Unlike [`RemoteConnection`], which runs in a spawned task, it holds the dial
/// itself: replacing it on the next keystroke cancels the previous dial.
#[derive(Default)]
pub(super) struct ManualProbe {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) state: ui::ManualEndpointState,
    /// The viewer's Slint version, when it's `Incompatible`.
    pub(super) version: String,
    pending: Option<Pin<Box<dyn std::future::Future<Output = Result<(), DialError>>>>>,
}

impl ManualProbe {
    /// Replaces the probe in flight. An empty host resets it.
    pub(super) fn start(&mut self, host: String, port: i32) {
        if host.is_empty() {
            *self = Self::default();
            return;
        }
        let Some(port) = u16::try_from(port).ok().filter(|port| *port != 0) else {
            *self =
                Self { host, state: ui::ManualEndpointState::Unreachable, ..Default::default() };
            return;
        };
        let viewer = RemoteViewer::manual(&host, port);
        self.pending = Some(Box::pin(async move {
            crate::remote_client::close(&mut remote_connection::dial(&viewer).await?).await;
            Ok(())
        }));
        self.host = host;
        self.port = port;
        self.state = ui::ManualEndpointState::Probing;
        self.version.clear();
    }

    /// Pending while no probe is in flight.
    pub(super) async fn finished(&mut self) {
        let Some(pending) = &mut self.pending else { return std::future::pending().await };
        let result = pending.await;
        self.pending = None;
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
