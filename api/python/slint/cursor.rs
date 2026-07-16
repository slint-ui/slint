// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;

/// Python wrapper for Slint's `mouse-cursor` type.
#[pyclass(unsendable, name = "MouseCursorInner", skip_from_py_object)]
#[derive(Clone)]
pub struct PyMouseCursorInner {
    pub mouse_cursor_inner: i_slint_core::cursor::MouseCursorInner,
}

#[pymethods]
impl PyMouseCursorInner {}

impl From<i_slint_core::cursor::MouseCursorInner> for PyMouseCursorInner {
    fn from(mouse_cursor_inner: i_slint_core::cursor::MouseCursorInner) -> Self {
        Self { mouse_cursor_inner }
    }
}
