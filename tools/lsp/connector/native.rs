// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{editor_preview, preview};

use std::io::BufRead;

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget, PreviewToLspMessage};

pub struct EmbeddedLspToPreview {
    server_notifier: crate::ServerNotifier,
}

impl EmbeddedLspToPreview {
    pub fn new(server_notifier: crate::ServerNotifier) -> Self {
        Self { server_notifier }
    }
}

impl editor_preview::LspToPreview for EmbeddedLspToPreview {
    fn send(&self, message: &LspToPreviewMessage) {
        let _ = self.server_notifier.send_notification::<LspToPreviewMessage>(message.clone());
    }

    fn preview_target(&self) -> PreviewTarget {
        PreviewTarget::EmbeddedWasm
    }
}

pub struct RemoteControlledPreviewToLsp {}

impl Default for RemoteControlledPreviewToLsp {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteControlledPreviewToLsp {
    /// Creates a RemoteControlledPreviewToLsp connector.
    ///
    /// This means the applications lifetime is bound to the lifetime of the
    /// application's STDIN: We quit as soon as that gets fishy or closed.
    ///
    /// It also means we do not need to join the reader thread: The OS will clean
    /// that one up for us anyway.
    ///
    /// Note: If the Slint backend has not been set yet, this will set a backend with the
    /// default Slint BackendSelector.
    pub fn new() -> Self {
        let _ = Self::process_input();
        Self {}
    }

    fn process_input() -> std::thread::JoinHandle<std::result::Result<(), String>> {
        // Ensure the backend is set up before the reader thread starts. This fixes
        // bug #10274 on macOS where a race condition was causing the reader thread to already
        // process messages before the event loop was running.
        //
        // Use .ok() to ignore any errors, as the backend might already be set by the user and that's fine.
        slint::BackendSelector::new().select().ok();

        std::thread::spawn(move || -> Result<(), String> {
            let reader = std::io::BufReader::new(std::io::stdin().lock());
            for line in reader.lines() {
                let Ok(line) = line else {
                    tracing::debug!("Preview: stdin closed, quitting");
                    let _ = slint::quit_event_loop();
                    return Ok(());
                };
                if let Ok(message) = serde_json::from_str(&line) {
                    slint::invoke_from_event_loop(move || {
                        preview::lsp_to_preview(message);
                    })
                    .map_err(|err| {
                        let err = err.to_string();
                        tracing::error!("Failed to queue message onto event loop - reader thread will exit: {err}");
                        err
                    })?;
                }
            }
            tracing::debug!("Preview: stdin EOF, quitting");
            let _ = slint::quit_event_loop();
            Ok(())
        })
    }
}

impl editor_preview::PreviewToLsp for RemoteControlledPreviewToLsp {
    #[allow(clippy::print_stdout)]
    fn send(&self, message: &PreviewToLspMessage) -> editor_preview::Result<()> {
        let message = serde_json::to_string(message).map_err(|e| e.to_string())?;
        println!("{message}");
        Ok(())
    }
}
