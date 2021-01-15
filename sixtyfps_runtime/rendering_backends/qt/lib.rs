/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![recursion_limit = "512"]

use std::rc::Rc;

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
            let window = Rc::new(sixtyfps_corelib::window::Window::new(qt_window.clone()));
            qt_window.self_weak.set(Rc::downgrade(&window)).ok().unwrap();
            ComponentWindow::new(window)
        }
    }

    fn register_application_font_from_memory(
        &'static self,
        _data: &'static [u8],
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
}
