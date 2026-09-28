// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! An application that keeps the winit window of a hidden window alive is told about it.

use std::cell::RefCell;
use std::rc::Rc;

use slint::winit_030::WinitWindowAccessor;

/// Part of the message the backend logs about a window kept alive after being hidden.
const REPORT: &str = "references to it still exist";

slint::slint! {
    export component App inherits Window {
        width: 100px;
        height: 100px;
    }
}

fn main() {
    // Only Wayland destroys the window when it is hidden, and only then is there
    // something that could be kept alive. Take that path on every platform.
    unsafe { std::env::set_var("SLINT_DESTROY_WINDOW_ON_HIDE", "1") };

    slint::BackendSelector::new().backend_name("winit".into()).select().unwrap();

    let messages = Rc::new(RefCell::new(Vec::new()));
    let collect = messages.clone();
    i_slint_core::with_global_context(
        || unreachable!("the backend selector set the platform"),
        |ctx| {
            ctx.set_log_message_handler(Some(Box::new(move |message| {
                collect.borrow_mut().push(message.message_arguments().to_string());
            })))
        },
    )
    .unwrap();

    let reported = messages.clone();
    slint::spawn_local(async move {
        let app = App::new().unwrap();
        app.show().unwrap();
        let kept_alive = app.window().winit_window().await.unwrap();

        app.window().hide().unwrap();

        // The report comes once the event loop is done with what hid the window, so read it
        // on the next iteration.
        slint::Timer::single_shot(std::time::Duration::default(), move || {
            let reports: Vec<_> =
                reported.take().into_iter().filter(|message| message.contains(REPORT)).collect();
            assert_eq!(reports.len(), 1, "expected one report, got {reports:?}");

            // Let go of the window while the event loop is still there to destroy it.
            drop(kept_alive);
            slint::quit_event_loop().unwrap();
        });
    })
    .unwrap();

    slint::run_event_loop_until_quit().unwrap();
}
