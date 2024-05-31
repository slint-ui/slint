// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;

use crate::errors::PyColorParseError;

#[derive(FromPyObject)]
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

#[pyclass]
#[derive(Clone)]
pub struct PyColor {
    pub color: slint_interpreter::Color,
}

#[pymethods]
impl PyColor {
    #[new]
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

    #[getter]
    fn red(&self) -> u8 {
        self.color.red()
    }

    #[getter]
    fn green(&self) -> u8 {
        self.color.green()
    }

    #[getter]
    fn blue(&self) -> u8 {
        self.color.blue()
    }

    #[getter]
    fn alpha(&self) -> u8 {
        self.color.alpha()
    }

    fn brighter(&self, factor: f32) -> Self {
        Self { color: self.color.brighter(factor) }
    }

    fn darker(&self, factor: f32) -> Self {
        Self { color: self.color.darker(factor) }
    }

    fn transparentize(&self, factor: f32) -> Self {
        Self { color: self.color.transparentize(factor) }
    }

    fn mix(&self, other: &Self, factor: f32) -> Self {
        Self { color: self.color.mix(&other.color, factor) }
    }

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
enum PyBrushInput {
    SolidColor(PyColor),
}

#[pyclass]
pub struct PyBrush {
    pub brush: slint_interpreter::Brush,
}

#[pymethods]
impl PyBrush {
    #[new]
    fn py_new(maybe_value: Option<PyBrushInput>) -> PyResult<Self> {
        let Some(value) = maybe_value else {
            return Ok(Self { brush: Default::default() });
        };

        match value {
            PyBrushInput::SolidColor(pycol) => Ok(Self { brush: pycol.color.into() }),
        }
    }

    #[getter]
    fn color(&self) -> PyColor {
        self.brush.color().into()
    }

    fn is_transparent(&self) -> bool {
        self.brush.is_transparent()
    }

    fn is_opaque(&self) -> bool {
        self.brush.is_opaque()
    }

    fn brighter(&self, factor: f32) -> Self {
        Self { brush: self.brush.brighter(factor) }
    }

    fn darker(&self, factor: f32) -> Self {
        Self { brush: self.brush.darker(factor) }
    }

    fn transparentize(&self, amount: f32) -> Self {
        Self { brush: self.brush.transparentize(amount) }
    }

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
