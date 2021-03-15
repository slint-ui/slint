/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
# SixtyFPS interpreter library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead
*/
#![warn(missing_docs)]
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]

mod api;
mod dynamic_component;
mod dynamic_type;
mod eval;
mod global_component;
mod value_model;

#[doc(inline)]
pub use api::*;

/// This function can be used to register a custom TrueType font with SixtyFPS,
/// for use with the `font-family` property. The provided path must refer to a valid TrueType
/// font.
pub fn register_font_from_path<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_path(path.as_ref())
}

/// This function can be used to register a custom TrueType font with SixtyFPS,
/// for use with the `font-family` property. The provided slice must be a valid TrueType
/// font.
pub fn register_font_from_memory(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_memory(data)
}
