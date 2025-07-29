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
    fn send(&self, message: &common::LspToPreviewMessage) -> common::Result<()> {
        if self.preview_is_running() {
            let mut inner = self.inner.borrow_mut();
            let inner = inner.as_mut().unwrap();
            let message = serde_json::to_string(message).map_err(|e| e.to_string())?;
            writeln!(inner.to_child, "{message}")?;
        } else if let common::LspToPreviewMessage::ShowPreview(_) = message {
            self.start_preview().unwrap();
        }
        Ok(())
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
    fn send(&self, message: &common::LspToPreviewMessage) -> common::Result<()> {
        self.server_notifier.send_notification::<common::LspToPreviewMessage>(message.clone())
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
    fn send(&self, message: &common::LspToPreviewMessage) -> common::Result<()> {
        self.lsp_to_previews.get(&self.current_target.borrow()).unwrap().send(message)
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

#[cfg(target_vendor = "apple")]
fn toggle_always_on_top() {
    use slint::ComponentHandle;
    preview::PREVIEW_STATE.with_borrow_mut(move |preview_state| {
        let Some(ui) = preview_state.ui.as_ref() else { return };
        let api = ui.global::<crate::preview::ui::Api>();
        api.set_always_on_top(!api.get_always_on_top());
    });
}

// This function overrides the default app menu and makes the "Quit" item merely hide the UI,
// as the life-cycle of this process is determined by the editor. The returned menuitem must
// be kept alive for the duration of the event loop, as otherwise muda crashes.
#[cfg(target_vendor = "apple")]
pub fn init_apple_platform() -> Result<(), i_slint_core::api::PlatformError> {
    use muda::{accelerator, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

    let backend = i_slint_backend_winit::Backend::builder().with_default_menu_bar(false).build()?;

    slint::platform::set_platform(Box::new(backend)).map_err(|set_platform_err| {
        i_slint_core::api::PlatformError::from(set_platform_err.to_string())
    })?;

    let reload_menu_item = MenuItem::new(
        format!("Reload"),
        true,
        Some(accelerator::Accelerator::new(
            Some(accelerator::Modifiers::META),
            accelerator::Code::KeyR,
        )),
    );
    let keep_on_top_menu_item = CheckMenuItem::new(format!("Keep on Top"), true, false, None);

    let menu_bar = Menu::new();
    menu_bar.init_for_nsapp();
    let app_m = Submenu::new("App", true);
    let window_m = Submenu::new("&Window", true);
    menu_bar
        .append(&app_m)
        .and_then(|_| menu_bar.append(&window_m))
        .and_then(|_| {
            app_m.append_items(&[
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &reload_menu_item,
                &PredefinedMenuItem::quit(Some("Quit Slint Live-Preview")),
            ])
        })
        .and_then(|_| window_m.append_items(&[&keep_on_top_menu_item]))
        .map_err(|menu_bar_err| {
            i_slint_core::api::PlatformError::Other(menu_bar_err.to_string())
        })?;

    let reload_id = reload_menu_item.id().clone();
    let keep_on_top_id = keep_on_top_menu_item.id().clone();

    muda::MenuEvent::set_event_handler(Some(move |menu_event: muda::MenuEvent| {
        let reload_id = reload_id.clone();
        let keep_on_top_id = keep_on_top_id.clone();

        let _ = slint::invoke_from_event_loop(move || {
            if menu_event.id == reload_id {
                preview::reload_preview();
            } else if menu_event.id == keep_on_top_id {
                toggle_always_on_top();
            }
        });
    }));

    // Keep the menu items alive to prevent muda from crashing. The menu bar is a singleton, so this is an acceptable memory leak
    let _ = Box::leak(Box::new((reload_menu_item, keep_on_top_menu_item)));

    Ok(())
}
