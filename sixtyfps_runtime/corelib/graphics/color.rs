/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
This module contains color related types for the run-time library.
*/

use crate::properties::InterpolatedPropertyValue;

/// RgbaColor stores the red, green, blue and alpha components of a color
/// with the precision of the generic parameter T. For example if T is f32,
/// the values are normalized between 0 and 1. If T is u8, they values range
/// is 0 to 255.
/// This is merely a helper class for use with [`Color`].
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct RgbaColor<T> {
    /// The alpha component.
    pub alpha: T,
    /// The red channel.
    pub red: T,
    /// The green channel.
    pub green: T,
    /// The blue channel.
    pub blue: T,
}

/// Color represents a color in the SixtyFPS run-time, represented using 8-bit channels for
/// red, green, blue and the alpha (opacity).
/// It can be conveniently constructed and destructured using the to_ and from_ (a)rgb helper functions:
/// ```
/// # fn do_something_with_red_and_green(_:f32, _:f32) {}
/// # fn do_something_with_red(_:u8) {}
/// # use sixtyfps_corelib::graphics::{Color, RgbaColor};
/// # let some_color = Color::from_rgb_u8(0, 0, 0);
/// let col = some_color.to_argb_f32();
/// do_something_with_red_and_green(col.red, col.green);
///
/// let RgbaColor { red, blue, green, .. } = some_color.to_argb_u8();
/// do_something_with_red(red);
///
/// let new_col = Color::from(RgbaColor{ red: 0.5, green: 0.65, blue: 0.32, alpha: 1.});
/// ```
#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl From<RgbaColor<u8>> for Color {
    fn from(col: RgbaColor<u8>) -> Self {
        Self { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
    }
}

impl From<Color> for RgbaColor<u8> {
    fn from(col: Color) -> Self {
        RgbaColor { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
    }
}

impl From<RgbaColor<u8>> for RgbaColor<f32> {
    fn from(col: RgbaColor<u8>) -> Self {
        Self {
            red: (col.red as f32) / 255.0,
            green: (col.green as f32) / 255.0,
            blue: (col.blue as f32) / 255.0,
            alpha: (col.alpha as f32) / 255.0,
        }
    }
}

impl From<Color> for RgbaColor<f32> {
    fn from(col: Color) -> Self {
        let u8col: RgbaColor<u8> = col.into();
        u8col.into()
    }
}

impl From<RgbaColor<f32>> for Color {
    fn from(col: RgbaColor<f32>) -> Self {
        Self {
            red: (col.red * 255.) as u8,
            green: (col.green * 255.) as u8,
            blue: (col.blue * 255.) as u8,
            alpha: (col.alpha * 255.) as u8,
        }
    }
}

impl Color {
    /// Construct a color from an integer encoded as `0xAARRGGBB`
    pub const fn from_argb_encoded(encoded: u32) -> Color {
        Self {
            red: (encoded >> 16) as u8,
            green: (encoded >> 8) as u8,
            blue: encoded as u8,
            alpha: (encoded >> 24) as u8,
        }
    }

    /// Returns `(alpha, red, green, blue)` encoded as u32
    pub fn as_argb_encoded(&self) -> u32 {
        ((self.red as u32) << 16)
            | ((self.green as u32) << 8)
            | (self.blue as u32)
            | ((self.alpha as u32) << 24)
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    pub fn from_argb_u8(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue, alpha }
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub fn from_rgb_u8(red: u8, green: u8, blue: u8) -> Self {
        Self::from_argb_u8(255, red, green, blue)
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    pub fn from_argb_f32(alpha: f32, red: f32, green: f32, blue: f32) -> Self {
        RgbaColor { alpha, red, green, blue }.into()
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub fn from_rgb_f32(red: f32, green: f32, blue: f32) -> Self {
        Self::from_argb_f32(1.0, red, green, blue)
    }

    /// Converts this color to an RgbaColor struct for easy destructuring.
    pub fn to_argb_u8(&self) -> RgbaColor<u8> {
        RgbaColor::from(*self)
    }

    /// Converts this color to an RgbaColor struct for easy destructuring.
    pub fn to_argb_f32(&self) -> RgbaColor<f32> {
        RgbaColor::from(*self)
    }

    /// Returns the red channel of the color as u8 in the range 0..255.
    pub fn red(self) -> u8 {
        self.red
    }

    /// Returns the green channel of the color as u8 in the range 0..255.
    pub fn green(self) -> u8 {
        self.green
    }

    /// Returns the blue channel of the color as u8 in the range 0..255.
    pub fn blue(self) -> u8 {
        self.blue
    }

    /// Returns the alpha channel of the color as u8 in the range 0..255.
    pub fn alpha(self) -> u8 {
        self.alpha
    }
}

impl InterpolatedPropertyValue for Color {
    fn interpolate(self, target_value: Self, t: f32) -> Self {
        Self {
            red: self.red.interpolate(target_value.red, t),
            green: self.green.interpolate(target_value.green, t),
            blue: self.blue.interpolate(target_value.blue, t),
            alpha: self.alpha.interpolate(target_value.alpha, t),
        }
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "argb({}, {}, {}, {})", self.alpha, self.red, self.green, self.blue)
    }
}

#[cfg(feature = "femtovg_backend")]
impl From<&Color> for femtovg::Color {
    fn from(col: &Color) -> Self {
        Self::rgba(col.red, col.green, col.blue, col.alpha)
    }
}

#[cfg(feature = "femtovg_backend")]
impl From<Color> for femtovg::Color {
    fn from(col: Color) -> Self {
        Self::rgba(col.red, col.green, col.blue, col.alpha)
    }
}
