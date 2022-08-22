// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
The backend is the abstraction for crates that need to do the actual drawing and event loop
*/

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;

#[cfg(feature = "std")]
use once_cell::sync::OnceCell;

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use crate::unsafe_single_core::OnceCell;
use crate::window::PlatformWindow;

#[derive(Copy, Clone)]
/// Behavior describing how the event loop should terminate.
pub enum EventLoopQuitBehavior {
    /// Terminate the event loop when the last window was closed.
    QuitOnLastWindowClosed,
    /// Keep the event loop running until [`Backend::quit_event_loop()`] is called.
    QuitOnlyExplicitly,
}

/// Interface implemented by back-ends
pub trait Backend: Send + Sync {
    /// Instantiate a window for a component.
    fn create_window(&self) -> Rc<dyn PlatformWindow>;

    /// Spins an event loop and renders the visible windows.
    fn run_event_loop(&self, _behavior: EventLoopQuitBehavior) {
        unimplemented!()
    }

    /// Exits the event loop.
    fn quit_event_loop(&self) {
        unimplemented!()
    }

    /// Send an user event to from another thread that should be run in the GUI event loop
    fn post_event(&'static self, _event: Box<dyn FnOnce() + Send>) {
        unimplemented!()
    }

    fn duration_since_start(&'static self) -> core::time::Duration {
        #[cfg(feature = "std")]
        {
            let the_beginning = *INITIAL_INSTANT.get_or_init(instant::Instant::now);
            instant::Instant::now() - the_beginning
        }
        #[cfg(not(feature = "std"))]
        core::time::Duration::ZERO
    }

    /// Sends the given text into the system clipboard
    fn set_clipboard_text(&'static self, _text: &str) {}
    /// Returns a copy of text stored in the system clipboard, if any.
    fn clipboard_text(&'static self) -> Option<String> {
        None
    }
}

#[cfg(feature = "std")]
static INITIAL_INSTANT: once_cell::sync::OnceCell<instant::Instant> =
    once_cell::sync::OnceCell::new();

#[cfg(feature = "std")]
impl std::convert::From<crate::animations::Instant> for instant::Instant {
    fn from(our_instant: crate::animations::Instant) -> Self {
        let the_beginning = *INITIAL_INSTANT.get_or_init(instant::Instant::now);
        the_beginning + core::time::Duration::from_millis(our_instant.0)
    }
}

static PRIVATE_BACKEND_INSTANCE: OnceCell<Box<dyn Backend + 'static>> = OnceCell::new();

pub fn instance() -> Option<&'static dyn Backend> {
    use core::ops::Deref;
    PRIVATE_BACKEND_INSTANCE.get().map(|backend_box| backend_box.deref())
}

pub fn instance_or_init(
    factory_fn: impl FnOnce() -> Box<dyn Backend + 'static>,
) -> &'static dyn Backend {
    use core::ops::Deref;
    PRIVATE_BACKEND_INSTANCE.get_or_init(factory_fn).deref()
}

/// Fire timer events and update animations
///
/// This function should be called before rendering or processing input event.
/// It should basically be called on every iteration of the event loop.
pub fn update_timers_and_animations() {
    crate::timers::TimerList::maybe_activate_timers();
    crate::animations::update_animations();
}

/// Return the duration before the next timer should be activated. This is basically the
/// maximum time before calling [`upate_timers_and_animation()`].
///
/// That is typically called by the implementation of the event loop to know how long the
/// thread can go to sleep before the next event.
///
/// Note: this does not include animations. [`Window::has_active_animation()`](crate::api::Window::has_active_animation())
/// can be called to know if a window has running animation
pub fn duration_until_next_timer_update() -> Option<core::time::Duration> {
    crate::timers::TimerList::next_timeout().map(|timeout| {
        core::time::Duration::from_millis(
            timeout.0.saturating_sub(instance().unwrap().duration_since_start().as_millis() as u64),
        )
    })
}
