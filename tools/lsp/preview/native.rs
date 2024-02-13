// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore condvar

use super::PreviewState;
use crate::lsp_ext::Health;
use crate::ServerNotifier;
use once_cell::sync::Lazy;
use slint_interpreter::ComponentHandle;
use std::future::Future;
use std::sync::{Condvar, Mutex};

#[derive(PartialEq)]
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
}

static GUI_EVENT_LOOP_NOTIFIER: Lazy<Condvar> = Lazy::new(Condvar::new);
static GUI_EVENT_LOOP_STATE_REQUEST: Lazy<Mutex<RequestedGuiEventLoopState>> =
    Lazy::new(|| Mutex::new(RequestedGuiEventLoopState::Uninitialized));

thread_local! {static CLI_ARGS: std::cell::OnceCell<crate::Cli> = Default::default();}

pub fn run_in_ui_thread<F: Future<Output = ()> + 'static>(
    create_future: impl Send + FnOnce() -> F + 'static,
) {
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
    }
    i_slint_core::api::invoke_from_event_loop(move || {
        i_slint_core::future::spawn_local(create_future()).unwrap();
    })
    .unwrap();
}

pub fn start_ui_event_loop(cli_args: crate::Cli) {
    CLI_ARGS.with(|f| f.set(cli_args).ok());

    {
        let mut state_requested = GUI_EVENT_LOOP_STATE_REQUEST.lock().unwrap();

        while *state_requested == RequestedGuiEventLoopState::Uninitialized {
            state_requested = GUI_EVENT_LOOP_NOTIFIER.wait(state_requested).unwrap();
        }

        if *state_requested == RequestedGuiEventLoopState::QuitLoop {
            return;
        }

        if *state_requested == RequestedGuiEventLoopState::StartLoop {
            // make sure the backend is initialized
            i_slint_backend_selector::with_platform(|_| Ok(())).unwrap();
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

    slint::run_event_loop_until_quit().unwrap();
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

    // Make sure then sender channel gets dropped.
    if let Some(sender) = SERVER_NOTIFIER.get() {
        let mut sender = sender.lock().unwrap();
        *sender = None;
    };
}

pub fn open_ui(sender: &ServerNotifier) {
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
    }

    {
        let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        if cache.ui_is_visible {
            return; // UI is already up!
        }
        cache.ui_is_visible = true;

        let mut s = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap();
        *s = Some(sender.clone());
    };

    i_slint_core::api::invoke_from_event_loop(move || {
        super::PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();

            open_ui_impl(&mut preview_state);
        });
    })
    .unwrap();
}

pub(super) fn open_ui_impl(preview_state: &mut PreviewState) {
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

    // TODO: Handle Error!
    let experimental = std::env::var_os("SLINT_ENABLE_EXPERIMENTAL_FEATURES")
        .map(|s| !s.is_empty() && s != "0")
        .unwrap_or(false);

    let ui = preview_state
        .ui
        .get_or_insert_with(|| super::ui::create_ui(default_style, experimental).unwrap());
    ui.set_show_preview_ui(show_preview_ui);
    ui.window().set_fullscreen(fullscreen);
    ui.window().on_close_requested(|| {
        let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.ui_is_visible = false;

        let mut sender = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap();
        *sender = None;

        slint::CloseRequestResponse::HideWindow
    });
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

static SERVER_NOTIFIER: std::sync::OnceLock<Mutex<Option<ServerNotifier>>> =
    std::sync::OnceLock::new();

pub fn notify_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) -> Option<()> {
    super::set_diagnostics(diagnostics);

    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return Some(());
    };

    let lsp_diags = crate::preview::convert_diagnostics(diagnostics);

    for (url, diagnostics) in lsp_diags {
        crate::common::lsp_to_editor::notify_lsp_diagnostics(&sender, url, diagnostics)?;
    }
    Some(())
}

pub fn send_status(message: &str, health: Health) {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return;
    };

    crate::common::lsp_to_editor::send_status_notification(&sender, message, health)
}

pub fn ask_editor_to_show_document(file: &str, selection: lsp_types::Range) {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return;
    };
    let Ok(url) = lsp_types::Url::from_file_path(file) else { return };
    let fut = crate::common::lsp_to_editor::send_show_document_to_editor(sender, url, selection);
    slint_interpreter::spawn_local(fut).unwrap(); // Fire and forget.
}

pub fn send_message_to_lsp(message: crate::common::PreviewToLspMessage) {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return;
    };
    sender.send_message_to_lsp(message);
}
