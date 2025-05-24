// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore condvar

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};

use crate::common::{self, PreviewToLspMessage, SourceFileVersion};
use crate::preview;
use crate::ServerNotifier;
use slint_interpreter::ComponentHandle;
use std::future::Future;
use std::sync::{Condvar, LazyLock, Mutex};

struct ToPreviewInner {
    join_handle: std::thread::JoinHandle<std::result::Result<(), String>>,
    to_preview: std::process::ChildStdin,
}

pub struct ToPreview {
    inner: RefCell<Option<ToPreviewInner>>,
    preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
}

impl ToPreview {
    pub fn new(
        preview_to_lsp_channel: crossbeam_channel::Sender<common::PreviewToLspMessage>,
    ) -> Self {
        Self { inner: RefCell::new(None), preview_to_lsp_channel }
    }

    pub fn send(&self, message: &common::LspToPreviewMessage) {
        if self.preview_is_running() {
            let inner = self.inner.borrow();
            let inner = inner.as_ref().unwrap();

            let Ok(message) = serde_json::to_string(message).map_err(|e| e.to_string()) else {
                return;
            };

            let _ = write!(&inner.to_preview, "{message}",).map_err(|e| e.to_string());
        } else if let common::LspToPreviewMessage::ShowPreview(_) = message {
            self.start_preview().unwrap();
        }
    }

    fn preview_is_running(&self) -> bool {
        !(self.inner.borrow().as_ref().map(|i| i.join_handle.is_finished()).unwrap_or(true))
    }

    fn start_preview(&self) -> common::Result<()> {
        let mut child = std::process::Command::new(
            std::env::args_os().next().expect("I was started, so I should have this!"),
        )
        .args(["live-preview", "--remote-controlled"].iter())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

        let from_child = child.stdout.take().expect("Child has no stdout");
        let to_child = child.stdin.take().expect("Child has no stdin");

        let channel = self.preview_to_lsp_channel.clone();

        let join_handle = std::thread::spawn(move || -> Result<(), String> {
            let reader = std::io::BufReader::new(from_child);
            for line in reader.lines() {
                let line = line.map_err(|e| e.to_string())?;
                let message = serde_json::from_str(&line).map_err(|e| e.to_string())?;

                channel.send(message).map_err(|e| e.to_string())?;
            }
            Ok(())
        });

        *self.inner.borrow_mut() = Some(ToPreviewInner { join_handle, to_preview: to_child });

        Ok(())
    }
}

pub struct ToLsp {
    join_handle: Option<std::thread::JoinHandle<std::result::Result<(), String>>>,
}

impl Drop for ToLsp {
    fn drop(&mut self) {
        if let Some(jh) = self.join_handle.take() {
            jh.join().unwrap().unwrap();
        }
    }
}

impl ToLsp {
    pub fn new(channel: crossbeam_channel::Sender<common::LspToPreviewMessage>) -> Self {
        let join_handle = Self::process_input(channel);

        Self { join_handle: Some(join_handle) }
    }

    pub fn send(&self, message: &common::PreviewToLspMessage) {
        let Ok(message) = serde_json::to_string(message).map_err(|e| e.to_string()) else {
            return;
        };
        println!("{message}");
    }

    fn process_input(
        channel: crossbeam_channel::Sender<common::LspToPreviewMessage>,
    ) -> std::thread::JoinHandle<std::result::Result<(), String>> {
        std::thread::spawn(move || -> Result<(), String> {
            let reader = std::io::BufReader::new(std::io::stdin().lock());
            for line in reader.lines() {
                let line = line.map_err(|e| e.to_string())?;
                let message = serde_json::from_str(&line).map_err(|e| e.to_string())?;

                // eprintln!("    RECV: {message:?}");

                channel.send(message).map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }
}

#[derive(PartialEq, Debug)]
enum RequestedGuiEventLoopState {
    /// The UI event loop hasn't been started yet because no preview has been requested
    Uninitialized,
    /// The LSP thread requested the UI loop to start because a preview was requested,
    /// But the loop hasn't been started yet
    StartLoop,
    /// The Loop is now started so the LSP thread can start posting events
    LoopStarted,
    /// The LSP thread requested the application to be terminated
    QuitLoop,
    /// There was an error when initializing the UI thread
    InitializationError(String),
}

static GUI_EVENT_LOOP_NOTIFIER: LazyLock<Condvar> = LazyLock::new(Condvar::new);
static GUI_EVENT_LOOP_STATE_REQUEST: LazyLock<Mutex<RequestedGuiEventLoopState>> =
    LazyLock::new(|| Mutex::new(RequestedGuiEventLoopState::Uninitialized));

thread_local! {static CLI_ARGS: std::cell::OnceCell<crate::Cli> = Default::default();}

pub fn lsp_to_preview_message(message: common::LspToPreviewMessage) {
    fn ensure_ui_event_loop() -> Result<(), String> {
        let mut state_request = GUI_EVENT_LOOP_STATE_REQUEST.lock().unwrap();
        if *state_request == RequestedGuiEventLoopState::Uninitialized {
            *state_request = RequestedGuiEventLoopState::StartLoop;
            GUI_EVENT_LOOP_NOTIFIER.notify_one();
        }
        // We don't want to call post_event before the loop is properly initialized
        while *state_request == RequestedGuiEventLoopState::StartLoop {
            state_request = GUI_EVENT_LOOP_NOTIFIER.wait(state_request).unwrap();
        }

        if let RequestedGuiEventLoopState::InitializationError(err) = &*state_request {
            return Err(err.clone());
        }

        Ok(())
    }

    fn run_in_ui_thread<F: Future<Output = ()> + 'static>(
        create_future: impl Send + FnOnce() -> F + 'static,
    ) -> Result<(), String> {
        i_slint_core::api::invoke_from_event_loop(move || {
            slint::spawn_local(create_future()).unwrap();
        })
        .map_err(|e| e.to_string())
    }

    let loop_is_started = {
        *GUI_EVENT_LOOP_STATE_REQUEST.lock().unwrap() == RequestedGuiEventLoopState::LoopStarted
    };

    if matches!(message, common::LspToPreviewMessage::ShowPreview(_)) && !loop_is_started {
        if let Err(e) = ensure_ui_event_loop() {
            super::send_platform_error_notification(&e);
        }

        send_message_to_lsp(PreviewToLspMessage::RequestState { unused: true });
        return;
    }
    if loop_is_started {
        if let Err(e) = run_in_ui_thread(move || async move {
            super::lsp_to_preview_message_impl(message);
        }) {
            super::send_platform_error_notification(&e);
        }
    }
}

pub(super) fn open_ui_impl(
    preview_state: &mut preview::PreviewState,
) -> Result<(), slint::PlatformError> {
    let (default_style, show_preview_ui, fullscreen) = {
        let style = preview_state.config.style.clone();
        let style = if style.is_empty() {
            CLI_ARGS.with(|args| args.get().map(|a| a.style.clone()).unwrap_or_default())
        } else {
            style
        };
        let hide_ui = preview_state
            .config
            .hide_ui
            .or_else(|| CLI_ARGS.with(|args| args.get().map(|a| a.no_toolbar)))
            .unwrap_or(false);
        let fullscreen = CLI_ARGS.with(|args| args.get().map(|a| a.fullscreen).unwrap_or_default());
        (style, !hide_ui, fullscreen)
    };

    let experimental = std::env::var_os("SLINT_ENABLE_EXPERIMENTAL_FEATURES").is_some();

    let ui = match preview_state.ui.as_ref() {
        Some(ui) => ui,
        None => {
            let ui = crate::preview::ui::create_ui(default_style, experimental)?;
            super::send_telemetry(&mut [(
                "type".to_string(),
                serde_json::to_value("preview_opened").unwrap(),
            )]);
            preview_state.ui.insert(ui)
        }
    };

    preview_state.ui_is_visible = true;

    let api = ui.global::<crate::preview::ui::Api>();
    api.set_show_preview_ui(show_preview_ui);
    ui.window().set_fullscreen(fullscreen);
    ui.window().on_close_requested(|| {
        preview::PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            preview_state.ui_is_visible = false;
        });
        slint::CloseRequestResponse::HideWindow
    });
    Ok(())
}

/// Potentially called from other thread!

#[cfg(target_vendor = "apple")]
fn toggle_always_on_top() {
    i_slint_core::api::invoke_from_event_loop(move || {
        preview::PREVIEW_STATE.with(move |preview_state| {
            let preview_state = preview_state.borrow_mut();
            let Some(ui) = preview_state.ui.as_ref() else { return };
            let api = ui.global::<crate::preview::ui::Api>();
            api.set_always_on_top(!api.get_always_on_top());
        });
    })
    .unwrap(); // TODO: Handle Error
}

static SERVER_NOTIFIER: Mutex<Option<ServerNotifier>> = Mutex::new(None);

/// Give the UI thread a handle to send message back to the LSP thread
pub fn set_server_notifier(sender: ServerNotifier) {
    *SERVER_NOTIFIER.lock().unwrap() = Some(sender);
}

pub fn notify_diagnostics(
    diagnostics: HashMap<lsp_types::Url, (SourceFileVersion, Vec<lsp_types::Diagnostic>)>,
) -> Option<()> {
    let Some(sender) = SERVER_NOTIFIER.lock().unwrap().clone() else {
        return Some(());
    };

    for (url, (version, diagnostics)) in diagnostics {
        common::lsp_to_editor::notify_lsp_diagnostics(&sender, url, version, diagnostics)?;
    }
    Some(())
}

pub fn ask_editor_to_show_document(file: &str, selection: lsp_types::Range, take_focus: bool) {
    let Some(sender) = SERVER_NOTIFIER.lock().unwrap().clone() else {
        return;
    };
    let Ok(url) = lsp_types::Url::from_file_path(file) else { return };
    let fut =
        common::lsp_to_editor::send_show_document_to_editor(sender, url, selection, take_focus);
    slint_interpreter::spawn_local(fut).unwrap(); // Fire and forget.
}

pub fn send_message_to_lsp(message: PreviewToLspMessage) {
    // TODO: Replace this
    let Some(sender) = SERVER_NOTIFIER.lock().unwrap().clone() else {
        return;
    };
    // sender.send_message_to_lsp(message);
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
        if menu_event.id == close_id {
            close_ui();
        } else if menu_event.id == reload_id {
            preview::reload_preview();
        } else if menu_event.id == keep_on_top_id {
            toggle_always_on_top();
        }
    }));

    Ok((close_app_menu_item, reload_menu_item, keep_on_top_menu_item))
}
