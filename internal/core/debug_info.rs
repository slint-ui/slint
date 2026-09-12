// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! String encoding of declared-property values
//! for the debug-info introspection channel (`ItemTreeVTable::element_property_value`).
//!
//! Compiler-generated code and the interpreter share these helpers,
//! so both encode the same value to the same string:
//! booleans as `true`/`false`,
//! integers and durations (milliseconds) in decimal,
//! floats, lengths (logical pixels), angles (degrees) and percentages through `f32` `Display`,
//! colors and solid brushes as `#rrggbbaa`,
//! and enums as the `.slint` source spelling of the value.
//! Other types have no encoding.

use crate::string::SharedString;

/// Formats a boolean property value as `true` or `false`.
pub fn format_bool(value: bool) -> SharedString {
    if value { "true".into() } else { "false".into() }
}

/// Formats an integer-typed property value (`int`, or `duration` in milliseconds).
pub fn format_integer(value: i64) -> SharedString {
    crate::string::format(format_args!("{value}"))
}

/// Formats a float-typed property value (`float`, `length` in logical pixels,
/// `angle` in degrees, or `percent`).
pub fn format_float(value: f32) -> SharedString {
    crate::string::format(format_args!("{value}"))
}

/// Formats a color as `#rrggbbaa`.
pub fn format_color(color: crate::Color) -> SharedString {
    crate::string::format(format_args!(
        "#{:02x}{:02x}{:02x}{:02x}",
        color.red(),
        color.green(),
        color.blue(),
        color.alpha()
    ))
}

/// Formats a brush: a solid color as `#rrggbbaa`; gradients have no encoding.
pub fn format_brush(brush: &crate::Brush) -> Option<SharedString> {
    match brush {
        crate::Brush::SolidColor(color) => Some(format_color(*color)),
        _ => None,
    }
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    /// Generated C++ delegates float encoding here rather than formatting locally:
    /// no C++ float-to-string routine on the supported toolchains reproduces Rust's
    /// `f32` `Display` digits (GCC 10 has no floating-point `std::to_chars`).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_debug_info_format_float(out: *mut SharedString, value: f32) {
        unsafe { core::ptr::write(out, format_float(value)) }
    }
}
