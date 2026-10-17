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
use alloc::vec::Vec;
use core::cell::Cell;
use core::cell::RefCell;
use pin_weak::rc::PinWeak;

/// Type alias for the closure type installed via [`set_window_event_hook`].
/// Exposed so callers (notably tests) can save and restore a previously-installed hook.
pub type WindowEventHook =
    Box<dyn Fn(&Rc<dyn WindowAdapter>, &WindowEvent, crate::platform::WindowEventDispatchResult)>;

crate::thread_local! {
    pub(crate) static GLOBAL_CONTEXT : core::cell::OnceCell<SlintContext>
        = const { core::cell::OnceCell::new() }
}

/// The part of a `SlintContext` that every context created with
/// [`SlintContext::new_sharing_platform`] has in common:
/// the platform, and what the windowing system reports for the whole process.
/// A change made through one context is visible to all of them,
/// so state that a context should keep to itself belongs on `SlintContextInner`.
#[pin_project::pin_project]
pub(crate) struct PlatformContext {
    platform: Box<dyn Platform>,
    keepalive_count: Cell<isize>,

    #[pin]
    color_scheme: Property<ColorScheme>,
    #[pin]
    accent_color: Property<Color>,
    #[pin]
    platform_default_font_size: Property<Option<LogicalLength>>,
    modifiers: Cell<InternalKeyboardModifierState>,
    #[cfg(all(unix, not(target_os = "macos")))]
    xdg_app_id: RefCell<Option<crate::SharedString>>,
    pub(crate) window_shown_hook:
        RefCell<Option<Box<dyn FnMut(&Rc<dyn crate::platform::WindowAdapter>)>>>,
    pub(crate) window_event_hook: RefCell<Option<WindowEventHook>>,

    contexts: RefCell<Vec<SlintContextWeak>>,
}

impl PlatformContext {
    fn new(platform: Box<dyn Platform>) -> core::pin::Pin<Rc<Self>> {
        Rc::pin(Self {
            platform,
            keepalive_count: Cell::new(0),
            color_scheme: Property::new_named(
                ColorScheme::Unknown,
                "PlatformContext::color_scheme",
            ),
            accent_color: Property::new_named(Color::default(), "PlatformContext::accent_color"),
            platform_default_font_size: Property::new_named(
                None,
                "PlatformContext::platform_default_font_size",
            ),
            modifiers: Cell::new(Default::default()),
            #[cfg(all(unix, not(target_os = "macos")))]
            xdg_app_id: Default::default(),
            window_shown_hook: Default::default(),
            window_event_hook: Default::default(),
            contexts: Default::default(),
        })
    }

    fn acquire_keepalive(&self) {
        self.keepalive_count.set(self.keepalive_count.get() + 1);
    }

    fn release_keepalive(&self) {
        let count = self.keepalive_count.get() - 1;
        self.keepalive_count.set(count);
        if count <= 0 {
            let _ = self.platform.new_event_loop_proxy().and_then(|p| p.quit_event_loop().ok());
        }
    }

    pub(crate) fn activate_timers(&self, now: crate::animations::Instant) -> bool {
        let mut activated = false;
        self.for_each_context(|ctx| activated |= ctx.maybe_activate_timers(now));
        activated
    }

    pub(crate) fn next_timer_timeout(&self) -> Option<crate::animations::Instant> {
        let mut next = None;
        self.for_each_context(|ctx| {
            if let Some(timeout) = ctx.next_timer_timeout() {
                next = Some(next.map_or(timeout, |next| timeout.min(next)));
            }
        });
        next
    }

    fn register(&self, ctx: &SlintContext) {
        let weak = ctx.downgrade();
        let mut contexts = self.contexts.borrow_mut();
        match contexts.iter_mut().find(|slot| slot.upgrade().is_none()) {
            Some(slot) => *slot = weak,
            None => contexts.push(weak),
        }
    }

    fn for_each_context(&self, mut f: impl FnMut(&SlintContext)) {
        let mut index = 0;
        loop {
            let Some(weak) = self.contexts.borrow().get(index).cloned() else { break };
            if let Some(ctx) = weak.upgrade() {
                f(&ctx);
            }
            index += 1;
        }
    }
}

#[must_use]
pub(crate) struct KeepAlive(PinWeak<PlatformContext>);

impl Drop for KeepAlive {
    fn drop(&mut self) {
        if let Some(platform_context) = self.0.upgrade() {
            platform_context.release_keepalive();
        }
    }
}

#[pin_project::pin_project]
pub(crate) struct SlintContextInner {
    platform_context: core::pin::Pin<Rc<PlatformContext>>,

    /// Read by all translations, and marked dirty when the language changes so every
    /// translated string re-translates. The value is the currently selected language
    /// when bundling translations.
    #[pin]
    pub(crate) translations_dirty: Property<usize>,
    /// The bundled languages. `translations_dirty` holds the index of the selected one.
    pub(crate) translations_bundle:
        core::cell::RefCell<Option<crate::translations::BundledLanguages>>,
    #[cfg(feature = "tr")]
    external_translator: core::cell::RefCell<Option<Box<dyn tr::Translator>>>,
    #[pin]
    pub(crate) locale_decimal_separator: Property<char>,

    pub(crate) log_message_handler: RefCell<Option<crate::debug_log::LogMessageHandler>>,
    #[cfg(feature = "shared-parley")]
    pub(crate) font_context: core::cell::RefCell<crate::textlayout::sharedparley::FontContext>,
    #[cfg(feature = "shared-swash")]
    pub(crate) swash_scale_context: core::cell::RefCell<swash::scale::ScaleContext>,

    /// The timers registered on this context. Shared, so that `Timer` handles can hold a
    /// `Weak` to the list they registered in without knowing which context owns it.
    pub(crate) timers: crate::timers::TimerListRc,
}

/// This context is meant to hold the state and the backend.
/// Several contexts can share one platform, see [`Self::new_sharing_platform`].
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
        // Timers started before this thread had a context registered in the pending list;
        // take it over so those timers keep working.
        let this =
            Self::on_platform(PlatformContext::new(platform), crate::timers::take_pending_timers());
        // Claim this thread's context slot if it is still free, so that the ambient APIs
        // resolve here rather than to a list nothing drives.
        GLOBAL_CONTEXT.with(|slot| {
            let _ = slot.set(this.clone());
        });
        this.platform().bind_context(this.downgrade(), crate::InternalToken);
        this
    }

    /// Creates a context on the same platform as this one.
    ///
    /// The two share the event loop, the keepalive count, the window hooks, and the xdg app id.
    /// They also share what the platform reports about the system,
    /// such as the color scheme and keyboard modifiers.
    /// Each keeps its own timers, translations, locale, log handler, and fonts.
    /// Driving any context on the platform with [`Self::update_timers_and_animations`] drives all of them.
    ///
    /// The new context doesn't become this thread's current one.
    ///
    /// The platform stays bound to the context that created it.
    /// Backends reach the siblings through that one only, so it has to outlive them.
    pub fn new_sharing_platform(&self) -> Self {
        Self::on_platform(self.0.platform_context.clone(), Default::default())
    }

    fn on_platform(
        platform_context: core::pin::Pin<Rc<PlatformContext>>,
        timers: crate::timers::TimerListRc,
    ) -> Self {
        #[cfg(feature = "shared-parley")]
        let collection = crate::font_collection::take_or_create();

        let this = Self(Rc::pin(SlintContextInner {
            platform_context,

            translations_dirty: Property::new_named(0, "SlintContext::translations"),
            translations_bundle: Default::default(),
            #[cfg(feature = "tr")]
            external_translator: Default::default(),
            locale_decimal_separator: Property::new_named(
                i_slint_common::DEFAULT_DECIMAL_SEPARATOR,
                "SlintContext::locale_decimal_separator",
            ),

            log_message_handler: Default::default(),
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
            timers,
        }));
        // The list's deadlines are measured on this context's clock from now on. Done after
        // construction because it needs a handle to the context that owns it.
        crate::timers::set_owning_context(&this.0.timers, &this);
        this.platform_context().register(&this);
        this
    }

    /// This thread's context, or `None` if none was created yet or the thread is exiting.
    ///
    /// Setting a platform creates the context, and creating a component needs a platform.
    pub fn current() -> Option<Self> {
        GLOBAL_CONTEXT.try_with(|slot| slot.get().cloned()).ok().flatten()
    }

    /// Return a reference to the platform abstraction
    pub fn platform(&self) -> &dyn Platform {
        &*self.platform_context().get_ref().platform
    }

    pub(crate) fn modifiers(&self) -> &Cell<InternalKeyboardModifierState> {
        &self.platform_context().get_ref().modifiers
    }

    pub(crate) fn platform_context(&self) -> core::pin::Pin<&PlatformContext> {
        self.0.platform_context.as_ref()
    }

    /// Creates a window adapter with this context's platform, and associates its window
    /// with this context.
    pub fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let adapter = self.platform().create_window_adapter()?;
        crate::window::WindowInner::from_pub(adapter.window()).set_context(self.clone());
        Ok(adapter)
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
        self.platform().new_event_loop_proxy()
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
        self.platform().run_event_loop()
    }

    /// Creates a [`Timer`](crate::timers::Timer) that registers on this context rather than
    /// on whichever one is current when it is started.
    ///
    /// For the context that a thread runs its event loop on this is the same as
    /// `Timer::default()`, and the event loop activates the timer as usual,
    /// as it does for the contexts sharing that one's platform.
    /// On any other context, the owner is responsible for calling [`Self::maybe_activate_timers`].
    pub fn new_timer(&self) -> crate::timers::Timer {
        crate::timers::Timer::with_list(&self.0.timers)
    }

    /// Starts `timer` like [`Timer::start`](crate::timers::Timer::start), but registers it on
    /// this context if it hasn't been started yet.
    ///
    /// A timer that already belongs to a live context keeps firing on that one.
    pub fn start_timer(
        &self,
        timer: &crate::timers::Timer,
        mode: crate::timers::TimerMode,
        interval: core::time::Duration,
        callback: impl FnMut() + 'static,
    ) {
        timer.start_on(self, mode, interval, callback);
    }

    /// Runs `callback` once, `duration` from now, on this context.
    ///
    /// The context-bound counterpart of [`Timer::single_shot`](crate::timers::Timer::single_shot),
    /// which registers on whichever context is current instead.
    pub fn single_shot(&self, duration: core::time::Duration, callback: impl FnOnce() + 'static) {
        crate::timers::single_shot_on(&self.0.timers, duration, callback);
    }

    /// Advances the animations and the timers of every context on this context's platform,
    /// and runs any change handlers that fall out of it.
    ///
    /// This is what an event loop driving the platform should call at the top of each
    /// iteration. [`crate::platform::update_timers_and_animations`] is the same thing for
    /// whichever context is this thread's global one.
    pub fn update_timers_and_animations(&self) {
        let now = crate::animations::Instant::now(self);
        crate::animations::update_animations(now);
        self.platform_context().activate_timers(now);
        crate::properties::ChangeTracker::run_change_handlers();
    }

    /// How long the event loop driving this context's platform can go to sleep,
    /// before the next timer of any context on it is due.
    /// Returns `None` when none has an active timer.
    pub fn duration_until_next_timer_update(&self) -> Option<core::time::Duration> {
        let timeout = self.platform_context().next_timer_timeout()?;
        let now = crate::animations::Instant::now(self);
        Some(timeout - now)
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
        self.platform_context().project_ref().color_scheme.get()
    }

    /// Backend-side write path for the process-wide color scheme. Called by each
    /// platform's system-theme observer; `Property::set` short-circuits no-op writes.
    pub fn set_color_scheme(&self, scheme: ColorScheme) {
        self.platform_context().project_ref().color_scheme.set(scheme);
    }

    /// Returns the process-wide system accent color. Reads register a property dependency,
    /// so bindings re-evaluate when the platform reports an accent-color change.
    pub fn accent_color(&self) -> Color {
        self.platform_context().project_ref().accent_color.get()
    }

    /// Backend-side write path for the process-wide accent color. Called by each
    /// platform's system-theme observer; `Property::set` short-circuits no-op writes.
    pub fn set_accent_color(&self, color: Color) {
        self.platform_context().project_ref().accent_color.set(color);
    }

    /// Returns the platform-reported default font size, or `None` if the backend doesn't
    /// report one. Reads register a property dependency, so bindings re-evaluate when the
    /// platform reports a change (e.g. the user adjusts the system text size).
    pub fn platform_default_font_size(&self) -> Option<LogicalLength> {
        self.platform_context().project_ref().platform_default_font_size.get()
    }

    /// Backend-side write path for the platform-reported default font size. Called by
    /// backends that track the system setting; `Property::set` short-circuits no-op writes.
    pub fn set_platform_default_font_size(&self, size: Option<LogicalLength>) {
        self.platform_context().project_ref().platform_default_font_size.set(size);
    }

    #[doc(hidden)]
    pub fn dispatch_log_message(&self, message: crate::debug_log::LogMessage<'_>) {
        if let Some(handler) = self.0.log_message_handler.borrow().as_ref() {
            handler(message);
        } else {
            self.platform().debug_log(message.message_arguments());
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

    pub(crate) fn keep_alive(&self) -> KeepAlive {
        self.platform_context().acquire_keepalive();
        KeepAlive(PinWeak::downgrade(self.0.platform_context.clone()))
    }

    pub(crate) fn keep_alive_forever(&self) {
        self.platform_context().acquire_keepalive();
    }

    pub fn set_xdg_app_id(&self, _app_id: crate::SharedString) {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            self.platform_context().xdg_app_id.replace(Some(_app_id));
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    pub fn xdg_app_id(&self) -> Option<crate::SharedString> {
        self.platform_context().xdg_app_id.borrow().clone()
    }

    #[cfg(not(all(unix, not(target_os = "macos"))))]
    pub fn xdg_app_id(&self) -> Option<crate::SharedString> {
        None
    }

    /// Returns the locale's decimal separator, falling back to `translations::DEFAULT_SEPARATOR`.
    pub fn locale_decimal_separator(&self) -> char {
        self.0.as_ref().project_ref().locale_decimal_separator.get()
    }

    /// Format a number using this context's decimal separator.
    pub fn format_number(&self, n: f64) -> crate::SharedString {
        crate::string::format_number(self.locale_decimal_separator(), n)
    }

    /// Format a number with a fixed number of digits after the decimal point,
    /// using this context's decimal separator.
    pub fn format_number_fixed(&self, n: f64, digits: usize) -> crate::SharedString {
        crate::string::format_number_fixed(self.locale_decimal_separator(), n, digits)
    }

    /// Format a number with the given precision, using this context's decimal separator.
    pub fn format_number_precision(&self, n: f64, precision: usize) -> crate::SharedString {
        crate::string::format_number_precision(self.locale_decimal_separator(), n, precision)
    }

    /// Parse a number written with this context's decimal separator.
    pub fn parse_number(&self, string: &str) -> Option<f32> {
        crate::string::parse_number(self.locale_decimal_separator(), string)
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
            let _prefetch = crate::font_collection::prefetch();
            crate::platform::set_platform(factory()?).map_err(PlatformError::SetPlatformError)?;
            Ok(f(p.get().unwrap()))
        }
    })
}

/// Run `f` with this thread's context.
///
/// Unlike [`with_global_context`], this never creates a platform.
/// It fails with [`SetPlatformError::AlreadySet`](crate::platform::SetPlatformError::AlreadySet)
/// when another thread installed the event-loop proxy through [`set_platform`](crate::platform::set_platform),
/// and with [`PlatformError::NoPlatform`] otherwise.
pub fn with_existing_context<R>(f: impl FnOnce(&SlintContext) -> R) -> Result<R, PlatformError> {
    GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => Ok(f(ctx)),
        None if crate::platform::with_event_loop_proxy(|proxy| proxy.is_some()) => {
            Err(PlatformError::SetPlatformError(crate::platform::SetPlatformError::AlreadySet))
        }
        None => Err(PlatformError::NoPlatform),
    })
}

#[test]
#[cfg(feature = "std")]
fn with_existing_context_never_creates_a_platform() {
    assert!(matches!(with_existing_context(|_| ()), Err(PlatformError::NoPlatform)));

    let _ctx = SlintContext::new(Box::new(crate::testing::NoWindowPlatform));
    assert_eq!(with_existing_context(|_| 42).unwrap(), 42);
}

/// Internal function to set a hook that's invoked whenever a slint::Window is shown. This
/// is used by the system testing module. Returns a previously set hook, if any.
pub fn set_window_shown_hook(
    hook: Option<Box<dyn FnMut(&Rc<dyn crate::platform::WindowAdapter>)>>,
) -> Result<Option<Box<dyn FnMut(&Rc<dyn crate::platform::WindowAdapter>)>>, PlatformError> {
    replace_hook(hook, |platform_context| &platform_context.window_shown_hook, "shown")
}

/// Internal function to set a hook that's invoked after a window event was dispatched.
/// This is used by the system testing module. Returns a previously set hook, if any.
pub fn set_window_event_hook(
    hook: Option<WindowEventHook>,
) -> Result<Option<WindowEventHook>, PlatformError> {
    replace_hook(hook, |platform_context| &platform_context.window_event_hook, "event")
}

fn replace_hook<T>(
    hook: Option<T>,
    slot: fn(&PlatformContext) -> &RefCell<Option<T>>,
    name: &str,
) -> Result<Option<T>, PlatformError> {
    GLOBAL_CONTEXT.with(|p| {
        let ctx = p.get().ok_or(PlatformError::NoPlatform)?;
        let platform_context = ctx.platform_context();
        let mut slot = slot(platform_context.get_ref())
            .try_borrow_mut()
            .map_err(|_| PlatformError::Other(alloc::format!("{name} hook is currently in use")))?;
        Ok(core::mem::replace(&mut *slot, hook))
    })
}

#[cfg(all(test, feature = "std"))]
mod platform_sharing_tests {
    use super::*;
    use core::time::Duration;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Default)]
    struct Probe {
        time: Cell<Duration>,
        bind_count: Cell<usize>,
        quit: Arc<AtomicBool>,
    }

    struct TestPlatform(Rc<Probe>);

    impl Platform for TestPlatform {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Err(PlatformError::NoPlatform)
        }
        fn duration_since_start(&self) -> Duration {
            self.0.time.get()
        }
        fn bind_context(&self, _ctx: SlintContextWeak, _: crate::InternalToken) {
            self.0.bind_count.set(self.0.bind_count.get() + 1);
        }
        fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
            Some(Box::new(QuitRecorder(self.0.quit.clone())))
        }
    }

    struct QuitRecorder(Arc<AtomicBool>);

    impl EventLoopProxy for QuitRecorder {
        fn quit_event_loop(&self) -> Result<(), crate::api::EventLoopError> {
            self.0.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn invoke_from_event_loop(
            &self,
            _event: Box<dyn FnOnce() + Send>,
        ) -> Result<(), crate::api::EventLoopError> {
            Ok(())
        }
    }

    fn siblings() -> (Rc<Probe>, SlintContext, SlintContext) {
        let probe = Rc::new(Probe::default());
        let a = SlintContext::new(Box::new(TestPlatform(probe.clone())));
        let b = a.new_sharing_platform();
        (probe, a, b)
    }

    #[test]
    fn sibling_timer_fires_when_the_original_context_is_driven() {
        let (probe, a, b) = siblings();
        let fired = Rc::new(Cell::new(false));
        let fired_ = fired.clone();
        b.single_shot(Duration::from_millis(10), move || fired_.set(true));

        probe.time.set(Duration::from_millis(20));
        a.update_timers_and_animations();
        assert!(fired.get());
    }

    #[test]
    fn a_nested_update_from_a_timer_callback_drives_the_siblings() {
        let (probe, a, b) = siblings();
        let b_fired = Rc::new(Cell::new(0));
        let b_fired_ = b_fired.clone();
        let fired_while_nested = Rc::new(Cell::new(0));
        let fired_while_nested_ = fired_while_nested.clone();
        let a_ = a.clone();
        a.single_shot(Duration::from_millis(10), move || {
            a_.update_timers_and_animations();
            fired_while_nested_.set(b_fired_.get());
        });
        let b_fired_ = b_fired.clone();
        b.single_shot(Duration::from_millis(10), move || b_fired_.set(b_fired_.get() + 1));

        probe.time.set(Duration::from_millis(20));
        a.update_timers_and_animations();
        assert_eq!(fired_while_nested.get(), 1);
        assert_eq!(b_fired.get(), 1);
    }

    #[test]
    fn a_context_created_after_a_sibling_dropped_is_driven_once() {
        let (probe, a, b) = siblings();
        drop(b);
        let c = a.new_sharing_platform();
        assert_eq!(a.platform_context().contexts.borrow().len(), 2);

        let a_fired = Rc::new(Cell::new(0));
        let a_fired_ = a_fired.clone();
        a.single_shot(Duration::from_millis(10), move || a_fired_.set(a_fired_.get() + 1));
        let c_fired = Rc::new(Cell::new(0));
        let c_fired_ = c_fired.clone();
        c.single_shot(Duration::from_millis(10), move || c_fired_.set(c_fired_.get() + 1));

        probe.time.set(Duration::from_millis(20));
        a.update_timers_and_animations();
        assert_eq!((a_fired.get(), c_fired.get()), (1, 1));
    }

    #[test]
    fn next_timer_update_is_the_earliest_across_siblings() {
        let (_probe, a, b) = siblings();
        a.single_shot(Duration::from_millis(50), || {});
        b.single_shot(Duration::from_millis(10), || {});

        assert_eq!(a.duration_until_next_timer_update(), Some(Duration::from_millis(10)));
        assert_eq!(b.duration_until_next_timer_update(), Some(Duration::from_millis(10)));
    }

    #[test]
    fn event_loop_quits_when_the_last_keepalive_of_any_sibling_is_released() {
        let (probe, a, b) = siblings();
        let a_keepalive = a.keep_alive();
        let b_keepalive = b.keep_alive();

        drop(a_keepalive);
        assert!(!probe.quit.load(Ordering::SeqCst));
        drop(b_keepalive);
        assert!(probe.quit.load(Ordering::SeqCst));
    }

    #[test]
    fn a_keepalive_outlives_its_context() {
        let (probe, a, b) = siblings();
        let keepalive = b.keep_alive();
        drop(b);
        drop(keepalive);
        assert!(probe.quit.load(Ordering::SeqCst));
        assert_eq!(a.platform_context().keepalive_count.get(), 0);
    }

    #[test]
    fn system_settings_are_shared_between_siblings() {
        let (_probe, a, b) = siblings();

        let scheme = Property::new(ColorScheme::Unknown);
        let b_ = b.clone();
        scheme.set_binding(move || b_.color_scheme(None));
        let accent = Property::new(Color::default());
        let b_ = b.clone();
        accent.set_binding(move || b_.accent_color());
        let font_size = Property::new(None);
        let b_ = b.clone();
        font_size.set_binding(move || b_.platform_default_font_size());
        let (scheme, accent, font_size) =
            (core::pin::pin!(scheme), core::pin::pin!(accent), core::pin::pin!(font_size));
        assert_eq!(scheme.as_ref().get(), ColorScheme::Unknown);
        assert_eq!(accent.as_ref().get(), Color::default());
        assert_eq!(font_size.as_ref().get(), None);

        let red = Color::from_rgb_u8(255, 0, 0);
        let size = Some(LogicalLength::new(17.));
        a.set_color_scheme(ColorScheme::Dark);
        a.set_accent_color(red);
        a.set_platform_default_font_size(size);

        assert_eq!(b.color_scheme(None), ColorScheme::Dark);
        assert_eq!(scheme.as_ref().get(), ColorScheme::Dark);
        assert_eq!(accent.as_ref().get(), red);
        assert_eq!(font_size.as_ref().get(), size);
    }

    #[test]
    fn translations_and_locale_stay_per_context() {
        let (_probe, a, b) = siblings();

        a.set_bundled_languages([("en".into(), '.'), ("de".into(), ',')]);
        a.select_bundled_translation("de").unwrap();
        assert_eq!(a.locale_decimal_separator(), ',');
        assert!(b.0.translations_bundle.borrow().is_none());
        assert_eq!(b.0.as_ref().project_ref().translations_dirty.get(), 0);
        assert_eq!(b.locale_decimal_separator(), i_slint_common::DEFAULT_DECIMAL_SEPARATOR);

        b.set_locale("fr");
        assert_eq!(a.locale_decimal_separator(), ',');
    }

    #[test]
    fn sharing_the_platform_does_not_bind_it_again() {
        let (probe, a, _b) = siblings();
        let _c = a.new_sharing_platform();
        assert_eq!(probe.bind_count.get(), 1);
    }
}
