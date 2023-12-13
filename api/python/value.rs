// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use pyo3::prelude::*;

pub struct PyValue(pub slint_interpreter::Value);

impl IntoPy<PyObject> for PyValue {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self.0 {
            slint_interpreter::Value::Void => ().into_py(py),
            slint_interpreter::Value::Number(num) => num.into_py(py),
            slint_interpreter::Value::String(str) => str.into_py(py),
            slint_interpreter::Value::Bool(b) => b.into_py(py),
            slint_interpreter::Value::Image(_) => todo!(),
            slint_interpreter::Value::Model(_) => todo!(),
            slint_interpreter::Value::Struct(_) => todo!(),
            slint_interpreter::Value::Brush(_) => todo!(),
            _ => todo!(),
        }
    }
}

impl ToPyObject for PyValue {
    fn to_object(&self, py: Python<'_>) -> PyObject {
        match &self.0 {
            slint_interpreter::Value::Void => ().into_py(py),
            slint_interpreter::Value::Number(num) => num.into_py(py),
            slint_interpreter::Value::String(str) => str.into_py(py),
            slint_interpreter::Value::Bool(b) => b.into_py(py),
            slint_interpreter::Value::Image(_) => todo!(),
            slint_interpreter::Value::Model(_) => todo!(),
            slint_interpreter::Value::Struct(_) => todo!(),
            slint_interpreter::Value::Brush(_) => todo!(),
            _ => todo!(),
        }
    }
}

impl FromPyObject<'_> for PyValue {
    fn extract(ob: &PyAny) -> PyResult<Self> {
        if ob.is_none() {
            return Ok(slint_interpreter::Value::Void.into());
        }

        Ok(PyValue(
            ob.extract::<bool>()
                .map(|b| slint_interpreter::Value::Bool(b))
                .or_else(|_| {
                    ob.extract::<&'_ str>().map(|s| slint_interpreter::Value::String(s.into()))
                })
                .or_else(|_| {
                    ob.extract::<f64>().map(|num| slint_interpreter::Value::Number(num))
                })?,
        ))
    }
}
impl From<slint_interpreter::Value> for PyValue {
    fn from(value: slint_interpreter::Value) -> Self {
        Self(value)
    }
}
