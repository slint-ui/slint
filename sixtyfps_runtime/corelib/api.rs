// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!
This module contains types that are public and re-exported in the sixtyfps-rs as well as the sixtyfps-interpreter crate as public API.
*/

use std::rc::Rc;

use crate::window::WindowRc;

/// This type represents a window towards the windowing system, that's used to render the
/// scene of a component. It provides API to control windowing system specific aspects such
/// as the position on the screen.
#[repr(transparent)]
pub struct Window(WindowRc);

#[doc(hidden)]
impl From<WindowRc> for Window {
    fn from(window: WindowRc) -> Self {
        Self(window)
    }
}

impl Window {
    /// Registers the window with the windowing system in order to make it visible on the screen.
    pub fn show(&self) {
        self.0.show();
    }

    /// De-registers the window from the windowing system, therefore hiding it.
    pub fn hide(&self) {
        self.0.hide();
    }
}

impl crate::window::WindowHandleAccess for Window {
    fn window_handle(&self) -> &Rc<crate::window::Window> {
        &self.0
    }
}
