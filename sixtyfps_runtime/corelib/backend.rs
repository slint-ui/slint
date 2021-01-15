/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
The backend is the abstraction for crates that need to do the actual drawing and event loop
*/

use crate::window::ComponentWindow;

/// Interface implemented by backends
pub trait Backend: Send + Sync {
    /// Instentiate a window for a component.
    /// FIXME: should return a Box<dyn PlatformWindow>
    fn create_window(&'static self) -> ComponentWindow;

    /// This function can be used to register a custom TrueType font with SixtyFPS,
    /// for use with the `font-family` property. The provided slice must be a valid TrueType
    /// font.
    fn register_application_font_from_memory(
        &'static self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>>;
}
