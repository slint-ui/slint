/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

/*!

*NOTE*: This library is an internal crate for the [SixtyFPS project](https://sixtyfps.io).
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead.

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]
#![recursion_limit = "512"]

use sixtyfps_corelib::window::ComponentWindow;

#[cfg(not(no_qt))]
mod qt_window;
#[cfg(not(no_qt))]
mod qttypes;
#[cfg(not(no_qt))]
mod widgets;

mod key_generated;

#[doc(hidden)]
#[cold]
pub fn use_modules() -> usize {
    sixtyfps_corelib::use_modules() + {
        #[cfg(no_qt)]
        {
            0
        }
        #[cfg(not(no_qt))]
        {
            (&widgets::NativeButtonVTable) as *const _ as usize
        }
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
/// impl<T: sixtyfps_corelib::rtti::BuiltinItem, Next: DoSomething> DoSomething for (T, Next) {
///     fn do_something(/*...*/) {
///          /*...*/
///          Next::do_something(/*...*/);
///     }
/// }
/// sixtyfps_rendering_backend_qt::NativeWidgets::do_something(/*...*/)
/// ```
#[cfg(not(no_qt))]
#[rustfmt::skip]
pub type NativeWidgets =
    (widgets::NativeButton,
    (widgets::NativeCheckBox,
    (widgets::NativeSlider,
    (widgets::NativeSpinBox,
    (widgets::NativeGroupBox,
    (widgets::NativeLineEdit,
    (widgets::NativeScrollView,
    (widgets::NativeStandardListViewItem,
    (widgets::NativeComboBox,
            ())))))))));

#[cfg(not(no_qt))]
#[rustfmt::skip]
pub type NativeGlobals =
    (widgets::NativeStyleMetrics,
        ());

pub mod native_widgets {
    #[cfg(not(no_qt))]
    pub use super::widgets::*;
}

#[cfg(no_qt)]
pub type NativeWidgets = ();
#[cfg(no_qt)]
pub type NativeGlobals = ();

pub const HAS_NATIVE_STYLE: bool = cfg!(not(no_qt));
/// False if the backend was compiled without Qt so it wouldn't do anything
pub const IS_AVAILABLE: bool = cfg!(not(no_qt));

pub struct Backend;
impl sixtyfps_corelib::backend::Backend for Backend {
    fn create_window(&'static self) -> ComponentWindow {
        #[cfg(no_qt)]
        panic!("The Qt backend needs Qt");
        #[cfg(not(no_qt))]
        {
            let qt_window = qt_window::QtWindow::new();
            let window = std::rc::Rc::new(sixtyfps_corelib::window::Window::new(qt_window.clone()));
            qt_window.self_weak.set(std::rc::Rc::downgrade(&window)).ok().unwrap();
            ComponentWindow::new(window)
        }
    }

    fn run_event_loop(&'static self) {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            cpp! {unsafe [] {
                qApp->exec();
            } }
        };
    }

    fn register_font_from_memory(
        &'static self,
        _data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            let data = qttypes::QByteArray::from(_data);
            cpp! {unsafe [data as "QByteArray"] {
                ensure_initialized();
                QFontDatabase::addApplicationFontFromData(data);
            } }
        };
        Ok(())
    }

    fn register_font_from_path(
        &'static self,
        _path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            let encoded_path: qttypes::QByteArray = _path.to_string_lossy().as_bytes().into();
            cpp! {unsafe [encoded_path as "QByteArray"] {
                ensure_initialized();
                QFontDatabase::addApplicationFont(QFile::decodeName(encoded_path));
            } }
        };
        Ok(())
    }

    fn set_clipboard_text(&'static self, _text: String) {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            let text: qttypes::QString = _text.into();
            cpp! {unsafe [text as "QString"] {
                ensure_initialized();
                QGuiApplication::clipboard()->setText(text);
            } }
        }
    }

    fn clipboard_text(&'static self) -> Option<String> {
        #[cfg(not(no_qt))]
        {
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
        }
        None
    }
}
