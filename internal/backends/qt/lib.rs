// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore deinit fnbox qsize

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
#![recursion_limit = "2048"]

extern crate alloc;

use std::rc::Rc;

#[cfg(not(no_qt))]
mod qt_accessible;
#[cfg(not(no_qt))]
mod qt_widgets;
#[cfg(not(no_qt))]
mod qt_window;

mod accessible_generated;
mod key_generated;

#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() -> usize {
    #[cfg(no_qt)]
    {
        ffi::slint_qt_get_widget as usize
    }
    #[cfg(not(no_qt))]
    {
        qt_window::ffi::slint_qt_get_widget as usize
            + (&qt_widgets::NativeButtonVTable) as *const _ as usize
    }
}

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
    (qt_widgets::NativeSpinBox,
    (qt_widgets::NativeGroupBox,
    (qt_widgets::NativeLineEdit,
    (qt_widgets::NativeScrollView,
    (qt_widgets::NativeStandardListViewItem,
    (qt_widgets::NativeTableColumn,
    (qt_widgets::NativeComboBox,
    (qt_widgets::NativeComboBoxPopup,
    (qt_widgets::NativeTabWidget,
    (qt_widgets::NativeTab,
            ())))))))))))));

#[cfg(not(no_qt))]
#[rustfmt::skip]
pub type NativeGlobals =
    (qt_widgets::NativeStyleMetrics,
        ());

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
}

pub mod native_widgets {
    #[cfg(not(no_qt))]
    pub use super::qt_widgets::*;

    #[cfg(no_qt)]
    pub use super::native_style_metrics_stub::NativeStyleMetrics;
}

#[cfg(no_qt)]
pub type NativeWidgets = ();
#[cfg(no_qt)]
pub type NativeGlobals = ();

pub const HAS_NATIVE_STYLE: bool = cfg!(not(no_qt));

pub struct Backend;
impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(&self) -> Rc<dyn i_slint_core::window::WindowAdapter> {
        #[cfg(no_qt)]
        panic!("The Qt backend needs Qt");
        #[cfg(not(no_qt))]
        {
            qt_window::QtWindow::new()
        }
    }

    fn set_event_loop_quit_on_last_window_closed(&self, _quit_on_last_window_closed: bool) {
        #[cfg(not(no_qt))]
        {
            // Schedule any timers with Qt that were set up before this event loop start.
            use cpp::cpp;
            cpp! {unsafe [_quit_on_last_window_closed as "bool"] {
                ensure_initialized(true);
                qApp->setQuitOnLastWindowClosed(_quit_on_last_window_closed);
            } }
        };
    }

    fn run_event_loop(&self) {
        #[cfg(not(no_qt))]
        {
            // Schedule any timers with Qt that were set up before this event loop start.
            crate::qt_window::timer_event();
            use cpp::cpp;
            cpp! {unsafe [] {
                ensure_initialized(true);
                qApp->exec();
            } }
        };
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
    fn set_clipboard_text(&self, _text: &str) {
        use cpp::cpp;
        let text: qttypes::QString = _text.into();
        cpp! {unsafe [text as "QString"] {
            ensure_initialized();
            QGuiApplication::clipboard()->setText(text);
        } }
    }

    #[cfg(not(no_qt))]
    fn clipboard_text(&self) -> Option<String> {
        use cpp::cpp;
        let has_text = cpp! {unsafe [] -> bool as "bool" {
            ensure_initialized();
            return QGuiApplication::clipboard()->mimeData()->hasText();
        } };
        if has_text {
            return Some(
                cpp! { unsafe [] -> qttypes::QString as "QString" {
                    return QGuiApplication::clipboard()->text();
                }}
                .into(),
            );
        }
        None
    }
}
