// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore condvar

use crate::common;

use std::cell::RefCell;
use std::io::{BufRead as _, Write as _};

pub fn resource_url_mapper() -> Option<i_slint_compiler::ResourceUrlMapper> {
    None
}

struct NativeLspToPreviewInner {
    communication_handle: std::thread::JoinHandle<std::result::Result<(), String>>,
    to_child: std::process::ChildStdin,
}

pub struct NativeLspToPreview {
    inner: RefCell<Option<NativeLspToPreviewInner>>,
    preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
}

impl NativeLspToPreview {
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
            std::env::args_os().next().expect("I was started, so I should have this!"),
        )
        .args(["live-preview", "--remote-controlled"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

        let from_child = child.stdout.take().expect("Child has no stdout");
        let to_child = child.stdin.take().expect("Child has no stdin");

        let channel = self.preview_to_lsp_channel.clone();

        let communication_handle = std::thread::spawn(move || -> Result<(), String> {
            let reader = std::io::BufReader::new(from_child);
            for line in reader.lines() {
                let line = line.map_err(|e| e.to_string())?;
                if let Ok(message) = serde_json::from_str(&line) {
                    eprintln!("PREVIEW -> LSP: {message:?}");
                    channel.send(message).map_err(|e| e.to_string())?;
                }
            }
            let _ = child.wait();
            Ok(())
        });

        *self.inner.borrow_mut() = Some(NativeLspToPreviewInner { communication_handle, to_child });

        Ok(())
    }
}

impl Drop for NativeLspToPreview {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.borrow_mut().take() {
            let _ = inner.communication_handle.join();
        }
    }
}

impl common::LspToPreview for NativeLspToPreview {
    fn send(&self, message: &common::LspToPreviewMessage) -> common::Result<()> {
        if self.preview_is_running() {
            let mut inner = self.inner.borrow_mut();
            let inner = inner.as_mut().unwrap();
            let message = serde_json::to_string(message).map_err(|e| e.to_string())?;
            writeln!(inner.to_child, "{message}")?;
            eprintln!("LSP -> PREVIEW: {message}");
        } else if let common::LspToPreviewMessage::ShowPreview(_) = message {
            eprintln!("Starting preview");
            self.start_preview().unwrap();
        } else {
            eprintln!("Ignoring LSP -> PREVIEW communication attempt");
        }
        Ok(())
    }
}

pub struct NativePreviewToLsp {}

impl Default for NativePreviewToLsp {
    fn default() -> Self {
        Self::new()
    }
}

impl NativePreviewToLsp {
    pub fn new() -> Self {
        let _ = Self::process_input();
        Self {}
    }

    fn process_input() -> std::thread::JoinHandle<std::result::Result<(), String>> {
        std::thread::spawn(move || -> Result<(), String> {
            let reader = std::io::BufReader::new(std::io::stdin().lock());
            for line in reader.lines() {
                let line = line.map_err(|e| e.to_string())?;
                if let Ok(message) = serde_json::from_str(&line) {
                    slint::invoke_from_event_loop(move || {
                        crate::preview::connector::lsp_to_preview(message);
                    })
                    .map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        })
    }
}

impl common::PreviewToLsp for NativePreviewToLsp {
    fn send(&self, message: &common::PreviewToLspMessage) -> common::Result<()> {
        let message = serde_json::to_string(message).map_err(|e| e.to_string())?;
        println!("{message}");
        Ok(())
    }
}

#[cfg(target_vendor = "apple")]
fn toggle_always_on_top() {
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
fn init_apple_platform(
) -> Result<(muda::MenuItem, muda::MenuItem, muda::CheckMenuItem), i_slint_core::api::PlatformError>
{
    use muda::{accelerator, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

    let backend = i_slint_backend_winit::Backend::builder().with_default_menu_bar(false).build()?;

    slint::platform::set_platform(Box::new(backend)).map_err(|set_platform_err| {
        i_slint_core::api::PlatformError::from(set_platform_err.to_string())
    })?;

    let process_name = objc2_foundation::NSProcessInfo::processInfo().processName().to_string();
    let close_app_menu_item = MenuItem::new(
        format!("Close {process_name}"),
        true,
        Some(accelerator::Accelerator::new(
            Some(accelerator::Modifiers::META),
            accelerator::Code::KeyW,
        )),
    );
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
                &close_app_menu_item,
            ])
        })
        .and_then(|_| window_m.append_items(&[&keep_on_top_menu_item]))
        .map_err(|menu_bar_err| {
            i_slint_core::api::PlatformError::Other(menu_bar_err.to_string())
        })?;

    let close_id = close_app_menu_item.id().clone();
    let reload_id = reload_menu_item.id().clone();
    let keep_on_top_id = keep_on_top_menu_item.id().clone();

    muda::MenuEvent::set_event_handler(Some(move |menu_event: muda::MenuEvent| {
        let close_id = close_id.clone();
        let reload_id = reload_id.clone();
        let keep_on_top_id = keep_on_top_id.clone();

        let _ = slint::invoke_from_event_loop(move || {
            if menu_event.id == close_id {
                close_ui();
            } else if menu_event.id == reload_id {
                preview::reload_preview();
            } else if menu_event.id == keep_on_top_id {
                toggle_always_on_top();
            }
        });
    }));

    Ok((close_app_menu_item, reload_menu_item, keep_on_top_menu_item))
}
