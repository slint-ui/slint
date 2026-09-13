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
    Box<dyn Fn(&Rc<dyn WindowAdapter>, &WindowEvent, crate::platform::WindowEventDispatchResult)>;

/// Type alias for the closure type installed via [`SlintContext::set_open_file_handler`].
/// The closure receives the paths of the files that the operating system asked the
/// application to open (e.g. via macOS file associations and "Open With").
pub type OpenFileHandler = Box<dyn Fn(&[crate::SharedString])>;

/// The handler installed with [`SlintContext::set_open_file_handler`] together with the
/// file-open requests that arrived before a handler was around to receive them.
///
/// The requests are kept here, next to the handler they are destined for, so that a
/// handler installed late (e.g. after a cold launch where the operating system already
/// asked the app to open files) still receives them, in order.
#[derive(Default)]
pub(crate) struct OpenFileState {
    handler: Option<OpenFileHandler>,
    pending: alloc::vec::Vec<alloc::vec::Vec<crate::SharedString>>,
}

/// Restores the open-file handler that [`SlintContext::dispatch_open_files`] takes out of
/// its state while it runs the user callback. The handler is put back, unless the callback
/// installed a (new) handler itself. Restoring on drop also covers a panicking callback.
struct OpenFileHandlerGuard<'a> {
    state: &'a core::cell::RefCell<OpenFileState>,
    handler: Option<OpenFileHandler>,
}

impl Drop for OpenFileHandlerGuard<'_> {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.take() {
            let mut state = self.state.borrow_mut();
            if state.handler.is_none() {
                state.handler = Some(handler);
            }
        }
    }
}

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
    /// State of the "open files" feature: the handler installed with
    /// [`SlintContext::set_open_file_handler`] plus the file-open requests that arrived
    /// before a handler was around to receive them. Kept together so that a late handler
    /// still gets the requests it missed.
    pub(crate) open_file_state: RefCell<OpenFileState>,
    #[cfg(all(unix, not(target_os = "macos")))]
    xdg_app_id: core::cell::RefCell<Option<crate::SharedString>>,
    #[cfg(feature = "shared-parley")]
    pub(crate) font_context: core::cell::RefCell<crate::textlayout::sharedparley::FontContext>,
    #[cfg(feature = "shared-swash")]
    pub(crate) swash_scale_context: core::cell::RefCell<swash::scale::ScaleContext>,
    pub(crate) modifiers: Cell<InternalKeyboardModifierState>,

    /// The timers registered on this context. Shared, so that `Timer` handles can hold a
    /// `Weak` to the list they registered in without knowing which context owns it.
    pub(crate) timers: crate::timers::TimerListRc,
}

/// This context is meant to hold the state and the backend.
/// Currently it is not possible to have several platform at the same time in one process, but in the future it might be.
/// See issue #4294
#[derive(Clone)]
pub struct SlintContext(pub(crate) core::pin::Pin<Rc<SlintContextInner>>);

impl SlintContext {
    /// Create a new context with a given platform.
    ///
    /// If this thread has no context yet, the new one becomes it — first come, first
    /// served. That is what the ambient APIs resolve to: [`crate::timers::Timer`],
    /// `spawn_local`, `quit_event_loop` and friends. Contexts created afterwards are
    /// perfectly usable, but are not the thread's current one, so code holding such a
    /// context has to be explicit about it (e.g. [`Self::new_timer`]).
    pub fn new(platform: Box<dyn Platform + 'static>) -> Self {
        #[cfg(feature = "shared-parley")]
        let collection = i_slint_common::sharedfontique::create_collection(true);

        let this = Self(Rc::pin(SlintContextInner {
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
            open_file_state: Default::default(),
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
            // Timers started before this thread had a context registered in the pending
            // list; take it over so those timers keep working. It is the very list they
            // hold a `Weak` to, so nothing needs fixing up.
            timers: crate::timers::take_pending_timers(),
        }));
        // The list's deadlines are measured on this context's clock from now on. Done after
        // construction because it needs a handle to the context that owns it.
        crate::timers::set_owning_context(&this.0.timers, &this);
        // Claim this thread's context slot if it is still free, so that the ambient APIs
        // resolve here rather than to a list nothing drives. Fails harmlessly when the
        // thread already has a context: that one stays current.
        GLOBAL_CONTEXT.with(|slot| {
            let _ = slot.set(this.clone());
        });
        // Every context tells its platform which context it belongs to, not just the one
        // that becomes this thread's global: a platform is owned by exactly one context, and
        // a backend driving a context needs to be able to find it.
        this.platform().bind_context(this.downgrade(), crate::InternalToken);
        this
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

    /// Creates a [`Timer`](crate::timers::Timer) that registers on this context rather than
    /// on whichever one is current when it is started.
    ///
    /// For the context that a thread runs its event loop on this is the same as
    /// `Timer::default()`, and the event loop activates the timer as usual. A context that
    /// isn't the current one has no event loop driving it, so its owner is responsible for
    /// calling [`Self::maybe_activate_timers`].
    pub fn new_timer(&self) -> crate::timers::Timer {
        crate::timers::Timer::with_list(&self.0.timers)
    }

    /// Runs `callback` once, `duration` from now, on this context.
    ///
    /// The context-bound counterpart of [`Timer::single_shot`](crate::timers::Timer::single_shot),
    /// which registers on whichever context is current instead.
    pub fn single_shot(&self, duration: core::time::Duration, callback: impl FnOnce() + 'static) {
        crate::timers::single_shot_on(&self.0.timers, duration, callback);
    }

    /// Advances this context's animations and timers to its own clock, and runs any change
    /// handlers that fall out of it.
    ///
    /// This is what an event loop driving this context should call at the top of each
    /// iteration. [`crate::platform::update_timers_and_animations`] is the same thing for
    /// whichever context is this thread's global one.
    pub fn update_timers_and_animations(&self) {
        let now = crate::animations::Instant::now(self);
        crate::animations::update_animations(now);
        self.maybe_activate_timers(now);
        crate::properties::ChangeTracker::run_change_handlers();
    }

    /// How long this context can go to sleep before its next timer is due, or `None` when it
    /// has no active timer.
    ///
    /// The deadline and the clock it is measured against both come from this context, so
    /// they cannot disagree.
    pub fn duration_until_next_timer_update(&self) -> Option<core::time::Duration> {
        let timeout = self.next_timer_timeout()?;
        let now = crate::animations::Instant::now(self);
        Some(core::time::Duration::from_millis(timeout.0.saturating_sub(now.0)))
    }

    /// Fires the callbacks of this context's timers that have expired by `now`, and returns
    /// whether any of them was activated.
    pub fn maybe_activate_timers(&self, now: crate::animations::Instant) -> bool {
        crate::timers::TimerList::activate_expired(&self.0.timers, now)
    }

    /// Returns when this context's next timer is due, or `None` if it has no active timer.
    pub fn next_timer_timeout(&self) -> Option<crate::animations::Instant> {
        self.0.timers.borrow().first_timeout()
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

    /// Set the handler invoked when the operating system asks the application to open
    /// files, for example because the user opened a file by double-clicking it or via
    /// "Open With". The handler receives the paths of the files that were opened.
    ///
    /// Returns the previously-installed handler, if any.
    ///
    /// If the operating system already asked the application to open files before this
    /// handler was installed, those requests are delivered to the new handler
    /// immediately, in order of arrival.
    #[doc(hidden)]
    pub fn set_open_file_handler(
        &self,
        handler: Option<OpenFileHandler>,
    ) -> Option<OpenFileHandler> {
        let previous;
        let pending;
        {
            let mut state = self.0.open_file_state.borrow_mut();
            previous = core::mem::replace(&mut state.handler, handler);
            pending = if state.handler.is_some() {
                core::mem::take(&mut state.pending)
            } else {
                alloc::vec::Vec::new()
            };
        }
        for paths in pending {
            self.dispatch_open_files(&paths);
        }
        previous
    }

    /// Queue a file-open request from the operating system that cannot be dispatched to
    /// the handler yet, for example because the event loop isn't running. The requests
    /// are delivered when a handler is installed with [`Self::set_open_file_handler`] or
    /// when the backend forwards them after taking them with
    /// [`Self::take_pending_open_files`].
    #[doc(hidden)]
    pub fn queue_open_files(&self, paths: &[crate::SharedString]) {
        self.0.open_file_state.borrow_mut().pending.push(paths.to_vec());
    }

    /// Take the file-open requests queued with [`Self::queue_open_files`], in order,
    /// leaving the queue empty.
    #[doc(hidden)]
    pub fn take_pending_open_files(&self) -> alloc::vec::Vec<alloc::vec::Vec<crate::SharedString>> {
        core::mem::take(&mut self.0.open_file_state.borrow_mut().pending)
    }

    #[doc(hidden)]
    pub fn dispatch_open_files(&self, paths: &[crate::SharedString]) {
        let handler = self.0.open_file_state.borrow_mut().handler.take();
        let Some(handler) = handler else {
            crate::debug_log!(
                "Slint: ignoring a request to open {:?} because no open-file handler is installed (see slint::set_open_file_handler)",
                paths
            );
            return;
        };
        let guard = OpenFileHandlerGuard { state: &self.0.open_file_state, handler: Some(handler) };
        if let Some(handler) = guard.handler.as_ref() {
            handler(paths);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlatform;

    impl crate::platform::Platform for TestPlatform {
        fn create_window_adapter(
            &self,
        ) -> Result<std::rc::Rc<dyn crate::platform::WindowAdapter>, crate::api::PlatformError>
        {
            todo!()
        }
    }

    #[test]
    fn open_file_handler_is_dispatched() {
        crate::platform::set_platform(Box::new(TestPlatform)).unwrap();
        let ctx = GLOBAL_CONTEXT.with(|c| c.get().unwrap().clone());

        let dispatched: Rc<core::cell::RefCell<alloc::vec::Vec<crate::SharedString>>> =
            Rc::default();
        let dispatched2 = dispatched.clone();
        ctx.set_open_file_handler(Some(Box::new(move |paths| {
            *dispatched2.borrow_mut() = paths.to_vec();
        })));

        ctx.dispatch_open_files(&[
            crate::SharedString::from("a.txt"),
            crate::SharedString::from("b.txt"),
        ]);
        assert_eq!(dispatched.borrow().len(), 2);
        assert_eq!(dispatched.borrow()[0], crate::SharedString::from("a.txt"));

        ctx.set_open_file_handler(None);
        ctx.dispatch_open_files(&[crate::SharedString::from("c.txt")]);
        assert_eq!(dispatched.borrow().len(), 2);
    }

    #[test]
    fn queued_open_files_are_delivered_to_a_late_handler() {
        crate::platform::set_platform(Box::new(TestPlatform)).unwrap();
        let ctx = GLOBAL_CONTEXT.with(|c| c.get().unwrap().clone());

        ctx.queue_open_files(&[crate::SharedString::from("a.txt")]);
        ctx.queue_open_files(&[
            crate::SharedString::from("b.txt"),
            crate::SharedString::from("c.txt"),
        ]);

        let received: Rc<
            core::cell::RefCell<alloc::vec::Vec<alloc::vec::Vec<crate::SharedString>>>,
        > = Rc::default();
        let received2 = received.clone();
        ctx.set_open_file_handler(Some(Box::new(move |paths| {
            received2.borrow_mut().push(paths.to_vec());
        })));

        assert_eq!(
            received.borrow().as_slice(),
            &[
                [crate::SharedString::from("a.txt")].to_vec(),
                [crate::SharedString::from("b.txt"), crate::SharedString::from("c.txt")].to_vec(),
            ]
        );
    }

    #[test]
    fn take_pending_open_files_drains_the_queue() {
        crate::platform::set_platform(Box::new(TestPlatform)).unwrap();
        let ctx = GLOBAL_CONTEXT.with(|c| c.get().unwrap().clone());

        ctx.queue_open_files(&[crate::SharedString::from("a.txt")]);
        let pending = ctx.take_pending_open_files();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].as_slice(), &[crate::SharedString::from("a.txt")]);

        assert!(ctx.take_pending_open_files().is_empty());
    }
}
