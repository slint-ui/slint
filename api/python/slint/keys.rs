// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

/// Represents a key binding created by the `@keys(...)` macro in Slint.
///
/// This is an opaque type. Use `str()` to get a platform-native representation
/// of the key binding (e.g. "Ctrl+A" on Linux/Windows, "⌘A" on macOS).
#[gen_stub_pyclass]
#[pyclass(name = "Keys", skip_from_py_object)]
#[derive(Clone)]
pub struct PyKeys {
    pub keys: i_slint_core::input::Keys,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyKeys {
    /// Create a `Keys` from a list of string parts, e.g. `["Control", "Shift?", "Z"]`.
    ///
    /// Each element is either a modifier name or a key name. Raises ValueError on parse failure.
    #[staticmethod]
    fn from_parts(parts: Vec<String>) -> PyResult<Self> {
        i_slint_core::input::Keys::from_parts(parts.iter().map(|s| s.as_str()))
            .map(|k| Self { keys: k })
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    fn __str__(&self) -> String {
        self.keys.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Keys({:?})", self.keys)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.keys == other.keys
    }
}

impl From<i_slint_core::input::Keys> for PyKeys {
    fn from(keys: i_slint_core::input::Keys) -> Self {
        Self { keys }
    }
}
