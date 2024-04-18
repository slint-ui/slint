// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

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
        = once_cell::unsync::OnceCell::new()
}

pub(crate) struct SlintContextInner {
    pub(crate) platform: Box<dyn Platform>,
    pub(crate) window_count: core::cell::RefCell<isize>,
    /// This property is read by all translations, and marked dirty when the language change
    /// so that every translated string gets re-translated
    pub(crate) translations_dirty: core::pin::Pin<Box<Property<()>>>,
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
