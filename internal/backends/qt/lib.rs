// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore deinit fnbox qsize

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
// Bumped from 2048 to accommodate the number of rust!() invocations inside
// the large cpp! {{ }} block in qt_window.rs (gesture/input event handling).
#![recursion_limit = "4096"]
#![cfg_attr(slint_nightly_test, feature(non_exhaustive_omitted_patterns_lint))]
#![cfg_attr(slint_nightly_test, warn(non_exhaustive_omitted_patterns))]

extern crate alloc;

use i_slint_core::platform::PlatformError;
use std::rc::Rc;
#[cfg(not(no_qt))]
use std::sync::{Arc, atomic::AtomicUsize};

#[cfg(not(no_qt))]
thread_local! {
    /// Set once by [`Backend::bind_context`]; read from rust!() callbacks fired by the
    /// Qt event filter installed on `qApp` so palette/theme/font changes can push the new
    /// values onto the process-wide [`i_slint_core::SlintContext`] without going through
    /// any specific [`qt_window::QtWindow`].
    static QT_CONTEXT: std::cell::OnceCell<i_slint_core::SlintContextWeak> =
        const { std::cell::OnceCell::new() };
}

#[cfg(not(no_qt))]
mod qt_accessible;
#[cfg(not(no_qt))]
mod qt_widgets;
#[cfg(not(no_qt))]
mod qt_window;

mod accessible_generated;
mod key_generated;

#[cfg(no_qt)]
mod ffi {
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_qt_get_widget(
        _: &i_slint_core::window::WindowAdapterRc,
    ) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

/// NativeWidgets and NativeGlobals are "type list" containing all the native widgets and global types.
///
/// It is built as a tuple `(Type, Tail)`  where `Tail` is also a "type list". a `()` is the end.
///
/// So it can be used like this to do something for all types:
///
/// ```rust
/// trait DoSomething {
///     fn do_something(/*...*/) { /*...*/
///     }
/// }
/// impl DoSomething for () {}
/// impl<T: i_slint_core::rtti::BuiltinItem, Next: DoSomething> DoSomething for (T, Next) {
///     fn do_something(/*...*/) {
///          /*...*/
///          Next::do_something(/*...*/);
///     }
/// }
/// i_slint_backend_qt::NativeWidgets::do_something(/*...*/)
/// ```
#[cfg(not(no_qt))]
#[rustfmt::skip]
pub type NativeWidgets =
    (qt_widgets::NativeButton,
    (qt_widgets::NativeCheckBox,
    (qt_widgets::NativeSlider,
    (qt_widgets::NativeProgressIndicator,
    (qt_widgets::NativeSpinBox,
    (qt_widgets::NativeGroupBox,
    (qt_widgets::NativeLineEdit,
    (qt_widgets::NativeScrollView,
    (qt_widgets::NativeStandardListViewItem,
    (qt_widgets::NativeTableHeaderSection,
    (qt_widgets::NativeComboBox,
    (qt_widgets::NativeComboBoxPopup,
    (qt_widgets::NativeTabWidget,
    (qt_widgets::NativeTab,
            ()))))))))))))));

#[cfg(not(no_qt))]
#[rustfmt::skip]
pub type NativeGlobals =
    (qt_widgets::NativeStyleMetrics,
    (qt_widgets::NativePalette,
        ()));

#[cfg(no_qt)]
mod native_style_metrics_stub {
    use const_field_offset::FieldOffsets;
    use core::pin::Pin;
    #[cfg(feature = "rtti")]
    use i_slint_core::rtti::*;
    use i_slint_core_macros::*;

    /// cbindgen:ignore
    #[repr(C)]
    #[derive(FieldOffsets, SlintElement)]
    #[pin]
    #[pin_drop]
    pub struct NativeStyleMetrics {}

    impl const_field_offset::PinnedDrop for NativeStyleMetrics {
        fn drop(self: Pin<&mut Self>) {}
    }

    /// cbindgen:ignore
    #[repr(C)]
    #[derive(FieldOffsets, SlintElement)]
    #[pin]
    #[pin_drop]
    pub struct NativePalette {}

    impl const_field_offset::PinnedDrop for NativePalette {
        fn drop(self: Pin<&mut Self>) {}
    }
}

pub mod native_widgets {
    #[cfg(not(no_qt))]
    pub use super::qt_widgets::*;

    #[cfg(no_qt)]
    pub use super::native_style_metrics_stub::NativeStyleMetrics;

    #[cfg(no_qt)]
    pub use super::native_style_metrics_stub::NativePalette;
}

#[cfg(no_qt)]
pub type NativeWidgets = ();
#[cfg(no_qt)]
pub type NativeGlobals = ();

pub const HAS_NATIVE_STYLE: bool = cfg!(not(no_qt));

pub struct Backend {
    #[cfg(not(no_qt))]
    /// The generation is used to determine if a quit_event_loop call is meant for the current
    /// event loop or is from a stale event.
    event_loop_generation: Arc<AtomicUsize>,
}

impl Default for Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend {
    pub fn new() -> Self {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            // Initialize QApplication early for High-DPI support on Windows,
            // before the first calls to QStyle.
            cpp! {unsafe[] {
                ensure_initialized(true);
            }}
        }
        Self {
            #[cfg(not(no_qt))]
            event_loop_generation: Default::default(),
        }
    }
}

impl i_slint_core::platform::Platform for Backend {
    #[cfg(not(no_qt))]
    fn bind_context(&self, ctx: i_slint_core::SlintContextWeak, _: i_slint_core::InternalToken) {
        QT_CONTEXT.with(|cell| {
            let _ = cell.set(ctx);
        });
        // Read the host shell's current values once and push them to the context, then
        // install an `qApp`-level event filter that re-pushes whenever Qt reports a
        // theme/palette/font change. The previous design did the read in `QtWindow::new` and
        // the change-tracking in each window's `changeEvent`, which is wasteful when
        // there are multiple windows and outright broken when there are zero windows.
        update_palette_state();
        update_font_state();
        install_app_state_observer();
    }

    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn i_slint_core::window::WindowAdapter>, PlatformError> {
        #[cfg(no_qt)]
        return Err("Qt platform requested but Slint is compiled without Qt support".into());
        #[cfg(not(no_qt))]
        {
            Ok(qt_window::QtWindow::new(std::rc::Weak::<qt_window::QtWindow>::new()))
        }
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        #[cfg(not(no_qt))]
        {
            // Schedule any timers with Qt that were set up before this event loop start.
            crate::qt_window::timer_event();
            use cpp::cpp;
            // Note: fetch_add wraps on overflow, which is what we want here.
            self.event_loop_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            cpp! {unsafe [] {
                ensure_initialized(true);
                qApp->exec();
            } }
            Ok(())
        }
        #[cfg(no_qt)]
        Err("Qt platform requested but Slint is compiled without Qt support".into())
    }

    fn process_events(
        &self,
        _timeout: Option<core::time::Duration>,
        _: i_slint_core::InternalToken,
    ) -> Result<core::ops::ControlFlow<()>, PlatformError> {
        #[cfg(not(no_qt))]
        {
            // Schedule any timers with Qt that were set up before this event loop start.
            crate::qt_window::timer_event();
            use cpp::cpp;
            let timeout_ms: i32 = _timeout.map_or(-1, |d| d.as_millis() as i32);
            let loop_was_quit = cpp! {unsafe [timeout_ms as "int"] -> bool as "bool" {
                ensure_initialized(true);
                qApp->processEvents(QEventLoop::AllEvents, timeout_ms);
                return std::exchange(g_lastWindowClosed, false);
            } };
            Ok(if loop_was_quit {
                core::ops::ControlFlow::Break(())
            } else {
                core::ops::ControlFlow::Continue(())
            })
        }
        #[cfg(no_qt)]
        Err("Qt platform requested but Slint is compiled without Qt support".into())
    }

    #[cfg(not(no_qt))]
    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        struct Proxy(Arc<AtomicUsize>);
        impl i_slint_core::platform::EventLoopProxy for Proxy {
            fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
                let generation_now = self.0.load(std::sync::atomic::Ordering::Relaxed);
                let generation = Arc::clone(&self.0);
                // Note: Invoke QCoreApplication::exit(0) from the event loop as its thread-safety
                // is unspecified.
                self.invoke_from_event_loop(Box::new(move || {
                    if generation.load(std::sync::atomic::Ordering::Relaxed) == generation_now {
                        use cpp::cpp;
                        cpp! {unsafe [] {
                            // Note: Use exit instead of qApp->quit().
                            //
                            // As per commit 0c02f133f3daee146b805149e69bba8cee6727b2 in qtbase (qt6),
                            // quit() on QCoreApplication on macOS calls [NSApp terminate], which will
                            // not return to main. The latter however is documented behavior, and
                            // slint-viewer for example relies on the ability to return from run().
                            QCoreApplication::exit(0);
                        } }
                    }
                }))
            }

            fn invoke_from_event_loop(
                &self,
                _event: Box<dyn FnOnce() + Send>,
            ) -> Result<(), i_slint_core::api::EventLoopError> {
                use cpp::cpp;
                cpp! {{
                   struct TraitObject { void *a, *b; };
                   struct EventHolder {
                       TraitObject fnbox = {nullptr, nullptr};
                       ~EventHolder() {
                           if (fnbox.a != nullptr || fnbox.b != nullptr) {
                               rust!(Slint_delete_event_holder [fnbox: *mut dyn FnOnce() as "TraitObject"] {
                                   unsafe { drop(Box::from_raw(fnbox)) }
                               });
                           }
                       }
                       EventHolder(TraitObject f) : fnbox(f)  {}
                       EventHolder(const EventHolder&) = delete;
                       EventHolder& operator=(const EventHolder&) = delete;
                       EventHolder(EventHolder&& other) : fnbox(other.fnbox) {
                            other.fnbox = {nullptr, nullptr};
                       }
                       void operator()() {
                            if (fnbox.a != nullptr || fnbox.b != nullptr) {
                                TraitObject fnbox = std::move(this->fnbox);
                                this->fnbox = {nullptr, nullptr};
                                rust!(Slint_call_event_holder [fnbox: *mut dyn FnOnce() as "TraitObject"] {
                                   let b = unsafe { Box::from_raw(fnbox) };
                                   b();
                                   // in case the callback started a new timer
                                   crate::qt_window::restart_timer();
                                });
                            }

                       }
                   };
                }};
                let fnbox = Box::into_raw(_event);
                cpp! {unsafe [fnbox as "TraitObject"] {
                    QTimer::singleShot(0, qApp, EventHolder{fnbox});
                }}
                Ok(())
            }
        }
        Some(Box::new(Proxy(Arc::clone(&self.event_loop_generation))))
    }

    #[cfg(not(no_qt))]
    fn set_clipboard_text(&self, _text: &str, _clipboard: i_slint_core::platform::Clipboard) {
        use cpp::cpp;
        let is_selection: bool = match _clipboard {
            i_slint_core::platform::Clipboard::DefaultClipboard => false,
            i_slint_core::platform::Clipboard::SelectionClipboard => true,
            _ => return,
        };
        let text: qttypes::QString = _text.into();
        cpp! {unsafe [text as "QString", is_selection as "bool"] {
            ensure_initialized();
            if (is_selection && !QGuiApplication::clipboard()->supportsSelection())
                return;
            QGuiApplication::clipboard()->setText(text, is_selection ? QClipboard::Selection : QClipboard::Clipboard);
        } }
    }

    #[cfg(not(no_qt))]
    fn clipboard_text(&self, _clipboard: i_slint_core::platform::Clipboard) -> Option<String> {
        use cpp::cpp;
        let is_selection: bool = match _clipboard {
            i_slint_core::platform::Clipboard::DefaultClipboard => false,
            i_slint_core::platform::Clipboard::SelectionClipboard => true,
            _ => return None,
        };
        let has_text = cpp! {unsafe [is_selection as "bool"] -> bool as "bool" {
            ensure_initialized();
            if (is_selection && !QGuiApplication::clipboard()->supportsSelection())
                return false;
            return QGuiApplication::clipboard()->mimeData(is_selection ? QClipboard::Selection : QClipboard::Clipboard)->hasText();
        } };
        if has_text {
            return Some(
                cpp! { unsafe [is_selection as "bool"] -> qttypes::QString as "QString" {
                    return QGuiApplication::clipboard()->text(is_selection ? QClipboard::Selection : QClipboard::Clipboard);
                }}
                .into(),
            );
        }
        None
    }

    #[cfg(not(no_qt))]
    fn click_interval(&self) -> core::time::Duration {
        let duration_ms = unsafe {
            cpp::cpp! {[] -> u32 as "int" { return qApp->doubleClickInterval(); }}
        };
        core::time::Duration::from_millis(duration_ms as u64)
    }

    #[cfg(not(no_qt))]
    fn cursor_flash_cycle(&self) -> core::time::Duration {
        let duration_ms = unsafe {
            cpp::cpp! {[] -> i32 as "int" { return qApp->cursorFlashTime(); }}
        };
        if duration_ms > 0 {
            core::time::Duration::from_millis(duration_ms as u64)
        } else {
            core::time::Duration::ZERO
        }
    }

    #[cfg(not(no_qt))]
    fn open_url(&self, url: &str) -> Result<(), i_slint_core::platform::PlatformError> {
        let url: qttypes::QString = url.into();
        let success = unsafe {
            cpp::cpp! { [url as "QString"] -> bool as "bool" {
                return QDesktopServices::openUrl(url);
            }}
        };
        if success {
            Ok(())
        } else {
            Err(i_slint_core::platform::PlatformError::Other("Failed to open URL".into()))
        }
    }
}

#[cfg(not(no_qt))]
fn update_palette_state() {
    use cpp::cpp;
    let dark = cpp! {unsafe [] -> bool as "bool" {
        return qApp->palette().color(QPalette::Window).valueF() < 0.5;
    }};
    let argb = cpp! {unsafe [] -> u32 as "QRgb" {
        #if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
            return qApp->palette().color(QPalette::Accent).rgba();
        #else
            return qApp->palette().color(QPalette::Highlight).rgba();
        #endif
    }};
    let scheme = if dark {
        i_slint_core::items::ColorScheme::Dark
    } else {
        i_slint_core::items::ColorScheme::Light
    };
    let accent = i_slint_core::graphics::Color::from_argb_encoded(argb);
    QT_CONTEXT.with(|cell| {
        if let Some(ctx) = cell.get().and_then(|w| w.upgrade()) {
            ctx.set_color_scheme(scheme);
            ctx.set_accent_color(accent);
        }
    });
}

#[cfg(not(no_qt))]
fn update_font_state() {
    use cpp::cpp;
    let default_font_size = cpp! {unsafe [] -> i32 as "int" {
        return QFontInfo(qApp->font()).pixelSize();
    }};
    QT_CONTEXT.with(|cell| {
        if let Some(ctx) = cell.get().and_then(|w| w.upgrade()) {
            ctx.set_platform_default_font_size(Some(i_slint_core::lengths::LogicalLength::new(
                default_font_size as f32,
            )));
        }
    });
}

#[cfg(not(no_qt))]
fn install_app_state_observer() {
    use cpp::cpp;
    cpp! {{
        #include <QtCore/QEvent>
        #include <QtCore/QObject>
        #include <QtGui/QFontInfo>
        #include <QtWidgets/QApplication>

        struct SlintAppStateObserver : QObject {
            using QObject::QObject;
            bool eventFilter(QObject *watched, QEvent *event) override {
                if (watched == qApp) {
                    if (event->type() == QEvent::ApplicationPaletteChange
                        || event->type() == QEvent::ThemeChange) {
                        rust!(Slint_qt_palette_changed [] {
                            crate::update_palette_state();
                        });
                    } else if (event->type() == QEvent::ApplicationFontChange) {
                        rust!(Slint_qt_font_changed [] {
                            crate::update_font_state();
                        });
                    }
                }
                return false;
            }
        };
    }};
    cpp! {unsafe [] {
        ensure_initialized(true);
        // Parented to qApp so it lives as long as the application and is cleaned up
        // automatically on exit. installEventFilter doesn't take ownership.
        auto *observer = new SlintAppStateObserver(qApp);
        qApp->installEventFilter(observer);
    }};
}

/// This helper trait can be used to obtain access to a pointer to a QtWidget for a given
/// [`slint::Window`](slint:rust:slint/struct.window).")]
#[cfg(not(no_qt))]
pub trait QtWidgetAccessor {
    fn qt_widget_ptr(&self) -> Option<std::ptr::NonNull<()>>;
}

#[cfg(not(no_qt))]
impl QtWidgetAccessor for i_slint_core::api::Window {
    fn qt_widget_ptr(&self) -> Option<std::ptr::NonNull<()>> {
        i_slint_core::window::WindowInner::from_pub(self)
            .window_adapter()
            .internal(i_slint_core::InternalToken)
            .and_then(|wa| (wa as &dyn core::any::Any).downcast_ref::<qt_window::QtWindow>())
            .map(qt_window::QtWindow::widget_ptr)
    }
}
