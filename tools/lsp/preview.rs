/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#![allow(unused)]

use std::borrow::BorrowMut;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Wake;

struct FutureRunner {
    fut: Mutex<Option<Pin<Box<dyn Future<Output = ()>>>>>,
}

/// Safety: the future is only going to be run in the UI thread
unsafe impl Send for FutureRunner {}
/// Safety: the future is only going to be run in the UI thread
unsafe impl Sync for FutureRunner {}

impl Wake for FutureRunner {
    fn wake(self: Arc<Self>) {
        sixtyfps_rendering_backend_default::backend().post_event(Box::new(move || {
            let waker = self.clone().into();
            let mut cx = std::task::Context::from_waker(&waker);
            let mut fut_opt = self.fut.lock().unwrap();
            if let Some(fut) = &mut *fut_opt {
                match fut.as_mut().poll(&mut cx) {
                    std::task::Poll::Ready(_) => *fut_opt = None,
                    std::task::Poll::Pending => {}
                }
            }
        }))
    }
}

fn run_in_ui_thread(mut fut: Pin<Box<dyn Future<Output = ()>>>) {
    Arc::new(FutureRunner { fut: Mutex::new(Some(fut)) }).wake()
}

pub fn start_ui_event_loop() {
    sixtyfps_interpreter::run_event_loop();
}

pub fn load_preview(path: std::path::PathBuf) {
    run_in_ui_thread(Box::pin(async move { reload_preview(&path).await }));
}

async fn reload_preview(path: &std::path::Path) {
    let mut builder = sixtyfps_interpreter::ComponentCompiler::new();
    if let Some(compiled) = builder.build_from_path(path).await {
        #[derive(Default)]
        struct PreviewState {
            handle: Option<sixtyfps_interpreter::ComponentInstance>,
        }
        thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}
        PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            if let Some(handle) = preview_state.handle.take() {
                let window = handle.window();
                let handle = compiled.create_with_existing_window(window);
                handle.show();
                preview_state.handle = Some(handle);
            } else {
                let handle = compiled.create();
                handle.show();
                preview_state.handle = Some(handle);
            }
        });
    }
}
