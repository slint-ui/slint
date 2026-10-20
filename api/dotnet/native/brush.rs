// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Colors and brushes, through Slint's own implementations so that they give the same results
//! as in the other languages. Colors cross the boundary as `0xAARRGGBB`, brushes as values.

use slint_interpreter::{Brush, Color, Value};

use crate::{guard, into_raw};

fn color(argb: u32) -> Color {
    Color::from_argb_encoded(argb)
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_color_brighter(argb: u32, factor: f32) -> u32 {
    color(argb).brighter(factor).as_argb_encoded()
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_color_darker(argb: u32, factor: f32) -> u32 {
    color(argb).darker(factor).as_argb_encoded()
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_color_transparentize(argb: u32, factor: f32) -> u32 {
    color(argb).transparentize(factor).as_argb_encoded()
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_color_mix(argb: u32, other: u32, factor: f32) -> u32 {
    color(argb).mix(&color(other), factor).as_argb_encoded()
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_color_with_alpha(argb: u32, alpha: f32) -> u32 {
    color(argb).with_alpha(alpha).as_argb_encoded()
}

fn brush(value: &Value) -> Result<&Brush, String> {
    match value {
        Value::Brush(brush) => Ok(brush),
        _ => Err("value isn't a brush".into()),
    }
}

/// The color of a solid brush, or the first stop of a gradient.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_brush_color(value: &Value) -> u32 {
    guard(0, || Ok(brush(value)?.color().as_argb_encoded()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_brush_is_transparent(value: &Value) -> bool {
    guard(false, || Ok(brush(value)?.is_transparent()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_brush_is_opaque(value: &Value) -> bool {
    guard(false, || Ok(brush(value)?.is_opaque()))
}

/// `operation` selects 0 brighter, 1 darker, 2 transparentize, or 3 with_alpha.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_brush_derive(
    value: &Value,
    operation: i32,
    factor: f32,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let brush = brush(value)?;
        let derived = match operation {
            0 => brush.brighter(factor),
            1 => brush.darker(factor),
            2 => brush.transparentize(factor),
            3 => brush.with_alpha(factor),
            _ => return Err(format!("unknown brush operation {operation}")),
        };
        Ok(into_raw(Value::Brush(derived)))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_value_equals(a: &Value, b: &Value) -> bool {
    a == b
}
