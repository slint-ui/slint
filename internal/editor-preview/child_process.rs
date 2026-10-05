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
        self.inner.borrow().as_ref().is_some_and(|inner| !inner.communication_handle.is_finished())
    }

    pub fn start_preview(&self) -> crate::Result<()> {
        if self.preview_is_running() {
            return Ok(());
        }
        self.inner.borrow_mut().take();

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
            while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
                if let Ok(message) = serde_json::from_str(&line) {
                    let _ = channel.send(message);
                }
            }

            let exit_status = child.wait().await.map_err(|error| error.to_string());

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
            while let Some(mut message) = to_child_receiver.recv().await {
                message.push('\n');
                if let Err(error) = to_child.write_all(message.as_bytes()).await {
                    tracing::error!("Failed writing to preview child process: {error}");
                    break;
                }
            }
        });

        *self.inner.borrow_mut() =
            Some(ChildProcessLspToPreviewInner { communication_handle, to_child_sender });

        Ok(())
    }

    pub(crate) fn send_running(&self, message: &LspToPreviewMessage) {
        if let Some(inner) = self.inner.borrow().as_ref() {
            if let Ok(message) = serde_json::to_string(message) {
                let _ = inner.to_child_sender.send(message);
            }
        }
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
            self.send_running(message);
        } else if matches!(message, LspToPreviewMessage::ShowPreview(_)) {
            if let Err(error) = self.start_preview() {
                tracing::error!("Failed starting preview: {error}");
            }
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::LspToPreview;
    use std::io::{BufRead, Write};

    pub(crate) fn fixture_config(name: &str) -> (PathBuf, Vec<OsString>) {
        (
            std::env::current_exe().unwrap(),
            vec![
                "--exact".into(),
                format!("child_process::tests::{name}").into(),
                "--ignored".into(),
                "--nocapture".into(),
                "--skip".into(),
                "preview-transport-child-fixture".into(),
            ],
        )
    }

    pub(crate) fn component() -> i_slint_live_preview::protocol::PreviewComponent {
        i_slint_live_preview::protocol::PreviewComponent {
            url: lsp_types::Url::parse("file:///fixture.slint").unwrap(),
            component: Some("Fixture".into()),
        }
    }

    fn fixture_started() -> bool {
        if !std::env::args().any(|argument| argument == "preview-transport-child-fixture") {
            return false;
        }
        println!();
        println!(
            "{}",
            serde_json::to_string(&PreviewToLspMessage::DebugMessage {
                location: None,
                message: std::process::id().to_string(),
            })
            .unwrap()
        );
        println!(
            "{}",
            serde_json::to_string(&PreviewToLspMessage::RequestState {
                files: Vec::new(),
                settings: Vec::new(),
            })
            .unwrap()
        );
        std::io::stdout().flush().unwrap();
        true
    }

    #[test]
    #[ignore]
    fn echo_child() {
        if !fixture_started() {
            return;
        }
        for line in std::io::stdin().lock().lines() {
            let line = line.unwrap();
            if matches!(serde_json::from_str(&line), Ok(LspToPreviewMessage::Quit)) {
                std::process::exit(0);
            }
            println!(
                "{}",
                serde_json::to_string(&PreviewToLspMessage::DebugMessage {
                    location: None,
                    message: line,
                })
                .unwrap()
            );
            std::io::stdout().flush().unwrap();
        }
    }

    #[test]
    #[ignore]
    fn crashing_child() {
        if !fixture_started() {
            return;
        }
        std::process::exit(23);
    }

    pub(crate) async fn receive(
        receiver: &mut mpsc::UnboundedReceiver<PreviewToLspMessage>,
    ) -> PreviewToLspMessage {
        tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
            .await
            .unwrap()
            .unwrap()
    }

    pub(crate) async fn started(
        receiver: &mut mpsc::UnboundedReceiver<PreviewToLspMessage>,
    ) -> u32 {
        let PreviewToLspMessage::DebugMessage { message, .. } = receive(receiver).await else {
            panic!("Expected process ID");
        };
        let process_id = message.parse().unwrap();
        assert!(matches!(receive(receiver).await, PreviewToLspMessage::RequestState { .. }));
        process_id
    }

    pub(crate) fn assert_reaped(process_id: u32) {
        #[cfg(target_os = "linux")]
        assert!(!std::path::Path::new(&format!("/proc/{process_id}")).exists());
        #[cfg(not(target_os = "linux"))]
        let _ = process_id;
    }

    #[tokio::test]
    async fn explicit_start_forwards_without_automatic_restart() {
        let (executable, arguments) = fixture_config("echo_child");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let transport = ChildProcessLspToPreview::new(executable, arguments, sender);
        assert!(receiver.try_recv().is_err());
        transport.start_preview().unwrap();
        let process_id = started(&mut receiver).await;
        transport.start_preview().unwrap();
        transport.send_running(&LspToPreviewMessage::ShowPreview(component()));
        let PreviewToLspMessage::DebugMessage { message, .. } = receive(&mut receiver).await else {
            panic!("Expected forwarded preview request");
        };
        assert!(
            matches!(serde_json::from_str(&message), Ok(LspToPreviewMessage::ShowPreview(current)) if current == component())
        );
        transport.send(&LspToPreviewMessage::Quit);
        assert!(matches!(receive(&mut receiver).await, PreviewToLspMessage::Exited));
        assert_reaped(process_id);
        transport.send_running(&LspToPreviewMessage::ShowPreview(component()));
        assert!(receiver.try_recv().is_err());
        assert!(!transport.preview_is_running());
    }

    #[tokio::test]
    async fn unexpected_exit_reports_crash_and_reaps_child() {
        let (executable, arguments) = fixture_config("crashing_child");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let transport = ChildProcessLspToPreview::new(executable, arguments, sender);
        transport.start_preview().unwrap();
        let process_id = started(&mut receiver).await;
        let PreviewToLspMessage::SendShowMessage { message } = receive(&mut receiver).await else {
            panic!("Expected crash report");
        };
        assert_eq!(message.typ, lsp_types::MessageType::ERROR);
        assert!(message.message.contains("https://github.com/slint-ui/slint/issues"));
        assert!(matches!(receive(&mut receiver).await, PreviewToLspMessage::Exited));
        assert_reaped(process_id);
    }

    #[tokio::test]
    async fn automatic_start_and_quit_on_drop() {
        let (executable, arguments) = fixture_config("echo_child");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let transport = ChildProcessLspToPreview::new(executable, arguments, sender);
        transport.send(&LspToPreviewMessage::ShowPreview(component()));
        let process_id = started(&mut receiver).await;
        drop(transport);
        assert!(matches!(receive(&mut receiver).await, PreviewToLspMessage::Exited));
        assert_reaped(process_id);
    }
}
