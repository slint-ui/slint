// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

extern crate alloc;

use std::rc::Rc;

use i_slint_core::window::WindowInner;

mod glwindow;
use glwindow::*;
mod glcontext;
use glcontext::*;
pub(crate) mod event_loop;
mod renderer {
    pub mod femtovg;
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_input_helper;

mod stylemetrics;

#[cfg(target_arch = "wasm32")]
pub fn create_gl_window_with_canvas_id(canvas_id: String) -> Rc<WindowInner> {
    i_slint_core::window::WindowInner::new(|window| GLWindow::new(window, canvas_id))
}

#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() {}

pub type NativeWidgets = ();
pub type NativeGlobals = (stylemetrics::NativeStyleMetrics, ());
pub mod native_widgets {
    pub use super::stylemetrics::NativeStyleMetrics;
}
pub const HAS_NATIVE_STYLE: bool = false;

pub use stylemetrics::native_style_metrics_deinit;
pub use stylemetrics::native_style_metrics_init;

pub struct Backend;
impl i_slint_core::backend::Backend for Backend {
    fn create_window(&'static self) -> Rc<WindowInner> {
        i_slint_core::window::WindowInner::new(|window| {
            GLWindow::new(
                window,
                #[cfg(target_arch = "wasm32")]
                "canvas".into(),
            )
        })
    }

    fn run_event_loop(&'static self, behavior: i_slint_core::backend::EventLoopQuitBehavior) {
        crate::event_loop::run(behavior);
    }

    fn quit_event_loop(&'static self) {
        crate::event_loop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(crate::event_loop::CustomEvent::Exit).ok();
        })
    }

    fn register_font_from_memory(
        &'static self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::renderer::femtovg::Renderer::register_font_from_memory(data)
    }

    fn register_font_from_path(
        &'static self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self::renderer::femtovg::Renderer::register_font_from_path(path)
    }

    fn post_event(&'static self, event: Box<dyn FnOnce() + Send>) {
        let e = crate::event_loop::CustomEvent::UserEvent(event);
        #[cfg(not(target_arch = "wasm32"))]
        crate::event_loop::GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap().send_event(e);
        #[cfg(target_arch = "wasm32")]
        {
            crate::event_loop::GLOBAL_PROXY.with(|global_proxy| {
                let mut maybe_proxy = global_proxy.borrow_mut();
                let proxy = maybe_proxy.get_or_insert_with(Default::default);
                // Calling send_event is usually done by winit at the bottom of the stack,
                // in event handlers, and thus winit might decide to process the event
                // immediately within that stack.
                // To prevent re-entrancy issues that might happen by getting the application
                // event processed on top of the current stack, set winit in Poll mode so that
                // events are queued and process on top of a clean stack during a requested animation
                // frame a few moments later.
                // This also allows batching multiple post_event calls and redraw their state changes
                // all at once.
                proxy.send_event(crate::event_loop::CustomEvent::WakeEventLoopWorkaround);
                proxy.send_event(e);
            });
        }
    }
}
