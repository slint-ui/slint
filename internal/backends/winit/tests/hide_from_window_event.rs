// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hiding a window from one of its own event handlers must not report that the window
//! is kept alive: the event loop holds a reference to it while dispatching the event.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slint::winit_030::{CustomApplicationHandler, EventResult, winit};

/// Part of the message the backend logs about a window kept alive after being hidden.
const REPORT: &str = "references to it still exist";

slint::slint! {
    export component App inherits Window {
        width: 100px;
        height: 100px;
    }
}

/// Hides the window from a winit event, like an application does on a click on the close button.
struct HideOnRedraw {
    hidden: Rc<Cell<bool>>,
}

impl CustomApplicationHandler for HideOnRedraw {
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _winit_window: Option<&winit::window::Window>,
        slint_window: Option<&slint::Window>,
        event: &winit::event::WindowEvent,
    ) -> EventResult {
        // Every platform delivers a redraw for a window that was just shown.
        if !self.hidden.get()
            && matches!(event, winit::event::WindowEvent::RedrawRequested)
            && let Some(slint_window) = slint_window
        {
            self.hidden.set(true);
            slint_window.hide().unwrap();
        }
        EventResult::Propagate
    }
}

fn main() {
    // Only Wayland destroys the window when it is hidden, and only then is there
    // something that could be kept alive. Take that path on every platform.
    unsafe { std::env::set_var("SLINT_DESTROY_WINDOW_ON_HIDE", "1") };

    let hidden = Rc::new(Cell::new(false));
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_custom_application_handler(HideOnRedraw { hidden: hidden.clone() })
        .select()
        .unwrap();

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

    let app = App::new().unwrap();
    app.show().unwrap();
    // The window is the only one, so the event loop returns once it is hidden.
    slint::run_event_loop().unwrap();

    assert!(hidden.get(), "the window was never hidden");
    let reports: Vec<_> =
        messages.take().into_iter().filter(|message| message.contains(REPORT)).collect();
    assert!(reports.is_empty(), "the window was reported as kept alive: {reports:?}");
}
