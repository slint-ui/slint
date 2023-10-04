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
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
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
    if let Some(cache) = CONTENT_CACHE.get() {
        let mut cache = cache.lock().unwrap();
        cache.sender = None;
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
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        if cache.sender.is_some() {
            return; // UI is already up!
        }
        cache.sender = Some(sender.clone());
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
    let ui = preview_state.ui.get_or_insert_with(|| super::ui::PreviewUi::new().unwrap());
    ui.on_design_mode_changed(|design_mode| set_design_mode(design_mode));
    ui.window().on_close_requested(|| {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.sender = None;
        slint::CloseRequestResponse::HideWindow
    });
    ui.show().unwrap();
}

pub fn close_ui() {
    {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        if cache.sender.is_none() {
            return; // UI is already up!
        }
        cache.sender = None;
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
        reload_preview(component).await
    });
}

#[derive(Default)]
struct ContentCache {
    source_code: HashMap<PathBuf, String>,
    dependency: HashSet<PathBuf>,
    current: PreviewComponent,
    sender: Option<crate::ServerNotifier>, // Set while a UI is up!
    highlight: Option<(PathBuf, u32)>,
    design_mode: bool,
}

static CONTENT_CACHE: once_cell::sync::OnceCell<Mutex<ContentCache>> =
    once_cell::sync::OnceCell::new();

pub fn set_contents(path: &Path, content: String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    cache.source_code.insert(path.to_owned(), content);
    if cache.dependency.contains(path) {
        let current = cache.current.clone();
        let sender = cache.sender.clone();
        drop(cache);

        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.sender = sender;

        if cache.sender.is_some() {
            load_preview(current);
        }
    }
}

pub fn config_changed(style: &str, include_paths: &[PathBuf]) {
    if let Some(cache) = CONTENT_CACHE.get() {
        let mut cache = cache.lock().unwrap();
        let style = style.to_string();
        if cache.current.style != style || cache.current.include_paths != include_paths {
            cache.current.style = style;
            cache.current.include_paths = include_paths.to_vec();
            let current = cache.current.clone();
            let sender = cache.sender.clone();
            drop(cache);

            let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            cache.sender = sender;

            if cache.sender.is_some() {
                load_preview(current);
            }
        }
    };
}

/// If the file is in the cache, returns it.
/// In any was, register it as a dependency
fn get_file_from_cache(path: PathBuf) -> Option<String> {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let r = cache.source_code.get(&path).cloned();
    cache.dependency.insert(path);
    r
}

#[derive(Default)]
struct PreviewState {
    ui: Option<super::ui::PreviewUi>,
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

fn set_design_mode(enable: bool) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let Some(sender) = cache.sender.clone() else {
        return;
    };

    cache.design_mode = enable;

    configure_design_mode(enable, &sender);
    send_notification(
        &sender,
        if enable { "Design mode enabled." } else { "Design mode disabled." },
        Health::Ok,
    );
}

fn show_document_request_from_element_callback(
    file: &str,
    start_line: u32,
    start_column: u32,
    _end_line: u32,
    _end_column: u32,
) -> Option<lsp_types::ShowDocumentParams> {
    use lsp_types::{Position, Range, ShowDocumentParams, Url};

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

fn configure_design_mode(enabled: bool, sender: &crate::ServerNotifier) {
    let sender = sender.clone();
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
                        if file.is_empty() || start_column == 0 || end_column == 0 {
                            return;
                        }
                        let Some(params) = show_document_request_from_element_callback(
                            file,
                            start_line,
                            start_column,
                            end_line,
                            end_column,
                        ) else {
                            return;
                        };
                        let Ok(fut) =
                            sender.send_request::<lsp_types::request::ShowDocument>(params)
                        else {
                            return;
                        };
                        i_slint_core::future::spawn_local(fut).unwrap();
                    },
                ));
            }
        })
    });
}

async fn reload_preview(preview_component: PreviewComponent) {
    let (design_mode, sender) = {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.dependency.clear();
        cache.current = preview_component.clone();
        (cache.design_mode, cache.sender.clone())
    };
    let Some(sender) = sender else {
        return;
    };

    send_notification(&sender, "Loading Preview…", Health::Ok);

    let mut builder = slint_interpreter::ComponentCompiler::default();

    if !preview_component.style.is_empty() {
        builder.set_style(preview_component.style);
    }
    builder.set_include_paths(preview_component.include_paths);

    builder.set_file_loader(|path| {
        let path = path.to_owned();
        Box::pin(async move { get_file_from_cache(path).map(Result::Ok) })
    });

    let compiled = if let Some(mut from_cache) = get_file_from_cache(preview_component.path.clone())
    {
        if let Some(component) = &preview_component.component {
            from_cache =
                format!("{}\nexport component _Preview inherits {} {{ }}\n", from_cache, component);
        }
        builder.build_from_source(from_cache, preview_component.path).await
    } else {
        builder.build_from_path(preview_component.path).await
    };

    notify_diagnostics(builder.diagnostics(), &sender);

    if let Some(compiled) = compiled {
        PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            open_ui_impl(&mut preview_state);

            let shared_handle = preview_state.handle.clone();

            let factory = slint::ComponentFactory::new(move || {
                let instance = compiled.create().unwrap(); // TODO: Handle Error!
                if let Some((path, offset)) =
                    CONTENT_CACHE.get().and_then(|c| c.lock().unwrap().highlight.clone())
                {
                    instance.highlight(path, offset);
                }

                shared_handle.replace(Some(instance.clone_strong()));

                Some(instance)
            });
            preview_state.ui.as_ref().unwrap().set_preview_area(factory);
        });
        send_notification(&sender, "Preview Loaded", Health::Ok);
    } else {
        send_notification(&sender, "Preview not updated", Health::Error);
    }

    configure_design_mode(design_mode, &sender);

    CONTENT_CACHE.get_or_init(Default::default).lock().unwrap().sender.replace(sender);
}

fn notify_diagnostics(
    diagnostics: &[slint_interpreter::Diagnostic],
    sender: &crate::ServerNotifier,
) -> Option<()> {
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

fn send_notification(sender: &crate::ServerNotifier, arg: &str, health: Health) {
    sender
        .send_notification(
            ServerStatusNotification::METHOD.into(),
            ServerStatusParams { health, quiescent: false, message: Some(arg.into()) },
        )
        .unwrap_or_else(|e| eprintln!("Error sending notification: {:?}", e));
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn highlight(path: Option<PathBuf>, offset: u32) {
    let highlight = path.map(|x| (x, offset));
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

    if cache.highlight == highlight {
        return;
    }
    cache.highlight = highlight;

    if cache.highlight.as_ref().map_or(true, |(path, _)| cache.dependency.contains(path)) {
        run_in_ui_thread(move || async move {
            PREVIEW_STATE.with(|preview_state| {
                let preview_state = preview_state.borrow();
                let handle = preview_state.handle.borrow();
                if let (Some(cache), Some(handle)) = (CONTENT_CACHE.get(), &*handle) {
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
