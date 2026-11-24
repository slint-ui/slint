// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Python-native value types mirroring `slint::LogicalPosition`, `slint::LogicalSize`,
//! `slint::PhysicalPosition`, and `slint::PhysicalSize`.
//!
//! The Slint interpreter delivers the logical types as generic `Value::Struct` records,
//! a `HashMap` of field name → value.
//! Without these classes, they would reach Python as opaque `PyStruct` wrappers.
//! Wrapping them in real pyo3 classes gives users named types, working `isinstance`, useful
//! `repr()` output, and a natural constructor: `slint.LogicalPosition(x, y)`.

use pyo3::prelude::*;
use std::hash::{Hash, Hasher};

/// Python requires equal objects to hash the same, and `-0.0 == 0.0`.
fn hash_bits(value: f32) -> u32 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

/// A 2D position in logical pixels.
#[pyclass(name = "LogicalPosition", frozen, eq, from_py_object)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PyLogicalPosition {
    /// The horizontal coordinate.
    #[pyo3(get)]
    pub x: f32,
    /// The vertical coordinate.
    #[pyo3(get)]
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
        hash_bits(self.x).hash(&mut h);
        hash_bits(self.y).hash(&mut h);
        h.finish()
    }
}

/// A 2D size in logical pixels.
#[pyclass(name = "LogicalSize", frozen, eq, from_py_object)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PyLogicalSize {
    /// The width.
    #[pyo3(get)]
    pub width: f32,
    /// The height.
    #[pyo3(get)]
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
        hash_bits(self.width).hash(&mut h);
        hash_bits(self.height).hash(&mut h);
        h.finish()
    }
}

/// A 2D position in physical pixels.
#[pyclass(name = "PhysicalPosition", frozen, eq, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PyPhysicalPosition {
    /// The horizontal coordinate.
    #[pyo3(get)]
    pub x: i32,
    /// The vertical coordinate.
    #[pyo3(get)]
    pub y: i32,
}

#[pymethods]
impl PyPhysicalPosition {
    #[new]
    #[pyo3(signature = (x = 0, y = 0))]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn __repr__(&self) -> String {
        format!("PhysicalPosition(x={}, y={})", self.x, self.y)
    }

    fn __hash__(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }
}

/// A 2D size in physical pixels.
#[pyclass(name = "PhysicalSize", frozen, eq, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PyPhysicalSize {
    /// The width.
    #[pyo3(get)]
    pub width: u32,
    /// The height.
    #[pyo3(get)]
    pub height: u32,
}

#[pymethods]
impl PyPhysicalSize {
    #[new]
    #[pyo3(signature = (width = 0, height = 0))]
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    fn __repr__(&self) -> String {
        format!("PhysicalSize(width={}, height={})", self.width, self.height)
    }

    fn __hash__(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }
}
