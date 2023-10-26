// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::{graphics::GradientStop, Brush, Color};
use napi::{bindgen_prelude::External, Error, Result};

/// Color represents a color in the Slint run-time, represented using 8-bit channels for red, green, blue and the alpha (opacity).
#[napi(object, js_name = "Color")]
pub struct JsColor {
    /// Represents the red channel of the color as u8 in the range 0..255.
    pub red: f64,

    /// Represents the green channel of the color as u8 in the range 0..255.
    pub green: f64,

    /// Represents the blue channel of the color as u8 in the range 0..255.
    pub blue: f64,

    /// Represents the alpha channel of the color as u8 in the range 0..255.
    pub alpha: f64,
}

/// Color represents a color in the Slint run-time, represented using 8-bit channels for red, green, blue and the alpha (opacity).
#[napi(js_name = SlintColor)]
pub struct JsSlintColor {
    inner: Color,
}

impl From<Color> for JsSlintColor {
    fn from(color: Color) -> Self {
        Self { inner: color }
    }
}

#[napi]
impl JsSlintColor {
    /// Creates a new transparent color.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { inner: Color::default() }
    }

    /// Construct a color from an integer encoded as `0xAARRGGBB`
    #[napi(factory)]
    pub fn from_argb_encoded(encoded: u32) -> Self {
        Self { inner: Color::from_argb_encoded(encoded) }
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    #[napi(factory)]
    pub fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { inner: Color::from_rgb_u8(red, green, blue) }
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    #[napi(factory)]
    pub fn from_argb(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        Self { inner: Color::from_argb_u8(alpha, red, green, blue) }
    }

    /// Returns `(alpha, red, green, blue)` encoded as number.
    #[napi(getter)]
    pub fn as_argb_encoded(&self) -> u32 {
        self.inner.as_argb_encoded()
    }

    /// Returns the red channel of the color as number in the range 0..255.
    #[napi(getter)]
    pub fn red(&self) -> u8 {
        self.inner.red()
    }

    /// Returns the green channel of the color as number in the range 0..255.
    #[napi(getter)]
    pub fn green(&self) -> u8 {
        self.inner.green()
    }

    /// Returns the blue channel of the color as number in the range 0..255.
    #[napi(getter)]
    pub fn blue(&self) -> u8 {
        self.inner.blue()
    }

    /// Returns the alpha channel of the color as number in the range 0..255.
    #[napi(getter)]
    pub fn alpha(&self) -> u8 {
        self.inner.alpha()
    }

    // Returns a new version of this color that has the brightness increased
    /// by the specified factor. This is done by converting the color to the HSV
    /// color space and multiplying the brightness (value) with (1 + factor).
    /// The result is converted back to RGB and the alpha channel is unchanged.
    /// So for example `brighter(0.2)` will increase the brightness by 20%, and
    /// calling `brighter(-0.5)` will return a color that's 50% darker.
    #[napi]
    pub fn brighter(&self, factor: f64) -> JsSlintColor {
        JsSlintColor::from(self.inner.brighter(factor as f32))
    }

    /// Returns a new version of this color that has the brightness decreased
    /// by the specified factor. This is done by converting the color to the HSV
    /// color space and dividing the brightness (value) by (1 + factor). The
    /// result is converted back to RGB and the alpha channel is unchanged.
    /// So for example `darker(0.3)` will decrease the brightness by 30%.
    #[napi]
    pub fn darker(&self, factor: f64) -> JsSlintColor {
        JsSlintColor::from(self.inner.darker(factor as f32))
    }

    /// Returns a new version of this color with the opacity decreased by `factor`.
    ///
    /// The transparency is obtained by multiplying the alpha channel by `(1 - factor)`.
    #[napi]
    pub fn transparentize(&self, amount: f64) -> JsSlintColor {
        JsSlintColor::from(self.inner.transparentize(amount as f32))
    }

    /// Returns a new color that is a mix of `self` and `other`, with a proportion
    /// factor given by `factor` (which will be clamped to be between `0.0` and `1.0`).
    #[napi]
    pub fn mix(&self, other: &JsSlintColor, factor: f64) -> JsSlintColor {
        JsSlintColor::from(self.inner.mix(&other.inner, factor as f32))
    }

    /// Returns a new version of this color with the opacity set to `alpha`.
    #[napi]
    pub fn with_alpha(&self, alpha: f64) -> JsSlintColor {
        JsSlintColor::from(self.inner.with_alpha(alpha as f32))
    }

    /// Returns the color as string in hex representation e.g. `#000000` for black.
    #[napi]
    pub fn to_string(&self) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", self.red(), self.green(), self.blue(), self.alpha())
    }
}

/// A brush is a data structure that is used to describe how
/// a shape, such as a rectangle, path or even text, shall be filled.
/// A brush can also be applied to the outline of a shape, that means
/// the fill of the outline itself.
#[napi(js_name = Brush)]
pub struct JsBrush {
    inner: Brush,
}

impl From<Brush> for JsBrush {
    fn from(brush: Brush) -> Self {
        Self { inner: brush }
    }
}

impl From<JsSlintColor> for JsBrush {
    fn from(color: JsSlintColor) -> Self {
        Self::from(Brush::from(color.inner))
    }
}

#[napi]
impl JsBrush {
    #[napi(constructor)]
    pub fn new_with_color(color: JsColor) -> Result<Self> {
        if color.red < 0. || color.green < 0. || color.blue < 0. || color.alpha < 0. {
            return Err(Error::from_reason("A channel of Color cannot be negative"));
        }

        Ok(Self {
            inner: Brush::SolidColor(Color::from_argb_u8(
                color.alpha.floor() as u8,
                color.red.floor() as u8,
                color.green.floor() as u8,
                color.blue.floor() as u8,
            )),
        })
    }

    /// Creates a brush form a `Color`.
    pub fn from_slint_color(color: &JsSlintColor) -> Self {
        Self { inner: Brush::SolidColor(color.inner) }
    }

    /// @hidden
    #[napi(getter)]
    pub fn color(&self) -> JsSlintColor {
        self.inner.color().into()
    }

    /// Returns true if this brush contains a fully transparent color (alpha value is zero)
    #[napi(getter)]
    pub fn is_transparent(&self) -> bool {
        self.inner.is_transparent()
    }

    /// Returns true if this brush is fully opaque.
    #[napi(getter)]
    pub fn is_opaque(&self) -> bool {
        self.inner.is_opaque()
    }

    /// Returns a new version of this brush that has the brightness increased
    /// by the specified factor. This is done by calling [`Color::brighter`] on
    /// all the colors of this brush.
    #[napi]
    pub fn brighter(&self, factor: f64) -> JsBrush {
        JsBrush::from(self.inner.brighter(factor as f32))
    }

    /// Returns a new version of this brush that has the brightness decreased
    /// by the specified factor. This is done by calling [`Color::darker`] on
    /// all the color of this brush.
    #[napi]
    pub fn darker(&self, factor: f64) -> JsBrush {
        JsBrush::from(self.inner.darker(factor as f32))
    }

    /// Returns a new version of this brush with the opacity decreased by `factor`.
    ///
    /// The transparency is obtained by multiplying the alpha channel by `(1 - factor)`.
    #[napi]
    pub fn transparentize(&self, amount: f64) -> JsBrush {
        JsBrush::from(self.inner.transparentize(amount as f32))
    }

    /// Returns a new version of this brush with the related color's opacities
    /// set to `alpha`.
    #[napi]
    pub fn with_alpha(&self, alpha: f64) -> JsBrush {
        JsBrush::from(self.inner.with_alpha(alpha as f32))
    }

    /// @hidden
    #[napi(getter)]
    pub fn brush(&self) -> External<Brush> {
        External::new(self.inner.clone())
    }

    /// Returns the color as string in hex representation e.g. `#000000` for black.
    /// It is only implemented for solid color brushes.
    #[napi]
    pub fn to_string(&self) -> String {
        match &self.inner {
            Brush::SolidColor(_) => {
                return self.color().to_string();
            }
            Brush::LinearGradient(gradient) => {
                return format!(
                    "linear-gradient({}deg, {})",
                    gradient.angle(),
                    gradient_stops_to_string(gradient.stops())
                );
            }
            Brush::RadialGradient(gradient) => {
                return format!(
                    "radial-gradient(circle, {})",
                    gradient_stops_to_string(gradient.stops())
                );
            }
            _ => String::default(),
        }
    }
}

fn gradient_stops_to_string<'a>(stops: impl Iterator<Item = &'a GradientStop>) -> String {
    let stops: Vec<String> = stops
        .map(|s| {
            format!(
                "rgba({}, {}, {}, {}) {}%",
                s.color.red(),
                s.color.green(),
                s.color.blue(),
                s.color.alpha(),
                s.position * 100.
            )
        })
        .collect();

    let mut stops_string = String::default();
    let len = stops.len();

    for i in 0..len {
        stops_string.push_str(stops[i].as_str());

        if i < len - 1 {
            stops_string.push_str(", ");
        }
    }

    stops_string
}
