// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Python-native value types mirroring `slint::LogicalPosition` and `slint::LogicalSize`.
//!
//! The Slint interpreter delivers these as generic `Value::Struct` records (a `HashMap` of
//! field name → value) which would otherwise reach Python as opaque `PyStruct` wrappers.
//! Wrapping them in real pyo3 classes gives users named types, working `isinstance`, useful
//! `repr()` output, and a natural constructor: `slint.LogicalPosition(x, y)`.

use pyo3::prelude::*;
use std::hash::{Hash, Hasher};

/// A 2D position in logical pixels.
#[pyclass(name = "LogicalPosition", eq, from_py_object)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PyLogicalPosition {
    /// The horizontal coordinate.
    #[pyo3(get, set)]
    pub x: f32,
    /// The vertical coordinate.
    #[pyo3(get, set)]
    pub y: f32,
}

#[pymethods]
impl PyLogicalPosition {
    #[new]
    #[pyo3(signature = (x = 0.0, y = 0.0))]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn __repr__(&self) -> String {
        format!("LogicalPosition(x={}, y={})", self.x, self.y)
    }

    fn __hash__(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.x.to_bits().hash(&mut h);
        self.y.to_bits().hash(&mut h);
        h.finish()
    }
}

/// A 2D size in logical pixels.
#[pyclass(name = "LogicalSize", eq, from_py_object)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PyLogicalSize {
    /// The width.
    #[pyo3(get, set)]
    pub width: f32,
    /// The height.
    #[pyo3(get, set)]
    pub height: f32,
}

#[pymethods]
impl PyLogicalSize {
    #[new]
    #[pyo3(signature = (width = 0.0, height = 0.0))]
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    fn __repr__(&self) -> String {
        format!("LogicalSize(width={}, height={})", self.width, self.height)
    }

    fn __hash__(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.width.to_bits().hash(&mut h);
        self.height.to_bits().hash(&mut h);
        h.finish()
    }
}
