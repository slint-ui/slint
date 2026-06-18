// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;

/// Python wrapper for Slint's `styled-text` type.
#[pyclass(name = "StyledText", skip_from_py_object)]
#[derive(Clone)]
pub struct PyStyledText {
    pub styled_text: i_slint_core::styled_text::StyledText,
}

#[pymethods]
impl PyStyledText {
    /// Creates empty styled text.
    #[new]
    fn py_new() -> Self {
        Self { styled_text: Default::default() }
    }

    /// Creates styled text from plain text.
    #[staticmethod]
    fn from_plain_text(text: &str) -> Self {
        Self { styled_text: i_slint_core::styled_text::StyledText::from_plain_text(text) }
    }

    /// Parses markdown and returns a StyledText object.
    ///
    /// Raises a ValueError if the markdown contains unsupported syntax.
    #[staticmethod]
    fn from_markdown(markdown: &str) -> PyResult<Self> {
        i_slint_core::styled_text::StyledText::from_markdown(markdown)
            .map(|styled_text| Self { styled_text })
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
    }

    fn __repr__(&self) -> String {
        format!("StyledText({:?})", self.styled_text)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.styled_text == other.styled_text
    }
}

impl From<i_slint_core::styled_text::StyledText> for PyStyledText {
    fn from(styled_text: i_slint_core::styled_text::StyledText) -> Self {
        Self { styled_text }
    }
}
