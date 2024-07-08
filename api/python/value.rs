// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3::types::PyDict;

use std::collections::HashMap;

pub struct PyValue(pub slint_interpreter::Value);
struct PyValueRef<'a>(&'a slint_interpreter::Value);

impl IntoPy<PyObject> for PyValue {
    fn into_py(self, py: Python<'_>) -> PyObject {
        // Share the conversion code below that operates on the reference
        self.to_object(py).into_py(py)
    }
}

impl ToPyObject for PyValue {
    fn to_object(&self, py: Python<'_>) -> PyObject {
        PyValueRef(&self.0).to_object(py)
    }
}

impl<'a> IntoPy<PyObject> for PyValueRef<'a> {
    fn into_py(self, py: Python<'_>) -> PyObject {
        // Share the conversion code below that operates on the reference
        self.to_object(py).into_py(py)
    }
}

impl<'a> ToPyObject for PyValueRef<'a> {
    fn to_object(&self, py: Python<'_>) -> PyObject {
        match &self.0 {
            slint_interpreter::Value::Void => ().into_py(py),
            slint_interpreter::Value::Number(num) => num.into_py(py),
            slint_interpreter::Value::String(str) => str.into_py(py),
            slint_interpreter::Value::Bool(b) => b.into_py(py),
            slint_interpreter::Value::Image(image) => {
                crate::image::PyImage::from(image).into_py(py)
            }
            slint_interpreter::Value::Model(model) => {
                crate::models::PyModelShared::rust_into_js_model(model)
                    .unwrap_or_else(|| crate::models::ReadOnlyRustModel::from(model).into_py(py))
            }
            slint_interpreter::Value::Struct(structval) => {
                PyStruct { data: structval.clone() }.into_py(py)
            }
            slint_interpreter::Value::Brush(brush) => {
                crate::brush::PyBrush::from(brush.clone()).into_py(py)
            }
            v @ _ => {
                eprintln!("Python: conversion from slint to python needed for {:#?} and not implemented yet", v);
                ().into_py(py)
            }
        }
    }
}

impl FromPyObject<'_> for PyValue {
    fn extract(ob: &PyAny) -> PyResult<Self> {
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
                ob.extract::<&PyDict>().and_then(|dict| {
                    let dict_items: Result<Vec<(String, slint_interpreter::Value)>, PyErr> = dict
                        .iter()
                        .map(|(name, pyval)| {
                            let name = name.extract::<&str>()?.to_string();
                            let slintval = PyValue::extract(pyval)?;
                            Ok((name, slintval.0))
                        })
                        .collect::<Result<Vec<(_, _)>, PyErr>>();
                    Ok(slint_interpreter::Value::Struct(slint_interpreter::Struct::from_iter(
                        dict_items?.into_iter(),
                    )))
                })
            })?;

        Ok(PyValue(interpreter_val))
    }
}
impl From<slint_interpreter::Value> for PyValue {
    fn from(value: slint_interpreter::Value) -> Self {
        Self(value)
    }
}

#[pyclass(subclass, unsendable)]
#[derive(Clone, Default)]
pub struct PyStruct {
    data: slint_interpreter::Struct,
}

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

#[pyclass(unsendable)]
struct PyStructFieldIterator {
    inner: std::collections::hash_map::IntoIter<String, slint_interpreter::Value>,
}

#[pymethods]
impl PyStructFieldIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<(String, PyValue)> {
        slf.inner.next().map(|(name, val)| (name, PyValue(val)))
    }
}
