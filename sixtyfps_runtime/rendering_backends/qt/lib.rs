/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![recursion_limit = "512"]

use sixtyfps_corelib::window::ComponentWindow;

#[cfg(not(no_qt))]
mod qt_window;
#[cfg(not(no_qt))]
mod qttypes;
#[cfg(not(no_qt))]
mod widgets;

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

pub fn create_window() -> ComponentWindow {
    #[cfg(no_qt)]
    panic!("The Qt backend needs Qt");
    #[cfg(not(no_qt))]
    ComponentWindow::new(qt_window::QtWindow::new())
}

/// This function can be used to register a custom TrueType font with SixtyFPS,
/// for use with the `font-family` property. The provided slice must be a valid TrueType
/// font.
pub fn register_application_font_from_memory(
    _data: &'static [u8],
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO
    Ok(())
}
