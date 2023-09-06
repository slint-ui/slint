// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore condvar

use crate::common::PreviewComponent;
use crate::lsp_ext::{Health, ServerStatusNotification, ServerStatusParams};
use crate::ServerNotifier;

use lsp_types::notification::Notification;
use once_cell::sync::Lazy;
use slint_interpreter::ComponentHandle;
use std::cell::RefCell;
use std::collections::HashMap;
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
    LoopStated,
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
                    *state_request = RequestedGuiEventLoopState::LoopStated;
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

    // Make sure then sender channel gets dropped
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
        *s = Some(sender.clone())
    }

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            open_ui_impl(&mut preview_state)
        });
    })
    .unwrap();
}

fn open_ui_impl(preview_state: &mut PreviewState) {
    // TODO: Handle Error!
    let ui = preview_state.ui.get_or_insert_with(|| super::ui::create_ui().unwrap());
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
            return; // UI is already up!
        }
        cache.ui_is_visible = false;

        let mut sender = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap();
        *sender = None;
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
        super::reload_preview(component).await
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

pub fn send_status(message: &str, health: Health) {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return;
    };

    sender
        .send_notification(
            ServerStatusNotification::METHOD.into(),
            ServerStatusParams { health, quiescent: false, message: Some(message.into()) },
        )
        .unwrap_or_else(|e| eprintln!("Error sending notification: {:?}", e));
}

pub fn ask_editor_to_show_document(
    file: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return;
    };

    let Some(params) = show_document_request_from_element_callback(
        file,
        start_line,
        start_column,
        end_line,
        end_column,
    ) else {
        return;
    };
    let Ok(fut) = sender.send_request::<lsp_types::request::ShowDocument>(params) else {
        return;
    };
    i_slint_core::future::spawn_local(fut).unwrap();
}

fn show_document_request_from_element_callback(
    file: &str,
    start_line: u32,
    start_column: u32,
    _end_line: u32,
    end_column: u32,
) -> Option<lsp_types::ShowDocumentParams> {
    use lsp_types::{Position, Range, ShowDocumentParams, Url};

    if file.is_empty() || start_column == 0 || end_column == 0 {
        return None;
    }

    let start_pos = Position::new(start_line.saturating_sub(1), start_column.saturating_sub(1));
    // let end_pos = Position::new(end_line.saturating_sub(1), end_column.saturating_sub(1));
    // Place the cursor at the start of the range and do not mark up the entire range!
    let selection = Some(Range::new(start_pos, start_pos));

    Url::from_file_path(file).ok().map(|uri| ShowDocumentParams {
        uri,
        external: Some(false),
        take_focus: Some(true),
        selection,
    })
}

pub fn configure_design_mode(enabled: bool) {
    run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            let handle = preview_state.handle.borrow();
            if let Some(handle) = &*handle {
                handle.set_design_mode(enabled);

                handle.on_element_selected(Box::new(
                    move |file: &str,
                          start_line: u32,
                          start_column: u32,
                          end_line: u32,
                          end_column: u32| {
                        ask_editor_to_show_document(
                            file,
                            start_line,
                            start_column,
                            end_line,
                            end_column,
                        );
                    },
                ));
            }
        })
    });
}

/// This runs `set_preview_factory` in the UI thread
pub fn update_preview_area(compiled: slint_interpreter::ComponentDefinition) {
    PREVIEW_STATE.with(|preview_state| {
        let mut preview_state = preview_state.borrow_mut();

        open_ui_impl(&mut preview_state);

        let shared_handle = preview_state.handle.clone();

        super::set_preview_factory(
            preview_state.ui.as_ref().unwrap(),
            compiled,
            Box::new(move |instance| {
                shared_handle.replace(Some(instance));
            }),
        );
    });
}

pub fn notify_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) -> Option<()> {
    let Some(sender) = SERVER_NOTIFIER.get_or_init(Default::default).lock().unwrap().clone() else {
        return Some(());
    };

    let mut lsp_diags: HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>> = Default::default();
    for d in diagnostics {
        if d.source_file().map_or(true, |f| f.is_relative()) {
            continue;
        }
        let uri = lsp_types::Url::from_file_path(d.source_file().unwrap()).unwrap();
        lsp_diags.entry(uri).or_default().push(crate::util::to_lsp_diag(d));
    }

    for (uri, diagnostics) in lsp_diags {
        sender
            .send_notification(
                "textDocument/publishDiagnostics".into(),
                lsp_types::PublishDiagnosticsParams { uri, diagnostics, version: None },
            )
            .ok()?;
    }
    Some(())
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn highlight(path: Option<PathBuf>, offset: u32) {
    let highlight = path.map(|x| (x, offset));
    let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

    if cache.highlight == highlight {
        return;
    }
    cache.highlight = highlight;

    if cache.highlight.as_ref().map_or(true, |(path, _)| cache.dependency.contains(path)) {
        run_in_ui_thread(move || async move {
            PREVIEW_STATE.with(|preview_state| {
                let preview_state = preview_state.borrow();
                let handle = preview_state.handle.borrow();
                if let (Some(cache), Some(handle)) = (super::CONTENT_CACHE.get(), &*handle) {
                    if let Some((path, offset)) = cache.lock().unwrap().highlight.clone() {
                        handle.highlight(path, offset);
                    } else {
                        handle.highlight(PathBuf::default(), 0);
                    }
                }
            })
        })
    }
}
