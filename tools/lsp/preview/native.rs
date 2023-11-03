// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore condvar

use crate::common::PreviewComponent;
use crate::lsp_ext::Health;
use crate::ServerNotifier;

use once_cell::sync::Lazy;
use slint_interpreter::ComponentHandle;
use std::cell::RefCell;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
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

fn run_in_ui_thread<F: Future<Output = ()> + 'static>(
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

pub fn start_ui_event_loop() {
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

    i_slint_backend_selector::with_platform(|b| {
        b.set_event_loop_quit_on_last_window_closed(false);
        b.run_event_loop()
    })
    .unwrap();
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

    let default_style = {
        let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        if cache.ui_is_visible {
            return; // UI is already up!
        }
        cache.ui_is_visible = true;

        let mut s = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap();
        *s = Some(sender.clone());

        cache.default_style.clone()
    };

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            open_ui_impl(&mut preview_state, default_style);
        });
    })
    .unwrap();
}

fn open_ui_impl(preview_state: &mut PreviewState, default_style: String) {
    // TODO: Handle Error!
    let ui = preview_state.ui.get_or_insert_with(|| super::ui::create_ui(default_style).unwrap());
    ui.window().on_close_requested(|| {
        let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.ui_is_visible = false;

        let mut sender = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap();
        *sender = None;

        slint::CloseRequestResponse::HideWindow
    });
    ui.show().unwrap();
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
        PREVIEW_STATE.with(move |preview_state| {
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

pub fn load_preview(component: PreviewComponent) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static PENDING_EVENTS: AtomicU32 = AtomicU32::new(0);
    if PENDING_EVENTS.load(Ordering::SeqCst) > 0 {
        return;
    }
    PENDING_EVENTS.fetch_add(1, Ordering::SeqCst);
    run_in_ui_thread(move || async move {
        PENDING_EVENTS.fetch_sub(1, Ordering::SeqCst);

        set_current_style(super::reload_preview(component, get_current_style()).await);
    });
}

static SERVER_NOTIFIER: std::sync::OnceLock<Mutex<Option<ServerNotifier>>> =
    std::sync::OnceLock::new();

#[derive(Default)]
struct PreviewState {
    ui: Option<super::ui::PreviewUi>,
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

pub fn notify_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) -> Option<()> {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return Some(());
    };

    let lsp_diags = crate::preview::convert_diagnostics(diagnostics);

    for (url, diagnostics) in lsp_diags {
        crate::preview::notify_lsp_diagnostics(&sender, url, diagnostics)?;
    }
    Some(())
}

fn set_current_style(style: String) {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow_mut();
        if let Some(ui) = &preview_state.ui {
            ui.set_current_style(style.into())
        }
    });
}

fn get_current_style() -> String {
    PREVIEW_STATE.with(|preview_state| -> String {
        let preview_state = preview_state.borrow();
        if let Some(ui) = &preview_state.ui {
            ui.get_current_style().as_str().to_string()
        } else {
            String::new()
        }
    })
}

pub fn set_busy(busy: bool) {
    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow_mut();
            if let Some(ui) = &preview_state.ui {
                ui.set_is_busy(busy);
            }
        });
    })
    .unwrap();
}

pub fn send_status(message: &str, health: Health) {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return;
    };

    crate::preview::send_status_notification(&sender, message, health)
}

pub fn ask_editor_to_show_document(
    file: String,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return;
    };

    let fut = crate::send_show_document_to_editor(
        sender,
        file,
        start_line,
        start_column,
        end_line,
        end_column,
    );

    slint_interpreter::spawn_local(fut).unwrap(); // Fire and forget.
}

pub fn configure_design_mode(enabled: bool) {
    run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            let handle = preview_state.handle.borrow();
            if let Some(handle) = &*handle {
                super::configure_handle_for_design_mode(&handle, enabled);
            }
        })
    });
}

/// This runs `set_preview_factory` in the UI thread
pub fn update_preview_area(compiled: slint_interpreter::ComponentDefinition, design_mode: bool) {
    let default_style = {
        let cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.default_style.clone()
    };

    PREVIEW_STATE.with(|preview_state| {
        let mut preview_state = preview_state.borrow_mut();

        open_ui_impl(&mut preview_state, default_style);

        let shared_handle = preview_state.handle.clone();

        super::set_preview_factory(
            preview_state.ui.as_ref().unwrap(),
            compiled,
            Box::new(move |instance| {
                shared_handle.replace(Some(instance));
            }),
            design_mode,
        );
    });
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn update_highlight(path: PathBuf, offset: u32) {
    run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            let handle = preview_state.handle.borrow();
            if let Some(handle) = &*handle {
                handle.highlight(path, offset);
            }
        })
    })
}
