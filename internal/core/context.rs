// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::api::PlatformError;
use crate::platform::{EventLoopProxy, Platform};
#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::thread_local;
use crate::Property;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use alloc::rc::Rc;

thread_local! {
    pub(crate) static GLOBAL_CONTEXT : once_cell::unsync::OnceCell<SlintContext>
        = const { once_cell::unsync::OnceCell::new() }
}

pub(crate) struct SlintContextInner {
    pub(crate) platform: Box<dyn Platform>,
    pub(crate) window_count: core::cell::RefCell<isize>,
    /// This property is read by all translations, and marked dirty when the language change
    /// so that every translated string gets re-translated
    pub(crate) translations_dirty: core::pin::Pin<Box<Property<()>>>,
    pub(crate) window_shown_hook:
        core::cell::RefCell<Option<Box<dyn FnMut(&Rc<dyn crate::platform::WindowAdapter>)>>>,
}

/// This context is meant to hold the state and the backend.
/// Currently it is not possible to have several platform at the same time in one process, but in the future it might be.
/// See issue #4294
#[derive(Clone)]
pub struct SlintContext(pub(crate) Rc<SlintContextInner>);

impl SlintContext {
    /// Create a new context with a given platform
    pub fn new(platform: Box<dyn Platform + 'static>) -> Self {
        Self(Rc::new(SlintContextInner {
            platform,
            window_count: 0.into(),
            translations_dirty: Box::pin(Property::new_named((), "SlintContext::translations")),
            window_shown_hook: Default::default(),
        }))
    }

    /// Return an event proxy
    // FIXME: Make EvenLoopProxy clonable, and maybe wrap in a struct
    pub fn event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        self.0.platform.new_event_loop_proxy()
    }

    #[cfg(target_has_atomic = "ptr")]
    /// Context specific version of [`slint::spawn_local`](crate::future::spawn_local)
    pub fn spawn_local<F: core::future::Future + 'static>(
        &self,
        fut: F,
    ) -> Result<crate::future::JoinHandle<F::Output>, crate::api::EventLoopError> {
        crate::future::spawn_local_with_ctx(self, fut)
    }

    pub fn run_event_loop(&self) -> Result<(), PlatformError> {
        self.0.platform.run_event_loop()
    }
}

/// Internal function to access the platform abstraction.
/// The factory function is called if the platform abstraction is not yet
/// initialized, and should be given by the platform_selector
pub fn with_platform<R>(
    factory: impl FnOnce() -> Result<Box<dyn Platform + 'static>, PlatformError>,
    f: impl FnOnce(&dyn Platform) -> Result<R, PlatformError>,
) -> Result<R, PlatformError> {
    GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => f(&*ctx.0.platform),
        None => {
            crate::platform::set_platform(factory()?).map_err(PlatformError::SetPlatformError)?;
            f(&*p.get().unwrap().0.platform)
        }
    })
}

/// Internal function to set a hook that's invoked whenever a slint::Window is shown. This
/// is used by the system testing module. Returns a previously set hook, if any.
pub fn set_window_shown_hook(
    hook: Option<Box<dyn FnMut(&Rc<dyn crate::platform::WindowAdapter>)>>,
) -> Result<Option<Box<dyn FnMut(&Rc<dyn crate::platform::WindowAdapter>)>>, PlatformError> {
    GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => Ok(ctx.0.window_shown_hook.replace(hook)),
        None => Err(PlatformError::NoPlatform),
    })
}
