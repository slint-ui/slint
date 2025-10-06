// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::api::PlatformError;
use crate::platform::{EventLoopProxy, Platform};
use crate::Property;
use alloc::boxed::Box;
use alloc::rc::Rc;

crate::thread_local! {
    pub(crate) static GLOBAL_CONTEXT : once_cell::unsync::OnceCell<SlintContext>
        = const { once_cell::unsync::OnceCell::new() }
}

pub(crate) struct SlintContextInner {
    platform: Box<dyn Platform>,
    pub(crate) window_count: core::cell::RefCell<isize>,
    /// This property is read by all translations, and marked dirty when the language changes,
    /// so that every translated string gets re-translated. The property's value is the current selected
    /// language when bundling translations.
    pub(crate) translations_dirty: core::pin::Pin<Box<Property<usize>>>,
    pub(crate) translations_bundle_languages:
        core::cell::RefCell<Option<alloc::vec::Vec<&'static str>>>,
    pub(crate) window_shown_hook:
        core::cell::RefCell<Option<Box<dyn FnMut(&Rc<dyn crate::platform::WindowAdapter>)>>>,
    #[cfg(all(unix, not(target_os = "macos")))]
    xdg_app_id: core::cell::RefCell<Option<crate::SharedString>>,
    #[cfg(feature = "tr")]
    external_translator: core::cell::RefCell<Option<Box<dyn tr::Translator>>>,
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
            translations_dirty: Box::pin(Property::new_named(0, "SlintContext::translations")),
            translations_bundle_languages: Default::default(),
            window_shown_hook: Default::default(),
            #[cfg(all(unix, not(target_os = "macos")))]
            xdg_app_id: Default::default(),
            #[cfg(feature = "tr")]
            external_translator: Default::default(),
        }))
    }

    /// Return a reference to the platform abstraction
    pub fn platform(&self) -> &dyn Platform {
        &*self.0.platform
    }

    /// Return an event proxy
    // FIXME: Make EvenLoopProxy cloneable, and maybe wrap in a struct
    pub fn event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        self.0.platform.new_event_loop_proxy()
    }

    #[cfg(target_has_atomic = "ptr")]
    /// Context specific version of `slint::spawn_local`
    pub fn spawn_local<F: core::future::Future + 'static>(
        &self,
        fut: F,
    ) -> Result<crate::future::JoinHandle<F::Output>, crate::api::EventLoopError> {
        crate::future::spawn_local_with_ctx(self, fut)
    }

    pub fn run_event_loop(&self) -> Result<(), PlatformError> {
        self.0.platform.run_event_loop()
    }

    pub fn set_xdg_app_id(&self, _app_id: crate::SharedString) {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            self.0.xdg_app_id.replace(Some(_app_id));
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    pub fn xdg_app_id(&self) -> Option<crate::SharedString> {
        self.0.xdg_app_id.borrow().clone()
    }

    #[cfg(not(all(unix, not(target_os = "macos"))))]
    pub fn xdg_app_id(&self) -> Option<crate::SharedString> {
        None
    }

    #[cfg(feature = "tr")]
    pub fn set_external_translator(&self, translator: Option<Box<dyn tr::Translator>>) {
        *self.0.external_translator.borrow_mut() = translator;
    }

    #[cfg(feature = "tr")]
    pub fn external_translator(&self) -> Option<core::cell::Ref<'_, Box<dyn tr::Translator>>> {
        core::cell::Ref::filter_map(self.0.external_translator.borrow(), |maybe_translator| {
            maybe_translator.as_ref()
        })
        .ok()
    }
}

/// Internal function to access the context.
/// The factory function is called if the platform abstraction is not yet
/// initialized, and should be given by the platform_selector
pub fn with_global_context<R>(
    factory: impl FnOnce() -> Result<Box<dyn Platform + 'static>, PlatformError>,
    f: impl FnOnce(&SlintContext) -> R,
) -> Result<R, PlatformError> {
    GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => Ok(f(ctx)),
        None => {
            crate::platform::set_platform(factory()?).map_err(PlatformError::SetPlatformError)?;
            Ok(f(p.get().unwrap()))
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
