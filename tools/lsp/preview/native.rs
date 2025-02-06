// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore condvar

use std::collections::HashMap;

use super::PreviewState;
use crate::common::{PreviewToLspMessage, SourceFileVersion};
use crate::ServerNotifier;
use slint_interpreter::ComponentHandle;
use std::future::Future;
use std::sync::{Condvar, LazyLock, Mutex};

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

pub fn run_in_ui_thread<F: Future<Output = ()> + 'static>(
    create_future: impl Send + FnOnce() -> F + 'static,
) -> Result<(), String> {
    // Wake up the main thread to start the event loop, if possible
    {
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
    }

    i_slint_core::api::invoke_from_event_loop(move || {
        slint::spawn_local(create_future()).unwrap();
    })
    .map_err(|e| e.to_string())
}

/// This is the main entry for the Slint event loop. It runs on the main thread,
/// but only runs the event loop if a preview is requested to avoid potential
/// crash so that the LSP works without preview in that case.
pub fn start_ui_event_loop(cli_args: crate::Cli) {
    CLI_ARGS.with(|f| f.set(cli_args).ok());

    // NOTE: the result here must be kept alive for Apple platforms, as in the Ok case it holds a MenuItem
    // that must be kept alive.
    let loop_init_result;

    {
        let mut state_requested = GUI_EVENT_LOOP_STATE_REQUEST.lock().unwrap();

        // Wait until we either quit, or the LSP thread request to start the loop
        while *state_requested == RequestedGuiEventLoopState::Uninitialized {
            state_requested = GUI_EVENT_LOOP_NOTIFIER.wait(state_requested).unwrap();
        }

        if *state_requested == RequestedGuiEventLoopState::QuitLoop {
            return;
        }

        if *state_requested == RequestedGuiEventLoopState::StartLoop {
            #[cfg(target_vendor = "apple")]
            {
                // This can only be run once, as the event loop can't be restarted on macOS
                loop_init_result = init_apple_platform();
            }
            #[cfg(not(target_vendor = "apple"))]
            {
                // make sure the backend is initialized
                loop_init_result = i_slint_backend_selector::with_platform(|_| Ok(()));
            }
            match loop_init_result {
                Ok(_) => {}
                Err(err) => {
                    *state_requested =
                        RequestedGuiEventLoopState::InitializationError(err.to_string());
                    GUI_EVENT_LOOP_NOTIFIER.notify_one();
                    while *state_requested != RequestedGuiEventLoopState::QuitLoop {
                        state_requested = GUI_EVENT_LOOP_NOTIFIER.wait(state_requested).unwrap();
                    }
                    return;
                }
            };

            // Send an event so that once the loop is started, we notify the LSP thread that it can send more events
            i_slint_core::api::invoke_from_event_loop(|| {
                let mut state_request = GUI_EVENT_LOOP_STATE_REQUEST.lock().unwrap();
                if *state_request == RequestedGuiEventLoopState::StartLoop {
                    *state_request = RequestedGuiEventLoopState::LoopStarted;
                    GUI_EVENT_LOOP_NOTIFIER.notify_one();
                }
            })
            .unwrap();
        }
    }

    let loop_result = slint::run_event_loop_until_quit();
    if let Err(err) = loop_result {
        let mut state_requested = GUI_EVENT_LOOP_STATE_REQUEST.lock().unwrap();
        match *state_requested {
            RequestedGuiEventLoopState::InitializationError(_)
            | RequestedGuiEventLoopState::Uninitialized => unreachable!(),
            RequestedGuiEventLoopState::QuitLoop => return,
            RequestedGuiEventLoopState::StartLoop | RequestedGuiEventLoopState::LoopStarted => {
                *state_requested = RequestedGuiEventLoopState::InitializationError(err.to_string());
            }
        }
        GUI_EVENT_LOOP_NOTIFIER.notify_one();
        while *state_requested != RequestedGuiEventLoopState::QuitLoop {
            state_requested = GUI_EVENT_LOOP_NOTIFIER.wait(state_requested).unwrap();
        }
    }
}

pub fn quit_ui_event_loop() {
    // Wake up the main thread, in case it wasn't woken up earlier. If it wasn't, then don't request
    // a start of the event loop.
    {
        let mut state_request = GUI_EVENT_LOOP_STATE_REQUEST.lock().unwrap();
        *state_request = RequestedGuiEventLoopState::QuitLoop;
        GUI_EVENT_LOOP_NOTIFIER.notify_one();
    }

    close_ui();

    let _ = i_slint_core::api::quit_event_loop();

    // Make sure then sender channel gets dropped, otherwise the lsp thread will never quit
    *SERVER_NOTIFIER.lock().unwrap() = None
}

pub(super) fn open_ui_impl(preview_state: &mut PreviewState) -> Result<(), slint::PlatformError> {
    let (default_style, show_preview_ui, fullscreen) = {
        let cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        let style = cache.config.style.clone();
        let style = if style.is_empty() {
            CLI_ARGS.with(|args| args.get().map(|a| a.style.clone()).unwrap_or_default())
        } else {
            style
        };
        let hide_ui = cache
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
            let ui = super::ui::create_ui(default_style, experimental)?;
            preview_state.ui.insert(ui)
        }
    };

    super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap().ui_is_visible = true;

    let api = ui.global::<crate::preview::ui::Api>();
    api.set_show_preview_ui(show_preview_ui);
    ui.window().set_fullscreen(fullscreen);
    ui.window().on_close_requested(|| {
        let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.ui_is_visible = false;
        slint::CloseRequestResponse::HideWindow
    });
    Ok(())
}

pub fn close_ui() {
    {
        let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        if !cache.ui_is_visible {
            return; // UI is already down!
        }
        cache.ui_is_visible = false;
    }

    i_slint_core::api::invoke_from_event_loop(move || {
        super::PREVIEW_STATE.with(move |preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            close_ui_impl(&mut preview_state)
        });
    })
    .unwrap(); // TODO: Handle Error
}

fn close_ui_impl(preview_state: &mut PreviewState) {
    let ui = preview_state.ui.take();
    if let Some(ui) = ui {
        ui.hide().unwrap();
    }
}

#[cfg(target_vendor = "apple")]
fn toggle_always_on_top() {
    i_slint_core::api::invoke_from_event_loop(move || {
        super::PREVIEW_STATE.with(move |preview_state| {
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
        crate::common::lsp_to_editor::notify_lsp_diagnostics(&sender, url, version, diagnostics)?;
    }
    Some(())
}

pub fn ask_editor_to_show_document(file: &str, selection: lsp_types::Range, take_focus: bool) {
    let Some(sender) = SERVER_NOTIFIER.lock().unwrap().clone() else {
        return;
    };
    let Ok(url) = lsp_types::Url::from_file_path(file) else { return };
    let fut = crate::common::lsp_to_editor::send_show_document_to_editor(
        sender, url, selection, take_focus,
    );
    slint_interpreter::spawn_local(fut).unwrap(); // Fire and forget.
}

pub fn send_message_to_lsp(message: PreviewToLspMessage) {
    let Some(sender) = SERVER_NOTIFIER.lock().unwrap().clone() else {
        return;
    };
    sender.send_message_to_lsp(message);
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
            super::reload_preview();
        } else if menu_event.id == keep_on_top_id {
            toggle_always_on_top();
        }
    }));

    Ok((close_app_menu_item, reload_menu_item, keep_on_top_menu_item))
}
