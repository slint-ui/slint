// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
The backend is the abstraction for crates that need to do the actual drawing and event loop
*/

#![warn(missing_docs)]

pub use crate::api::PlatformError;
use crate::api::{LogicalPosition, LogicalSize};
pub use crate::renderer::Renderer;
pub use crate::software_renderer;
#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::unsafe_single_threaded::{thread_local, OnceCell};
pub use crate::window::WindowAdapter;
use crate::SharedString;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
#[cfg(feature = "std")]
use once_cell::sync::OnceCell;

/// This trait defines the interface between Slint and platform APIs typically provided by operating and windowing systems.
pub trait Platform {
    /// Instantiate a window for a component.
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError>;

    /// Spins an event loop and renders the visible windows.
    fn run_event_loop(&self) -> Result<(), PlatformError> {
        Err(PlatformError::NoEventLoopProvider)
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

    /// Returns the current interval to internal measure the duration to send a double click event.
    ///
    /// A double click event is a series of two pointer clicks.
    fn click_interval(&self) -> core::time::Duration {
        core::time::Duration::from_millis(500)
    }

    /// Sends the given text into the system clipboard.
    ///
    /// If the platform doesn't support the specified clipboard, this function should do nothing
    fn set_clipboard_text(&self, _text: &str, _clipboard: Clipboard) {}

    /// Returns a copy of text stored in the system clipboard, if any.
    ///
    /// If the platform doesn't support the specified clipboard, the function should return None
    fn clipboard_text(&self, _clipboard: Clipboard) -> Option<String> {
        None
    }

    /// This function is called when debug() is used in .slint files. The implementation
    /// should direct the output to some developer visible terminal. The default implementation
    /// uses stderr if available, or `console.log` when targeting wasm.
    fn debug_log(&self, _arguments: core::fmt::Arguments) {
        crate::tests::default_debug_log(_arguments);
    }
}

/// The clip board, used in [`Platform::clipboard_text`] and [Platform::set_clipboard_text`]
#[non_exhaustive]
#[derive(PartialEq, Clone, Default)]
pub enum Clipboard {
    /// This is the default clipboard used for text action for Ctrl+V,  Ctrl+C.
    /// Corresponds to the secondary clipboard on X11.
    #[default]
    DefaultClipboard,

    /// This is the clipboard that is used when text is selected
    /// Corresponds to the primary clipboard on X11.
    /// The Platform implementation should do nothing if copy on select is not supported on that platform.
    SelectionClipboard,
}

/// Trait that is returned by the [`Platform::new_event_loop_proxy`]
///
/// This are the implementation details for the function that may need to
/// communicate with the eventloop from different thread
pub trait EventLoopProxy: Send + Sync {
    /// Exits the event loop.
    ///
    /// This is what is called by [`slint::quit_event_loop()`](crate::api::quit_event_loop)
    fn quit_event_loop(&self) -> Result<(), crate::api::EventLoopError>;

    /// Invoke the function from the event loop.
    ///
    /// This is what is called by [`slint::invoke_from_event_loop()`](crate::api::invoke_from_event_loop)
    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), crate::api::EventLoopError>;
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
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
#[non_exhaustive]
pub enum SetPlatformError {
    /// The platform has been initialized in an earlier call to [`set_platform`].
    AlreadySet,
}

/// Set the Slint platform abstraction.
///
/// If the platform abstraction was already set this will return `Err`.
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

/// Call this function to update and potentially activate any pending timers, as well
/// as advance the state of any active animtaions.
///
/// This function should be called before rendering or processing input event, at the
/// beginning of each event loop iteration.
pub fn update_timers_and_animations() {
    crate::animations::update_animations();
    crate::timers::TimerList::maybe_activate_timers(crate::animations::Instant::now());
}

/// Returns the duration before the next timer is expected to be activated. This is the
/// largest amount of time that you can wait before calling [`update_timers_and_animations()`].
///
/// `None` is returned if there is no active timer.
///
/// Call this in your own event loop implementation to know how long the current thread can
/// go to sleep. Note that this does not take currently activate animations into account.
/// Only go to sleep if [`Window::has_active_animations()`](crate::api::Window::has_active_animations())
/// returns false.
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

// reexport key enum to the public api
pub use crate::input::key_codes::Key;
pub use crate::input::PointerEventButton;

/// A event that describes user input or windowing system events.
///
/// Slint backends typically receive events from the windowing system, translate them to this
/// enum and deliver to the scene of items via [`slint::Window::dispatch_event()`](`crate::api::Window::dispatch_event()`).
///
/// The pointer variants describe events originating from an input device such as a mouse
/// or a contact point on a touch-enabled surface.
///
/// All position fields are in logical window coordinates.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum WindowEvent {
    /// A pointer was pressed.
    PointerPressed {
        position: LogicalPosition,
        /// The button that was pressed.
        button: PointerEventButton,
    },
    /// A pointer was released.
    PointerReleased {
        position: LogicalPosition,
        /// The button that was released.
        button: PointerEventButton,
    },
    /// The position of the pointer has changed.
    PointerMoved { position: LogicalPosition },
    /// The wheel button of a mouse was rotated to initiate scrolling.
    PointerScrolled {
        position: LogicalPosition,
        /// The amount of logical pixels to scroll in the horizontal direction.
        delta_x: f32,
        /// The amount of logical pixels to scroll in the vertical direction.
        delta_y: f32,
    },
    /// The pointer exited the window.
    PointerExited,
    /// A key was pressed.
    KeyPressed {
        /// The unicode representation of the key pressed.
        ///
        /// # Example
        /// A specific key can be mapped to a unicode by using the [`Key`] enum
        /// ```rust
        /// let _ = slint::platform::WindowEvent::KeyPressed { text: slint::platform::Key::Shift.into() };
        /// ```
        text: SharedString,
    },
    /// A key was pressed.
    KeyReleased {
        /// The unicode representation of the key released.
        ///
        /// # Example
        /// A specific key can be mapped to a unicode by using the [`Key`] enum
        /// ```rust
        /// let _ = slint::platform::WindowEvent::KeyReleased { text: slint::platform::Key::Shift.into() };
        /// ```
        text: SharedString,
    },
    /// The window's scale factor has changed. This can happen for example when the display's resolution
    /// changes, the user selects a new scale factor in the system settings, or the window is moved to a
    /// different screen.
    /// Platform implementations should dispatch this event also right after the initial window creation,
    /// to set the initial scale factor the windowing system provided for the window.
    ScaleFactorChanged {
        /// The window system provided scale factor to map logical pixels to physical pixels.
        scale_factor: f32,
    },
    /// The window was resized.
    ///
    /// The backend must send this event to ensure that the `width` and `height` property of the root Window
    /// element are properly set.
    Resized {
        /// The new logical size of the window
        size: LogicalSize,
    },
}

impl WindowEvent {
    /// The position of the cursor for this event, if any
    pub fn position(&self) -> Option<LogicalPosition> {
        match self {
            WindowEvent::PointerPressed { position, .. } => Some(*position),
            WindowEvent::PointerReleased { position, .. } => Some(*position),
            WindowEvent::PointerMoved { position } => Some(*position),
            WindowEvent::PointerScrolled { position, .. } => Some(*position),
            _ => None,
        }
    }
}
