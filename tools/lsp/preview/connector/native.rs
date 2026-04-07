// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{common, preview};

use std::collections::HashMap;
use std::io::Write;
use std::{cell::RefCell, io::BufRead};

use tokio::io::AsyncReadExt as _;
use tokio::process::{ChildStderr, ChildStdout};
use tokio::task::JoinHandle;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    sync::mpsc,
};

pub fn resource_url_mapper() -> Option<i_slint_compiler::ResourceUrlMapper> {
    None
}

struct ChildProcessLspToPreviewInner {
    communication_handle: JoinHandle<Result<(), String>>,
    to_child_sender: mpsc::UnboundedSender<String>,
}

pub struct ChildProcessLspToPreview {
    inner: RefCell<Option<ChildProcessLspToPreviewInner>>,
    preview_to_lsp_channel: mpsc::UnboundedSender<common::PreviewToLspMessage>,
}

impl ChildProcessLspToPreview {
    pub fn new(preview_to_lsp_channel: mpsc::UnboundedSender<common::PreviewToLspMessage>) -> Self {
        Self { inner: RefCell::new(None), preview_to_lsp_channel }
    }

    fn preview_is_running(&self) -> bool {
        self.inner.borrow().as_ref().map(|i| !i.communication_handle.is_finished()).unwrap_or(false)
    }

    // We need to forward the stderr from the preview to our own manually.
    // This is necessary because our stderr might be configured in non-blocking mode
    // which causes a panic if used with eprintln! (see issue #10778).
    async fn forward_stderr(stderr: ChildStderr) -> Result<(), String> {
        let mut reader = tokio::io::BufReader::new(stderr);
        let mut buf = Vec::new();
        while reader.read_buf(&mut buf).await.map_err(|e| e.to_string())? > 0 {
            let mut written = 0;
            while written < buf.len() {
                let write = {
                    // lock stderr for as short as possible
                    let mut stderr = std::io::stderr().lock();
                    stderr.write(&buf[written..])
                };
                match write {
                    Ok(bytes_written) => written += bytes_written,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        // Our stderr is set up in non-blocking mode just try again after a delay
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                    Err(err) => {
                        tracing::warn!("Failed to forward preview stderr: {err}");
                        return Err(err.to_string());
                    }
                }
            }
            buf.clear();
        }
        Ok(())
    }

    async fn process_stdout(
        stdout: ChildStdout,
        channel: mpsc::UnboundedSender<common::PreviewToLspMessage>,
    ) -> Result<(), String> {
        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            if let Ok(message) = serde_json::from_str(&line) {
                channel.send(message).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn start_preview(&self) -> common::Result<()> {
        if let Some(inner) = self.inner.borrow_mut().take() {
            inner.communication_handle.abort();
        }

        let mut child = tokio::process::Command::new(
            std::env::current_exe().expect("Could not find executable name of the slint-lsp"),
        )
        .args(["live-preview", "--remote-controlled"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

        tracing::debug!("Preview process spawned (PID {:?})", child.id());

        let from_child = child.stdout.take().expect("Child has no stdout");
        let mut to_child = child.stdin.take().expect("Child has no stdin");
        let from_child_stderr = child.stderr.take().expect("Child has no stderr");

        let channel = self.preview_to_lsp_channel.clone();

        let preview_to_lsp_channel = self.preview_to_lsp_channel.clone();

        let communication_handle = tokio::spawn(async move {
            tokio::try_join! {
                Self::process_stdout(from_child, channel),
                Self::forward_stderr(from_child_stderr),
            }?;

            let exit_status = child.wait().await.map_err(|e| e.to_string());

            if exit_status.map(|exit_status| !exit_status.success()).unwrap_or(true) {
                let message =
                    "The Slint live preview crashed! Please open a bug on the [Slint bug tracker](https://github.com/slint-ui/slint/issues)."
                        .to_string();
                tracing::error!("{message}");

                let _ = preview_to_lsp_channel.send(common::PreviewToLspMessage::SendShowMessage {
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
            let message = serde_json::to_string(&common::LspToPreviewMessage::Quit).unwrap();
            let _ = inner.to_child_sender.send(message);
        }
    }
}

impl common::LspToPreview for ChildProcessLspToPreview {
    fn send(&self, message: &common::LspToPreviewMessage) {
        if self.preview_is_running() {
            let mut inner = self.inner.borrow_mut();
            let inner = inner.as_mut().unwrap();
            let Ok(message) = serde_json::to_string(message) else {
                tracing::debug!("Failed to serialize message to preview");
                return;
            };
            let _ = inner.to_child_sender.send(message);
        } else if let common::LspToPreviewMessage::ShowPreview(_) = message {
            tracing::debug!("Starting preview process");
            self.start_preview().unwrap();
        } else {
            tracing::warn!("Preview not running, dropping message: {:?}", message);
        }
    }

    fn preview_target(&self) -> common::PreviewTarget {
        common::PreviewTarget::ChildProcess
    }

    fn set_preview_target(&self, _: common::PreviewTarget) -> common::Result<()> {
        Err("Can not change the preview target".into())
    }
}

pub struct EmbeddedLspToPreview {
    server_notifier: crate::ServerNotifier,
}

impl EmbeddedLspToPreview {
    pub fn new(server_notifier: crate::ServerNotifier) -> Self {
        Self { server_notifier }
    }
}

impl common::LspToPreview for EmbeddedLspToPreview {
    fn send(&self, message: &common::LspToPreviewMessage) {
        let _ =
            self.server_notifier.send_notification::<common::LspToPreviewMessage>(message.clone());
    }

    fn preview_target(&self) -> common::PreviewTarget {
        common::PreviewTarget::EmbeddedWasm
    }

    fn set_preview_target(&self, _: common::PreviewTarget) -> common::Result<()> {
        Err("Can not change the preview target".into())
    }
}

pub struct SwitchableLspToPreview {
    lsp_to_previews: HashMap<common::PreviewTarget, Box<dyn common::LspToPreview>>,
    current_target: RefCell<common::PreviewTarget>,
}

impl SwitchableLspToPreview {
    pub fn new(
        lsp_to_previews: HashMap<common::PreviewTarget, Box<dyn common::LspToPreview>>,
        current_target: common::PreviewTarget,
    ) -> common::Result<Self> {
        if lsp_to_previews.contains_key(&current_target) {
            Ok(Self { lsp_to_previews, current_target: RefCell::new(current_target) })
        } else {
            Err("No such target".into())
        }
    }
}

impl common::LspToPreview for SwitchableLspToPreview {
    fn send(&self, message: &common::LspToPreviewMessage) {
        self.lsp_to_previews.get(&self.current_target.borrow()).unwrap().send(message);
    }

    fn preview_target(&self) -> common::PreviewTarget {
        self.current_target.borrow().clone()
    }

    fn set_preview_target(&self, target: common::PreviewTarget) -> common::Result<()> {
        if self.lsp_to_previews.contains_key(&target) {
            *self.current_target.borrow_mut() = target;
            Ok(())
        } else {
            Err("Target not found".into())
        }
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
                        preview::connector::lsp_to_preview(message);
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

impl common::PreviewToLsp for RemoteControlledPreviewToLsp {
    fn send(&self, message: &common::PreviewToLspMessage) -> common::Result<()> {
        let message = serde_json::to_string(message).map_err(|e| e.to_string())?;
        println!("{message}");
        Ok(())
    }
}
