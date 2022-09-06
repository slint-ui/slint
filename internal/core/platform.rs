// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
The backend is the abstraction for crates that need to do the actual drawing and event loop
*/

#![warn(missing_docs)]

#[cfg(feature = "swrenderer")]
pub use crate::swrenderer;
#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::unsafe_single_threaded::{thread_local, OnceCell};
pub use crate::window::WindowAdapter;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
#[cfg(feature = "std")]
use once_cell::sync::OnceCell;

/// Interface implemented by back-ends
pub trait Platform {
    /// Instantiate a window for a component.
    fn create_window_adapter(&self) -> Rc<dyn WindowAdapter>;

    /// Spins an event loop and renders the visible windows.
    fn run_event_loop(&self) {
        unimplemented!("The backend does not implement running an eventloop")
    }

    /// Specify if the event loop should quit quen the last window is closed.
    /// The default behavior is `true`.
    /// When this is set to `false`, the event loop must keep running until
    /// [`slint::quit_event_loop()`](crate::api::quit_event_loop()) is called
    #[doc(hidden)]
    fn set_event_loop_quit_on_last_window_closed(&self, _quit_on_last_window_closed: bool) {
        unimplemented!("The backend does not implement event loop quit behaviors")
    }

    /// Return an [`EventLoopProxy`] that can be used to send event to the event loop
    ///
    /// If this function returns `None` (the default implementation), then it will
    /// not be possible to send event to the event loop and the function
    /// [`slint::invoke_from_event_loop()`](crate::api::invoke_from_event_loop) and
    /// [`slint::quit_event_loop()`](crate::api::quit_event_loop) will panic
    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        None
    }

    /// Returns the current time as a monotonic duration since the start of the program
    ///
    /// This is used by the animations and timer to compute the elapsed time.
    ///
    /// When the `std` feature is enabled, this function is implemented in terms of
    /// [`std::time::Instant::now()`], but on `#![no_std]` platform, this function must
    /// be implemented.
    fn duration_since_start(&self) -> core::time::Duration {
        #[cfg(feature = "std")]
        {
            let the_beginning = *INITIAL_INSTANT.get_or_init(instant::Instant::now);
            instant::Instant::now() - the_beginning
        }
        #[cfg(not(feature = "std"))]
        unimplemented!("The platform abstraction must implement `duration_since_start`")
    }

    /// Sends the given text into the system clipboard
    fn set_clipboard_text(&self, _text: &str) {}
    /// Returns a copy of text stored in the system clipboard, if any.
    fn clipboard_text(&self) -> Option<String> {
        None
    }

    /// This function is called when debug() is used in .slint files. The implementation
    /// should direct the output to some developer visible terminal. The default implementation
    /// uses stderr if available, or `console.log` when targeting wasm.
    fn debug_log(&self, _arguments: core::fmt::Arguments) {
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                use wasm_bindgen::prelude::*;

                #[wasm_bindgen]
                extern "C" {
                    #[wasm_bindgen(js_namespace = console)]
                    pub fn log(s: &str);
                }

                log(&_arguments.to_string());
            } else if #[cfg(feature = "std")] {
                eprintln!("{}", _arguments);
            }
        }
    }
}

/// Trait that is returned by the [`Platform::new_event_loop_proxy`]
///
/// This are the implementation details for the function that may need to
/// communicate with the eventloop from different thread
pub trait EventLoopProxy: Send + Sync {
    /// Exits the event loop.
    ///
    /// This is what is called by [`slint::quit_event_loop()`](crate::api::quit_event_loop)
    fn quit_event_loop(&self);

    /// Invoke the function from the event loop.
    ///
    /// This is what is called by [`slint::invoke_from_event_loop()`](crate::api::invoke_from_event_loop)
    fn invoke_from_event_loop(&self, event: Box<dyn FnOnce() + Send>);
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

thread_local! {
    /// Internal: Singleton of the platform abstraction.
    pub(crate) static PLATFORM_INSTANCE : once_cell::unsync::OnceCell<Box<dyn Platform>>
        = once_cell::unsync::OnceCell::new()
}
static EVENTLOOP_PROXY: OnceCell<Box<dyn EventLoopProxy + 'static>> = OnceCell::new();

pub(crate) fn event_loop_proxy() -> Option<&'static dyn EventLoopProxy> {
    EVENTLOOP_PROXY.get().map(core::ops::Deref::deref)
}

/// This enum describes the different error scenarios that may occur when [`set_platform`]
/// fails.
#[derive(Debug, Clone)]
#[repr(C)]
#[non_exhaustive]
pub enum SetPlatformError {
    /// The platform has been initialized in an earlier call to [`set_platform`].
    AlreadySet,
}

/// Set the slint platform abstraction.
///
/// If the platform abstraction was already set this will return `Err`
pub fn set_platform(platform: Box<dyn Platform + 'static>) -> Result<(), SetPlatformError> {
    PLATFORM_INSTANCE.with(|instance| {
        if instance.get().is_some() {
            return Err(SetPlatformError::AlreadySet);
        }
        if let Some(proxy) = platform.new_event_loop_proxy() {
            EVENTLOOP_PROXY.set(proxy).map_err(|_| SetPlatformError::AlreadySet)?
        }
        instance.set(platform.into()).map_err(|_| SetPlatformError::AlreadySet).unwrap();
        Ok(())
    })
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
/// maximum time before calling [`update_timers_and_animations()`].
///
/// That is typically called by the implementation of the event loop to know how long the
/// thread can go to sleep before the next event.
///
/// Note: this does not include animations. [`Window::has_active_animations()`](crate::api::Window::has_active_animations())
/// can be called to know if a window has running animation
pub fn duration_until_next_timer_update() -> Option<core::time::Duration> {
    crate::timers::TimerList::next_timeout().map(|timeout| {
        let duration_since_start = crate::platform::PLATFORM_INSTANCE
            .with(|p| p.get().map(|p| p.duration_since_start()))
            .unwrap_or_default();
        core::time::Duration::from_millis(
            timeout.0.saturating_sub(duration_since_start.as_millis() as u64),
        )
    })
}
