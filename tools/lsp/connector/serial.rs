// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The serial side of the remote preview, see [`crate::editor_preview::serial`].

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewToLspMessage, RemoteConnectionState,
};
use tokio::sync::mpsc as tokio_mpsc;

use super::remote::RemoteLspToPreview;
use crate::editor_preview::LspToPreviews;
use crate::editor_preview::serial::{SerialConnection, SerialEvent};

/// Lives on the LSP's thread, next to the WebSocket transport in
/// [`super::remote::RemoteLspToPreview`].
pub struct SerialLspToPreview {
    preview_to_lsp_sender: tokio_mpsc::UnboundedSender<PreviewToLspMessage>,
    to_previews: Weak<LspToPreviews>,
    connection: Rc<RefCell<Option<SerialConnection>>>,
    /// Bumped on every connect or disconnect, so that a replaced session's events are ignored.
    generation: Rc<Cell<u64>>,
}

impl SerialLspToPreview {
    pub fn new(
        preview_to_lsp_sender: tokio_mpsc::UnboundedSender<PreviewToLspMessage>,
        to_previews: Weak<LspToPreviews>,
    ) -> Self {
        Self {
            preview_to_lsp_sender,
            to_previews,
            connection: Rc::default(),
            generation: Rc::default(),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connection.borrow().is_some()
    }

    pub fn send(&self, message: &LspToPreviewMessage) {
        if let Some(connection) = self.connection.borrow().as_ref() {
            connection.send(message);
        }
    }

    pub fn connect(&self, port_name: String) {
        let previous = self.connection.borrow_mut().take();
        self.stop();
        let generation = self.generation.get();
        let (connection, mut events) = SerialConnection::connect(port_name.clone(), previous);
        *self.connection.borrow_mut() = Some(connection);
        RemoteLspToPreview::emit_state(
            &self.to_previews,
            RemoteConnectionState::Connecting,
            port_name.clone(),
            None,
        );

        let to_previews = self.to_previews.clone();
        let preview_to_lsp = self.preview_to_lsp_sender.clone();
        let current_connection = self.connection.clone();
        let current_generation = self.generation.clone();
        crate::editor_preview::spawn_local(async move {
            while let Some(event) = events.recv().await {
                if current_generation.get() != generation {
                    return;
                }
                let (state, error) = match event {
                    SerialEvent::Message(message) => {
                        let _ = preview_to_lsp.send(message);
                        continue;
                    }
                    SerialEvent::Connected { problem } => {
                        (RemoteConnectionState::Connected, problem)
                    }
                    SerialEvent::Failed(err) => {
                        current_connection.borrow_mut().take();
                        (RemoteConnectionState::Failed, Some(err))
                    }
                };
                RemoteLspToPreview::emit_state(&to_previews, state, port_name.clone(), error);
            }
        });
    }

    pub fn disconnect(&self) {
        if self.is_connected() {
            self.stop();
            RemoteLspToPreview::emit_state(
                &self.to_previews,
                RemoteConnectionState::Disconnected,
                String::new(),
                None,
            );
        }
    }

    fn stop(&self) {
        self.generation.set(self.generation.get() + 1);
        self.connection.borrow_mut().take();
    }
}
