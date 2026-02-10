// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

#[gen_stub_pyclass]
#[pyclass(name = "KeyboardModifiers")]
#[derive(Clone)]
pub struct PyKeyboardModifiers {
    pub inner: slint_interpreter::KeyboardModifiers,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyKeyboardModifiers {
    #[new]
    #[pyo3(signature = (shift=false, control=false, alt=false, meta=false))]
    fn py_new(shift: bool, control: bool, alt: bool, meta: bool) -> Self {
        let mut inner = slint_interpreter::KeyboardModifiers::default();
        inner.shift = shift;
        inner.control = control;
        inner.alt = alt;
        inner.meta = meta;
        Self { inner }
    }

    #[getter]
    fn shift(&self) -> bool {
        self.inner.shift
    }

    #[setter]
    fn set_shift(&mut self, value: bool) {
        self.inner.shift = value;
    }

    #[getter]
    fn control(&self) -> bool {
        self.inner.control
    }

    #[setter]
    fn set_control(&mut self, value: bool) {
        self.inner.control = value;
    }

    #[getter]
    fn alt(&self) -> bool {
        self.inner.alt
    }

    #[setter]
    fn set_alt(&mut self, value: bool) {
        self.inner.alt = value;
    }

    #[getter]
    fn meta(&self) -> bool {
        self.inner.meta
    }

    #[setter]
    fn set_meta(&mut self, value: bool) {
        self.inner.meta = value;
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl From<slint_interpreter::KeyboardModifiers> for PyKeyboardModifiers {
    fn from(inner: slint_interpreter::KeyboardModifiers) -> Self {
        Self { inner }
    }
}

impl From<PyKeyboardModifiers> for slint_interpreter::KeyboardModifiers {
    fn from(val: PyKeyboardModifiers) -> Self {
        val.inner
    }
}
