// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, ffi::OsString, path::PathBuf};

#[cfg(feature = "preview-process")]
use std::{io::BufRead, rc::Rc};

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget, PreviewToLspMessage};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    sync::mpsc,
    task::JoinHandle,
};

struct ChildProcessLspToPreviewInner {
    communication_handle: JoinHandle<Result<(), String>>,
    to_child_sender: mpsc::UnboundedSender<String>,
}

pub struct ChildProcessLspToPreview {
    inner: RefCell<Option<ChildProcessLspToPreviewInner>>,
    executable: PathBuf,
    arguments: Vec<OsString>,
    preview_to_lsp_channel: mpsc::UnboundedSender<PreviewToLspMessage>,
}

impl ChildProcessLspToPreview {
    pub fn new(
        executable: PathBuf,
        arguments: Vec<OsString>,
        preview_to_lsp_channel: mpsc::UnboundedSender<PreviewToLspMessage>,
    ) -> Self {
        Self { inner: RefCell::new(None), executable, arguments, preview_to_lsp_channel }
    }

    fn preview_is_running(&self) -> bool {
        self.inner.borrow().as_ref().map(|i| !i.communication_handle.is_finished()).unwrap_or(false)
    }

    fn start_preview(&self) -> crate::Result<()> {
        if let Some(inner) = self.inner.borrow_mut().take() {
            inner.communication_handle.abort();
        }

        let mut child = tokio::process::Command::new(&self.executable)
            .args(&self.arguments)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        tracing::debug!("Preview process spawned (PID {:?})", child.id());

        let from_child = child.stdout.take().expect("Child has no stdout");
        let mut to_child = child.stdin.take().expect("Child has no stdin");

        let channel = self.preview_to_lsp_channel.clone();

        let communication_handle = tokio::spawn(async move {
            let _exited_guard = scopeguard::guard(channel.clone(), |channel| {
                channel.send(PreviewToLspMessage::Exited).ok();
            });
            let reader = tokio::io::BufReader::new(from_child);
            let mut lines = reader.lines();
            while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
                if let Ok(message) = serde_json::from_str(&line) {
                    channel.send(message).map_err(|e| e.to_string())?;
                }
            }

            let exit_status = child.wait().await.map_err(|e| e.to_string());

            if exit_status.map(|exit_status| !exit_status.success()).unwrap_or(true) {
                let message =
                    "The Slint live preview crashed! Please open a bug on the [Slint bug tracker](https://github.com/slint-ui/slint/issues)."
                        .to_string();
                tracing::error!("{message}");

                let _ = channel.send(PreviewToLspMessage::SendShowMessage {
                    message: lsp_types::ShowMessageParams {
                        typ: lsp_types::MessageType::ERROR,
                        message,
                    },
                });
            }
            Ok(())
        });

        let (to_child_sender, mut to_child_receiver) = mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            while let Some(mut msg) = to_child_receiver.recv().await {
                msg.push('\n');
                if let Err(err) = to_child.write_all(msg.as_bytes()).await {
                    tracing::error!("Failed writing to preview child process: {err}");
                    break;
                }
            }
        });

        *self.inner.borrow_mut() =
            Some(ChildProcessLspToPreviewInner { communication_handle, to_child_sender });

        Ok(())
    }
}

impl Drop for ChildProcessLspToPreview {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.borrow_mut().take() {
            let message = serde_json::to_string(&LspToPreviewMessage::Quit).unwrap();
            let _ = inner.to_child_sender.send(message);
        }
    }
}

impl crate::LspToPreview for ChildProcessLspToPreview {
    fn send(&self, message: &LspToPreviewMessage) {
        if self.preview_is_running() {
            let mut inner = self.inner.borrow_mut();
            let inner = inner.as_mut().unwrap();
            let Ok(message) = serde_json::to_string(message) else {
                tracing::debug!("Failed to serialize message to preview");
                return;
            };
            let _ = inner.to_child_sender.send(message);
        } else if let LspToPreviewMessage::ShowPreview(_) = message {
            tracing::debug!("Starting preview process");
            self.start_preview().unwrap();
        } else {
            tracing::debug!("Preview not running, dropping message: {:?}", message);
        }
    }

    fn preview_target(&self) -> PreviewTarget {
        PreviewTarget::ChildProcess
    }
}

#[cfg(feature = "preview-process")]
pub struct RemoteControlledPreviewToLsp {}

#[cfg(feature = "preview-process")]
impl RemoteControlledPreviewToLsp {
    /// Creates a `RemoteControlledPreviewToLsp` connector.
    ///
    /// The application's lifetime is bound to stdin.
    /// The OS cleans up the reader thread when the process exits.
    ///
    /// Note: If the Slint backend has not been set yet, this will set a backend with the
    /// default Slint BackendSelector.
    pub fn new(
        message_handler: impl Fn(LspToPreviewMessage) -> crate::Result<()> + Send + 'static,
        connection_closed: impl Fn() + Send + 'static,
    ) -> Self {
        // Ensure the backend is set up before the reader thread starts. This fixes
        // bug #10274 on macOS where a race condition was causing the reader thread to already
        // process messages before the event loop was running.
        //
        // Use .ok() to ignore any errors, as the backend might already be set by the user and that's fine.
        slint_interpreter::BackendSelector::new().select().ok();

        std::thread::spawn(move || -> Result<(), String> {
            let reader = std::io::BufReader::new(std::io::stdin().lock());
            for line in reader.lines() {
                let Ok(line) = line else {
                    tracing::debug!("Preview: stdin closed, quitting");
                    connection_closed();
                    return Ok(());
                };
                if let Ok(message) = serde_json::from_str(&line) {
                    message_handler(message).map_err(|error| {
                        let error = error.to_string();
                        tracing::error!(
                            "Failed to queue message onto event loop - reader thread will exit: {error}"
                        );
                        error
                    })?;
                }
            }
            tracing::debug!("Preview: stdin EOF, quitting");
            connection_closed();
            Ok(())
        });
        Self {}
    }
}

#[cfg(feature = "preview-process")]
impl crate::PreviewToLsp for RemoteControlledPreviewToLsp {
    #[allow(clippy::print_stdout)]
    fn send(&self, message: &PreviewToLspMessage) -> crate::Result<()> {
        let message = serde_json::to_string(message).map_err(|error| error.to_string())?;
        println!("{message}");
        Ok(())
    }
}

#[cfg(feature = "preview-process")]
pub fn run() -> crate::Result<()> {
    let (to_preview, from_editor) = mpsc::unbounded_channel();
    let to_editor = Rc::new(RemoteControlledPreviewToLsp::new(
        move |message| {
            to_preview.send(message)?;
            Ok(())
        },
        || {
            slint_interpreter::quit_event_loop().ok();
        },
    )) as Rc<dyn crate::PreviewToLsp>;

    slint_interpreter::spawn_local(async_compat::Compat::new(async move {
        let local_set = tokio::task::LocalSet::new();
        local_set
            .run_until(async move {
                if let Err(error) = i_slint_live_preview::preview_sessions::run_with_channels(
                    from_editor,
                    to_editor,
                )
                .await
                {
                    tracing::error!("Preview error: {error}");
                }
                slint_interpreter::quit_event_loop().ok();
            })
            .await;
    }))?;
    slint_interpreter::run_event_loop()?;
    Ok(())
}
