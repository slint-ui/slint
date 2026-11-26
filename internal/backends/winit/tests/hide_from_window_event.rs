// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hiding a window from one of its own event handlers must not report that the window
//! is kept alive: the event loop holds a reference to it while dispatching the event.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::{CustomApplicationHandler, EventResult, winit};

/// Part of the message the backend logs about a window kept alive after being hidden.
const REPORT: &str = "references to it still exist";

slint::slint! {
    #[edition = "1.20"]
    export component App inherits Window {
        width: 100px;
        height: 100px;
    }
}

/// Hides the window from its first winit event, as an application does on a close button click.
struct HideOnFirstEvent;

impl CustomApplicationHandler for HideOnFirstEvent {
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _winit_window: Option<&winit::window::Window>,
        slint_window: Option<&slint::Window>,
        event: &winit::event::WindowEvent,
    ) -> EventResult {
        if let Some(slint_window) = slint_window
            && slint_window.is_visible()
        {
            eprintln!("Hiding the window from {event:?}");
            slint_window.hide().unwrap();
        }
        EventResult::Propagate
    }
}

fn main() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(60));
        eprintln!("The event loop didn't return within a minute");
        std::process::exit(1);
    });

    // Only Wayland destroys the window when it is hidden, and only then is there
    // something that could be kept alive. Take that path on every platform.
    unsafe { std::env::set_var("SLINT_DESTROY_WINDOW_ON_HIDE", "1") };

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_custom_application_handler(HideOnFirstEvent)
        .select()
        .unwrap();

    let messages = Rc::new(RefCell::new(Vec::new()));
    let collect = messages.clone();
    i_slint_core::with_existing_context(|ctx| {
        ctx.set_log_message_handler(Some(Box::new(move |message| {
            collect.borrow_mut().push(message.message_arguments().to_string());
        })))
    })
    .expect("the backend selector set the platform");

    let app = App::new().unwrap();
    app.show().unwrap();
    // The window is the only one, so the event loop returns once it is hidden.
    slint::run_event_loop().unwrap();

    let reports: Vec<_> =
        messages.take().into_iter().filter(|message| message.contains(REPORT)).collect();
    assert!(reports.is_empty(), "the window was reported as kept alive: {reports:?}");
}
