// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{common, preview};

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead as _, Write as _};
use std::sync::{Arc, Mutex};

pub fn resource_url_mapper() -> Option<i_slint_compiler::ResourceUrlMapper> {
    None
}

struct ChildProcessLspToPreviewInner {
    communication_handle: std::thread::JoinHandle<std::result::Result<(), String>>,
    to_child: std::process::ChildStdin,
    child: Arc<Mutex<std::process::Child>>,
}

pub struct ChildProcessLspToPreview {
    inner: RefCell<Option<ChildProcessLspToPreviewInner>>,
    preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
}

impl ChildProcessLspToPreview {
    pub fn new(
        preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
    ) -> Self {
        Self { inner: RefCell::new(None), preview_to_lsp_channel }
    }

    fn preview_is_running(&self) -> bool {
        self.inner.borrow().as_ref().map(|i| !i.communication_handle.is_finished()).unwrap_or(false)
    }

    fn start_preview(&self) -> common::Result<()> {
        if let Some(inner) = self.inner.borrow_mut().take() {
            let _ = inner.communication_handle.join();
        }

        let mut child = std::process::Command::new(
            std::env::current_exe().expect("Could not find executable name of the slint-lsp"),
        )
        .args(["live-preview", "--remote-controlled"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

        let from_child = child.stdout.take().expect("Child has no stdout");
        let to_child = child.stdin.take().expect("Child has no stdin");

        let channel = self.preview_to_lsp_channel.clone();

        let child = Arc::new(Mutex::new(child));
        let child_clone = child.clone();

        let preview_to_lsp_channel = self.preview_to_lsp_channel.clone();

        let communication_handle = std::thread::spawn(move || -> Result<(), String> {
            let reader = std::io::BufReader::new(from_child);
            for line in reader.lines() {
                let line = line.map_err(|e| e.to_string())?;
                if let Ok(message) = serde_json::from_str(&line) {
                    channel.send(message).map_err(|e| e.to_string())?;
                }
            }
            let mut child = child_clone.lock().expect("This can be waited for...");
            let exit_status = child.wait().map_err(|e| e.to_string())?;

            if !exit_status.success() {
                let message =
                    "The Slint live preview crashed! Please open a bug on the [Slint bug tracker](https://github.com/slint-ui/slint/issues)."
                        .to_string();
                eprintln!("{message}");

                let _ = preview_to_lsp_channel.send(common::PreviewToLspMessage::SendShowMessage {
                    message: lsp_types::ShowMessageParams {
                        typ: lsp_types::MessageType::ERROR,
                        message,
                    },
                });
            }
            Ok(())
        });

        *self.inner.borrow_mut() =
            Some(ChildProcessLspToPreviewInner { communication_handle, to_child, child });

        Ok(())
    }
}

impl Drop for ChildProcessLspToPreview {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.borrow_mut().take() {
            {
                let mut child = inner.child.lock().expect("Can lock the child");
                let _ = child.kill();
            }

            let _ = inner.communication_handle.join();
        }
    }
}

impl common::LspToPreview for ChildProcessLspToPreview {
    fn send(&self, message: &common::LspToPreviewMessage) {
        if self.preview_is_running() {
            let mut inner = self.inner.borrow_mut();
            let inner = inner.as_mut().unwrap();
            let Ok(message) = serde_json::to_string(message) else {
                return;
            };
            let _ = writeln!(inner.to_child, "{message}");
        } else if let common::LspToPreviewMessage::ShowPreview(_) = message {
            self.start_preview().unwrap();
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
        let _ = self.lsp_to_previews.get(&self.current_target.borrow()).unwrap().send(message);
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
    /// Creates a RemoteConfrolledPreviewToLsp connector.
    ///
    /// This means the applications lifetime is bound to the lifetime of the
    /// application's STDIN: We quit as soon as that gets fishy or closed.
    ///
    /// It also means we do not need to join the reader thread: The OS will clean
    /// that one up for us anyway.
    pub fn new() -> Self {
        let _ = Self::process_input();
        Self {}
    }

    fn process_input() -> std::thread::JoinHandle<std::result::Result<(), String>> {
        std::thread::spawn(move || -> Result<(), String> {
            let reader = std::io::BufReader::new(std::io::stdin().lock());
            for line in reader.lines() {
                let Ok(line) = line else {
                    let _ = slint::quit_event_loop();
                    return Ok(());
                };
                if let Ok(message) = serde_json::from_str(&line) {
                    slint::invoke_from_event_loop(move || {
                        preview::connector::lsp_to_preview(message);
                    })
                    .map_err(|e| e.to_string())?;
                }
            }
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

// This function overrides the default app menu and makes the "Quit" item merely hide the UI,
// as the life-cycle of this process is determined by the editor. The returned menuitem must
// be kept alive for the duration of the event loop, as otherwise muda crashes.
#[cfg(target_vendor = "apple")]
pub fn init_apple_platform() -> Result<(), i_slint_core::api::PlatformError> {
    let backend = i_slint_backend_winit::Backend::builder().with_default_menu_bar(false).build()?;

    slint::platform::set_platform(Box::new(backend)).map_err(|set_platform_err| {
        i_slint_core::api::PlatformError::from(set_platform_err.to_string())
    })?;

    Ok(())
}
