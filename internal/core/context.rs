// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Property;
use crate::api::PlatformError;
use crate::graphics::Color;
use crate::input::InternalKeyboardModifierState;
use crate::item_tree::{ItemRc, ItemTreeRc};
use crate::items::ColorScheme;
use crate::lengths::LogicalLength;
use crate::platform::{EventLoopProxy, Platform, WindowAdapter, WindowEvent};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::Cell;
use core::cell::RefCell;
use pin_weak::rc::PinWeak;

/// Type alias for the closure type installed via [`set_window_event_hook`].
/// Exposed so callers (notably tests) can save and restore a previously-installed hook.
pub type WindowEventHook =
    Box<dyn Fn(&Rc<dyn WindowAdapter>, &WindowEvent, crate::api::WindowEventDispatchResult)>;

crate::thread_local! {
    pub(crate) static GLOBAL_CONTEXT : once_cell::unsync::OnceCell<SlintContext>
        = const { once_cell::unsync::OnceCell::new() }
}

#[pin_project::pin_project]
pub(crate) struct SlintContextInner {
    platform: Box<dyn Platform>,
    pub(crate) window_count: core::cell::RefCell<isize>,

    /// Read by all translations, and marked dirty when the language changes so every
    /// translated string re-translates. The value is the currently selected language
    /// when bundling translations.
    #[pin]
    pub(crate) translations_dirty: Property<usize>,
    pub(crate) translations_bundle:
        core::cell::RefCell<Option<alloc::vec::Vec<i_slint_common::TranslationsBundled>>>,
    #[cfg(feature = "tr")]
    external_translator: core::cell::RefCell<Option<Box<dyn tr::Translator>>>,
    #[pin]
    pub(crate) locale_decimal_separator: Property<char>,

    /// Process-wide color scheme. Backends' system-theme observers write here; bindings
    /// read from it through [`SlintContext::color_scheme`]. Window-less components like
    /// `SystemTrayIcon` rely on this as their default source.
    #[pin]
    pub(crate) color_scheme: Property<ColorScheme>,
    /// Process-wide system accent color. Backends' system-theme observers write here;
    /// bindings read from it through [`SlintContext::accent_color`]. Defaults to a
    /// transparent color when the platform doesn't expose one.
    #[pin]
    pub(crate) accent_color: Property<Color>,
    /// Process-wide default font size as reported by the platform (e.g. iOS Dynamic
    /// Type). Backends write here; `WindowItem::resolved_default_font_size` consults it
    /// before falling back to `textlayout::DEFAULT_FONT_SIZE`. `None` when the backend
    /// doesn't report one.
    #[pin]
    pub(crate) platform_default_font_size: Property<Option<LogicalLength>>,
    pub(crate) window_shown_hook:
        core::cell::RefCell<Option<Box<dyn FnMut(&Rc<dyn crate::platform::WindowAdapter>)>>>,
    pub(crate) window_event_hook: core::cell::RefCell<Option<WindowEventHook>>,
    pub(crate) log_message_handler: RefCell<Option<crate::debug_log::LogMessageHandler>>,
    #[cfg(all(unix, not(target_os = "macos")))]
    xdg_app_id: core::cell::RefCell<Option<crate::SharedString>>,
    #[cfg(feature = "shared-parley")]
    pub(crate) font_context: core::cell::RefCell<crate::textlayout::sharedparley::FontContext>,
    #[cfg(feature = "shared-swash")]
    pub(crate) swash_scale_context: core::cell::RefCell<swash::scale::ScaleContext>,
    pub(crate) modifiers: Cell<InternalKeyboardModifierState>,
}

/// This context is meant to hold the state and the backend.
/// Currently it is not possible to have several platform at the same time in one process, but in the future it might be.
/// See issue #4294
#[derive(Clone)]
pub struct SlintContext(pub(crate) core::pin::Pin<Rc<SlintContextInner>>);

impl SlintContext {
    /// Create a new context with a given platform
    pub fn new(platform: Box<dyn Platform + 'static>) -> Self {
        #[cfg(feature = "shared-parley")]
        let collection = i_slint_common::sharedfontique::create_collection(true);

        Self(Rc::pin(SlintContextInner {
            platform,
            window_count: 0.into(),

            translations_dirty: Property::new_named(0, "SlintContext::translations"),
            translations_bundle: Default::default(),
            #[cfg(feature = "tr")]
            external_translator: Default::default(),
            locale_decimal_separator: Property::new_named(
                i_slint_common::DEFAULT_DECIMAL_SEPARATOR,
                "SlintContext::locale_decimal_separator",
            ),

            color_scheme: Property::new_named(ColorScheme::Unknown, "SlintContext::color_scheme"),
            accent_color: Property::new_named(Color::default(), "SlintContext::accent_color"),
            platform_default_font_size: Property::new_named(
                None,
                "SlintContext::platform_default_font_size",
            ),
            window_shown_hook: Default::default(),
            window_event_hook: Default::default(),
            log_message_handler: Default::default(),
            #[cfg(all(unix, not(target_os = "macos")))]
            xdg_app_id: Default::default(),
            #[cfg(feature = "shared-parley")]
            font_context: {
                let font_context = parley::FontContext {
                    collection: collection.inner,
                    source_cache: collection.source_cache,
                };
                core::cell::RefCell::new(crate::textlayout::sharedparley::FontContext::new(
                    font_context,
                ))
            },
            #[cfg(feature = "shared-swash")]
            swash_scale_context: core::cell::RefCell::new(swash::scale::ScaleContext::new()),
            modifiers: Cell::new(Default::default()),
        }))
    }

    /// Return a reference to the platform abstraction
    pub fn platform(&self) -> &dyn Platform {
        &*self.0.platform
    }

    /// Return a reference to the font context
    #[cfg(feature = "shared-parley")]
    pub fn font_context(
        &self,
    ) -> &core::cell::RefCell<crate::textlayout::sharedparley::FontContext> {
        &self.0.font_context
    }

    /// Return a reference to the swash scale context
    #[cfg(feature = "shared-swash")]
    pub fn swash_scale_context(&self) -> &core::cell::RefCell<swash::scale::ScaleContext> {
        &self.0.swash_scale_context
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

    /// Returns the effective color scheme for the given component root, or the
    /// process-wide scheme when `root` is `None`. A `SystemTrayIcon`-rooted
    /// component resolves against the tray's own scheme first, falling back to
    /// the process-wide value when the tray reports `Unknown`. Reads register a
    /// property dependency, so bindings re-evaluate when the platform reports a
    /// system-theme change.
    pub fn color_scheme(&self, root: Option<&ItemTreeRc>) -> ColorScheme {
        if let Some(root) = root {
            let root_item = ItemRc::new_root(root.clone());
            if let Some(tray) = root_item.downcast::<crate::items::SystemTrayIcon>() {
                let scheme = tray.as_pin_ref().color_scheme();
                if scheme != ColorScheme::Unknown {
                    return scheme;
                }
            }
        }
        self.0.as_ref().project_ref().color_scheme.get()
    }

    /// Backend-side write path for the process-wide color scheme. Called by each
    /// platform's system-theme observer; `Property::set` short-circuits no-op writes.
    pub fn set_color_scheme(&self, scheme: ColorScheme) {
        self.0.as_ref().project_ref().color_scheme.set(scheme);
    }

    /// Returns the process-wide system accent color. Reads register a property dependency,
    /// so bindings re-evaluate when the platform reports an accent-color change.
    pub fn accent_color(&self) -> Color {
        self.0.as_ref().project_ref().accent_color.get()
    }

    /// Backend-side write path for the process-wide accent color. Called by each
    /// platform's system-theme observer; `Property::set` short-circuits no-op writes.
    pub fn set_accent_color(&self, color: Color) {
        self.0.as_ref().project_ref().accent_color.set(color);
    }

    /// Returns the platform-reported default font size, or `None` if the backend doesn't
    /// report one. Reads register a property dependency, so bindings re-evaluate when the
    /// platform reports a change (e.g. the user adjusts the system text size).
    pub fn platform_default_font_size(&self) -> Option<LogicalLength> {
        self.0.as_ref().project_ref().platform_default_font_size.get()
    }

    /// Backend-side write path for the platform-reported default font size. Called by
    /// backends that track the system setting; `Property::set` short-circuits no-op writes.
    pub fn set_platform_default_font_size(&self, size: Option<LogicalLength>) {
        self.0.as_ref().project_ref().platform_default_font_size.set(size);
    }

    #[doc(hidden)]
    pub fn dispatch_log_message(&self, message: crate::debug_log::LogMessage<'_>) {
        if let Some(handler) = self.0.log_message_handler.borrow().as_ref() {
            handler(message);
        } else {
            self.0.platform.debug_log(message.message_arguments());
        }
    }

    #[doc(hidden)]
    pub fn set_log_message_handler(
        &self,
        handler: Option<crate::debug_log::LogMessageHandler>,
    ) -> Option<crate::debug_log::LogMessageHandler> {
        let mut slot = self.0.log_message_handler.borrow_mut();
        core::mem::replace(&mut *slot, handler)
    }

    /// Add one to the counter of "things keeping the event loop alive".
    /// Visible windows and visible system tray icons are the canonical
    /// callers; they pair with [`Self::release_keepalive`].
    pub(crate) fn acquire_keepalive(&self) {
        *self.0.window_count.borrow_mut() += 1;
    }

    /// Subtract one from the keepalive counter and quit the event loop if
    /// nothing is keeping it alive anymore. Mirrors the post-decrement quit
    /// that [`crate::window::WindowInner::hide`] used to do inline.
    pub(crate) fn release_keepalive(&self) {
        let mut count = self.0.window_count.borrow_mut();
        *count -= 1;
        if *count <= 0 {
            drop(count);
            let _ = self.event_loop_proxy().and_then(|p| p.quit_event_loop().ok());
        }
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

    /// Returns the locale's decimal separator, falling back to `translations::DEFAULT_SEPARATOR`.
    pub fn locale_decimal_separator(&self) -> char {
        self.0.as_ref().project_ref().locale_decimal_separator.get()
    }

    /// Override the locale used for decimal separator detection (testing only).
    #[cfg(feature = "std")]
    pub fn set_locale(&self, locale: &str) {
        self.0
            .as_ref()
            .project_ref()
            .locale_decimal_separator
            .set(i_slint_common::decimal_separator_for_locale(locale));
    }

    #[cfg(feature = "tr")]
    pub fn set_external_translator(&self, translator: Option<Box<dyn tr::Translator>>) {
        *self.0.external_translator.borrow_mut() = translator;
        self.0.as_ref().project_ref().translations_dirty.mark_dirty();
    }

    #[cfg(feature = "tr")]
    pub fn external_translator(&self) -> Option<core::cell::Ref<'_, Box<dyn tr::Translator>>> {
        core::cell::Ref::filter_map(self.0.external_translator.borrow(), |maybe_translator| {
            maybe_translator.as_ref()
        })
        .ok()
    }

    /// Returns a weak handle to this context, suitable for stashing in places that must
    /// not keep the context alive (e.g. a backend that's owned by the context itself).
    pub fn downgrade(&self) -> SlintContextWeak {
        SlintContextWeak(PinWeak::downgrade(self.0.clone()))
    }
}

/// Weak handle to a [`SlintContext`]. Backends that opt into
/// [`crate::platform::Platform::bind_context`] receive one of these right after
/// `set_platform` so they can spawn futures and write process-wide state without
/// holding the context strongly.
#[derive(Clone)]
pub struct SlintContextWeak(PinWeak<SlintContextInner>);

impl SlintContextWeak {
    /// Attempts to upgrade to a strong [`SlintContext`].
    pub fn upgrade(&self) -> Option<SlintContext> {
        self.0.upgrade().map(SlintContext)
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
            if crate::platform::with_event_loop_proxy(|proxy| proxy.is_some()) {
                return Err(PlatformError::SetPlatformError(
                    crate::platform::SetPlatformError::AlreadySet,
                ));
            }
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

/// Internal function to set a hook that's invoked after a window event was dispatched.
/// This is used by the system testing module. Returns a previously set hook, if any.
pub fn set_window_event_hook(
    hook: Option<WindowEventHook>,
) -> Result<Option<WindowEventHook>, PlatformError> {
    GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => {
            let mut slot = ctx.0.window_event_hook.try_borrow_mut().map_err(|_| {
                PlatformError::Other(alloc::string::String::from("event hook is currently in use"))
            })?;
            Ok(core::mem::replace(&mut *slot, hook))
        }
        None => Err(PlatformError::NoPlatform),
    })
}
