/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#[cfg(not(no_qt))]
mod qttypes;
#[cfg(not(no_qt))]
pub mod widgets;

// FIXME: right now, we are just re-exposing the GL backend, but eventually, we want the Qt
// backend to use QPainter to draw directly on the window.
pub use sixtyfps_rendering_backend_gl::*;

#[doc(hidden)]
#[cold]
pub fn use_modules() {
    sixtyfps_corelib::use_modules();
}

#[cfg(not(no_qt))]
#[rustfmt::skip]
pub type NativeWidgets = (widgets::QtStyleButton, (widgets::QtStyleCheckBox, (widgets::QtStyleSlider, (widgets::QtStyleSpinBox, ()))));

#[cfg(no_qt)]
pub type NativeWidgets = ();
