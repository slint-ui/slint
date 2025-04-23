// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods, impl_stub_type};

use crate::errors::PyColorParseError;

#[gen_stub_pyclass]
#[pyclass]
#[derive(FromPyObject)]
struct RgbaColor {
    #[pyo3(get, set)]
    red: u8,
    #[pyo3(get, set)]
    green: u8,
    #[pyo3(get, set)]
    blue: u8,
    #[pyo3(get, set)]
    alpha: u8,
}

#[gen_stub_pyclass]
#[pyclass]
#[derive(FromPyObject)]
struct RgbColor {
    #[pyo3(get, set)]
    red: u8,
    #[pyo3(get, set)]
    green: u8,
    #[pyo3(get, set)]
    blue: u8,
}

#[derive(FromPyObject)]
#[pyclass]
enum PyColorInput {
    ColorStr(String),
    // This variant must come before RgbColor
    RgbaColor {
        #[pyo3(item)]
        red: u8,
        #[pyo3(item)]
        green: u8,
        #[pyo3(item)]
        blue: u8,
        #[pyo3(item)]
        alpha: u8,
    },
    RgbColor {
        #[pyo3(item)]
        red: u8,
        #[pyo3(item)]
        green: u8,
        #[pyo3(item)]
        blue: u8,
    },
}

impl_stub_type!(PyColorInput = String | RgbaColor | RgbColor);

/// A Color object represents a color in the RGB color space with an alpha. Each color channel and the alpha is represented
/// as an 8-bit integer. The alpha channel is 0 for fully transparent and 255 for fully opaque.
///
/// Construct colors from a CSS color string, or by specifying the red, green, blue, and (optional) alpha channels in a dict.
#[gen_stub_pyclass]
#[pyclass(name = "Color")]
#[derive(Clone)]
pub struct PyColor {
    pub color: slint_interpreter::Color,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyColor {
    #[new]
    #[pyo3(signature = (maybe_value=None))]
    fn py_new(maybe_value: Option<PyColorInput>) -> PyResult<Self> {
        let Some(value) = maybe_value else {
            return Ok(Self { color: Default::default() });
        };

        match value {
            PyColorInput::ColorStr(color_str) => color_str
                .parse::<css_color_parser2::Color>()
                .map(|c| Self {
                    color: slint_interpreter::Color::from_argb_u8(
                        (c.a * 255.) as u8,
                        c.r,
                        c.g,
                        c.b,
                    ),
                })
                .map_err(|color_err| PyColorParseError(color_err).into()),
            PyColorInput::RgbaColor { red, green, blue, alpha } => {
                Ok(Self { color: slint_interpreter::Color::from_argb_u8(alpha, red, green, blue) })
            }
            PyColorInput::RgbColor { red, green, blue } => {
                Ok(Self { color: slint_interpreter::Color::from_rgb_u8(red, green, blue) })
            }
        }
    }

    /// The red channel.
    #[getter]
    fn red(&self) -> u8 {
        self.color.red()
    }

    /// The green channel.
    #[getter]
    fn green(&self) -> u8 {
        self.color.green()
    }

    /// The blue channel.
    #[getter]
    fn blue(&self) -> u8 {
        self.color.blue()
    }

    /// The alpha channel.
    #[getter]
    fn alpha(&self) -> u8 {
        self.color.alpha()
    }

    /// Returns a new color that is brighter than this color by the given factor.
    fn brighter(&self, factor: f32) -> Self {
        Self { color: self.color.brighter(factor) }
    }

    /// Returns a new color that is darker than this color by the given factor.
    fn darker(&self, factor: f32) -> Self {
        Self { color: self.color.darker(factor) }
    }

    /// Returns a new version of this color with the opacity decreased by `factor`.
    ///
    /// The transparency is obtained by multiplying the alpha channel by `(1 - factor)`.
    fn transparentize(&self, factor: f32) -> Self {
        Self { color: self.color.transparentize(factor) }
    }

    /// Returns a new color that is a mix of this color and `other`. The specified factor is
    /// clamped to be between `0.0` and `1.0` and then applied to this color, while `1.0 - factor`
    /// is applied to `other`.
    fn mix(&self, other: &Self, factor: f32) -> Self {
        Self { color: self.color.mix(&other.color, factor) }
    }

    /// Returns a new version of this color with the opacity set to `alpha`.
    fn with_alpha(&self, alpha: f32) -> Self {
        Self { color: self.color.with_alpha(alpha) }
    }

    fn __str__(&self) -> String {
        self.color.to_string()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.color == other.color
    }
}

impl From<slint_interpreter::Color> for PyColor {
    fn from(color: slint_interpreter::Color) -> Self {
        Self { color }
    }
}

#[derive(FromPyObject)]
#[pyclass]
enum PyBrushInput {
    SolidColor(PyColor),
}

impl_stub_type!(PyBrushInput = PyColor);

/// A brush is a data structure that is used to describe how a shape, such as a rectangle, path or even text,
/// shall be filled. A brush can also be applied to the outline of a shape, that means the fill of the outline itself.
///
/// Brushes can only be constructed from solid colors.
///
/// **Note:** In future, we plan to reduce this constraint and allow for declaring graidient brushes programmatically.
#[gen_stub_pyclass]
#[pyclass(name = "Brush")]
pub struct PyBrush {
    pub brush: slint_interpreter::Brush,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyBrush {
    #[new]
    #[pyo3(signature = (maybe_value=None))]
    fn py_new(maybe_value: Option<PyBrushInput>) -> PyResult<Self> {
        let Some(value) = maybe_value else {
            return Ok(Self { brush: Default::default() });
        };

        match value {
            PyBrushInput::SolidColor(pycol) => Ok(Self { brush: pycol.color.into() }),
        }
    }

    /// The brush's color.
    #[getter]
    fn color(&self) -> PyColor {
        self.brush.color().into()
    }

    /// Returns true if this brush contains a fully transparent color (alpha value is zero).
    fn is_transparent(&self) -> bool {
        self.brush.is_transparent()
    }

    /// Returns true if this brush is fully opaque.
    fn is_opaque(&self) -> bool {
        self.brush.is_opaque()
    }

    /// Returns a new version of this brush that has the brightness increased
    /// by the specified factor. This is done by calling `Color.brighter` on
    /// all the colors of this brush.
    fn brighter(&self, factor: f32) -> Self {
        Self { brush: self.brush.brighter(factor) }
    }

    /// Returns a new version of this brush that has the brightness decreased
    /// by the specified factor. This is done by calling `Color.darker` on
    /// all the color of this brush.
    fn darker(&self, factor: f32) -> Self {
        Self { brush: self.brush.darker(factor) }
    }

    /// Returns a new version of this brush with the opacity decreased by `factor`.
    ///
    /// The transparency is obtained by multiplying the alpha channel by `(1 - factor)`.
    ///
    /// See also `Color.transparentize`.
    fn transparentize(&self, amount: f32) -> Self {
        Self { brush: self.brush.transparentize(amount) }
    }

    /// Returns a new version of this brush with the related color's opacities
    /// set to `alpha`.
    fn with_alpha(&self, alpha: f32) -> Self {
        Self { brush: self.brush.with_alpha(alpha) }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.brush == other.brush
    }
}

impl From<slint_interpreter::Brush> for PyBrush {
    fn from(brush: slint_interpreter::Brush) -> Self {
        Self { brush }
    }
}
