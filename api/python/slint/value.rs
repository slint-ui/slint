// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

use std::collections::HashMap;

#[gen_stub_pyclass]
pub struct PyValue(pub slint_interpreter::Value);

impl<'py> IntoPyObject<'py> for PyValue {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match &self.0 {
            slint_interpreter::Value::Void => ().into_bound_py_any(py),
            slint_interpreter::Value::Number(num) => num.into_bound_py_any(py),
            slint_interpreter::Value::String(str) => str.into_bound_py_any(py),
            slint_interpreter::Value::Bool(b) => b.into_bound_py_any(py),
            slint_interpreter::Value::Image(image) => {
                crate::image::PyImage::from(image).into_bound_py_any(py)
            }
            slint_interpreter::Value::Model(model) => {
                crate::models::PyModelShared::rust_into_py_model(model, py).map_or_else(
                    || crate::models::ReadOnlyRustModel::from(model).into_bound_py_any(py),
                    |m| Ok(m),
                )
            }
            slint_interpreter::Value::Struct(structval) => {
                PyStruct { data: structval.clone() }.into_bound_py_any(py)
            }
            slint_interpreter::Value::Brush(brush) => {
                crate::brush::PyBrush::from(brush.clone()).into_bound_py_any(py)
            }
            v @ _ => {
                eprintln!("Python: conversion from slint to python needed for {v:#?} and not implemented yet");
                ().into_bound_py_any(py)
            }
        }
    }
}

impl<'py> FromPyObject<'py> for PyValue {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        if ob.is_none() {
            return Ok(slint_interpreter::Value::Void.into());
        }

        let interpreter_val = ob
            .extract::<bool>()
            .map(|b| slint_interpreter::Value::Bool(b))
            .or_else(|_| {
                ob.extract::<&'_ str>().map(|s| slint_interpreter::Value::String(s.into()))
            })
            .or_else(|_| ob.extract::<f64>().map(|num| slint_interpreter::Value::Number(num)))
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::image::PyImage>>()
                    .map(|pyimg| slint_interpreter::Value::Image(pyimg.image.clone()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::brush::PyBrush>>()
                    .map(|pybrush| slint_interpreter::Value::Brush(pybrush.brush.clone()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::brush::PyColor>>()
                    .map(|pycolor| slint_interpreter::Value::Brush(pycolor.color.clone().into()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::models::PyModelBase>>()
                    .map(|pymodel| slint_interpreter::Value::Model(pymodel.as_model()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::models::ReadOnlyRustModel>>()
                    .map(|rustmodel| slint_interpreter::Value::Model(rustmodel.0.clone()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, PyStruct>>().and_then(|pystruct| {
                    Ok(slint_interpreter::Value::Struct(pystruct.data.clone()))
                })
            })
            .or_else(|_| {
                let dict = ob.downcast::<PyDict>()?;
                let dict_items: Result<Vec<(String, slint_interpreter::Value)>, PyErr> = dict
                    .iter()
                    .map(|(name, pyval)| {
                        let name = name.extract::<&str>()?.to_string();
                        let slintval = PyValue::extract_bound(&pyval)?;
                        Ok((name, slintval.0))
                    })
                    .collect::<Result<Vec<(_, _)>, PyErr>>();
                Ok::<_, PyErr>(slint_interpreter::Value::Struct(
                    slint_interpreter::Struct::from_iter(dict_items?.into_iter()),
                ))
            })?;

        Ok(PyValue(interpreter_val))
    }
}
impl From<slint_interpreter::Value> for PyValue {
    fn from(value: slint_interpreter::Value) -> Self {
        Self(value)
    }
}

#[gen_stub_pyclass]
#[pyclass(subclass, unsendable)]
#[derive(Clone, Default)]
pub struct PyStruct {
    data: slint_interpreter::Struct,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyStruct {
    #[new]
    fn new() -> Self {
        Default::default()
    }

    fn __getattr__(&self, key: &str) -> PyResult<PyValue> {
        self.data.get_field(key).map_or_else(
            || {
                Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                    "Python: No such field {key} on PyStruct"
                )))
            },
            |value| Ok(value.clone().into()),
        )
    }
    fn __setattr__(&mut self, py: Python<'_>, key: String, value: PyObject) -> PyResult<()> {
        let pv: PyValue = value.extract(py)?;
        self.data.set_field(key, pv.0);
        Ok(())
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyStructFieldIterator {
        PyStructFieldIterator {
            inner: slf
                .data
                .iter()
                .map(|(name, val)| (name.to_string(), val.clone()))
                .collect::<HashMap<_, _>>()
                .into_iter(),
        }
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

impl From<slint_interpreter::Struct> for PyStruct {
    fn from(data: slint_interpreter::Struct) -> Self {
        Self { data }
    }
}

#[gen_stub_pyclass]
#[pyclass(unsendable)]
struct PyStructFieldIterator {
    inner: std::collections::hash_map::IntoIter<String, slint_interpreter::Value>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyStructFieldIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<(String, PyValue)> {
        slf.inner.next().map(|(name, val)| (name, PyValue(val)))
    }
}
