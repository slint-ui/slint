// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore deinit fnbox qsize

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![recursion_limit = "2048"]

extern crate alloc;

use i_slint_core::platform::PlatformError;
use std::rc::Rc;

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
    #[no_mangle]
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

pub struct Backend;

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
        Self {}
    }
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn i_slint_core::window::WindowAdapter>, PlatformError> {
        #[cfg(no_qt)]
        return Err("Qt platform requested but Slint is compiled without Qt support".into());
        #[cfg(not(no_qt))]
        {
            Ok(qt_window::QtWindow::new())
        }
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        #[cfg(not(no_qt))]
        {
            // Schedule any timers with Qt that were set up before this event loop start.
            crate::qt_window::timer_event();
            use cpp::cpp;
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
        _timeout: core::time::Duration,
        _: i_slint_core::InternalToken,
    ) -> Result<core::ops::ControlFlow<()>, PlatformError> {
        #[cfg(not(no_qt))]
        {
            // Schedule any timers with Qt that were set up before this event loop start.
            crate::qt_window::timer_event();
            use cpp::cpp;
            let timeout_ms: i32 = _timeout.as_millis() as _;
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
        struct Proxy;
        impl i_slint_core::platform::EventLoopProxy for Proxy {
            fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
                use cpp::cpp;
                cpp! {unsafe [] {
                    // Use a quit event to avoid qApp->quit() calling
                    // [NSApp terminate:nil] and us never returning from the
                    // event loop - slint-viewer relies on the ability to
                    // return from run().
                    QCoreApplication::postEvent(qApp, new QEvent(QEvent::Quit));
                } }
                Ok(())
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
                                   drop(Box::from_raw(fnbox))
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
                                   let b = Box::from_raw(fnbox);
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
        Some(Box::new(Proxy))
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
}
