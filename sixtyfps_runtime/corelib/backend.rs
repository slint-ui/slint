// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!
The backend is the abstraction for crates that need to do the actual drawing and event loop
*/

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;

use crate::graphics::{Image, Size};
use crate::window::Window;

#[cfg(feature = "std")]
use once_cell::sync::OnceCell;

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use crate::unsafe_single_core::OnceCell;

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
    /// FIXME: should return a Box<dyn PlatformWindow>
    fn create_window(&'static self) -> Rc<Window>;

    /// Spins an event loop and renders the visible windows.
    fn run_event_loop(&'static self, behavior: EventLoopQuitBehavior);

    /// Exits the event loop.
    fn quit_event_loop(&'static self);

    #[cfg(feature = "std")] // FIXME: just because of the Error
    /// This function can be used to register a custom TrueType font with SixtyFPS,
    /// for use with the `font-family` property. The provided slice must be a valid TrueType
    /// font.
    fn register_font_from_memory(
        &'static self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>>;

    #[cfg(feature = "std")]
    /// This function can be used to register a custom TrueType font with SixtyFPS,
    /// for use with the `font-family` property. The provided path must refer to a valid TrueType
    /// font.
    fn register_font_from_path(
        &'static self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn set_clipboard_text(&'static self, text: String);
    fn clipboard_text(&'static self) -> Option<String>;

    /// Send an user event to from another thread that should be run in the GUI event loop
    fn post_event(&'static self, event: Box<dyn FnOnce() + Send>);

    fn image_size(&'static self, image: &Image) -> Size;

    fn duration_since_start(&'static self) -> core::time::Duration {
        #[cfg(feature = "std")]
        {
            let the_beginning = *INITIAL_INSTANT.get_or_init(instant::Instant::now);
            return instant::Instant::now() - the_beginning;
        }
        #[cfg(not(feature = "std"))]
        return core::time::Duration::ZERO;
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
