// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;

use i_slint_compiler::langtype::Type;

use i_slint_core::model::{Model, ModelRc};

#[gen_stub_pyclass]
pub struct SlintToPyValue {
    pub slint_value: slint_interpreter::Value,
    pub type_collection: TypeCollection,
}

impl<'py> IntoPyObject<'py> for SlintToPyValue {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let type_collection = self.type_collection;
        match self.slint_value {
            slint_interpreter::Value::Void => ().into_bound_py_any(py),
            slint_interpreter::Value::Number(num) => num.into_bound_py_any(py),
            slint_interpreter::Value::String(str) => str.into_bound_py_any(py),
            slint_interpreter::Value::Bool(b) => b.into_bound_py_any(py),
            slint_interpreter::Value::Image(image) => {
                crate::image::PyImage::from(image).into_bound_py_any(py)
            }
            slint_interpreter::Value::Model(model) => {
                crate::models::PyModelShared::rust_into_py_model(&model, py).map_or_else(
                    || type_collection.model_to_py(&model).into_bound_py_any(py),
                    |m| Ok(m),
                )
            }
            slint_interpreter::Value::Struct(structval) => {
                type_collection.struct_to_py(structval).into_bound_py_any(py)
            }
            slint_interpreter::Value::Brush(brush) => {
                crate::brush::PyBrush::from(brush).into_bound_py_any(py)
            }
            slint_interpreter::Value::EnumerationValue(enum_name, enum_value) => {
                type_collection.enum_to_py(&enum_name, &enum_value, py)?.into_bound_py_any(py)
            }
            v @ _ => {
                eprintln!("Python: conversion from slint to python needed for {v:#?} and not implemented yet");
                ().into_bound_py_any(py)
            }
        }
    }
}

#[gen_stub_pyclass]
#[pyclass(subclass, unsendable)]
#[derive(Clone)]
pub struct PyStruct {
    pub data: slint_interpreter::Struct,
    pub type_collection: TypeCollection,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyStruct {
    fn __getattr__(&self, key: &str) -> PyResult<SlintToPyValue> {
        self.data.get_field(key).map_or_else(
            || {
                Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                    "Python: No such field {key} on PyStruct"
                )))
            },
            |value| Ok(self.type_collection.to_py_value(value.clone())),
        )
    }
    fn __setattr__(&mut self, py: Python<'_>, key: String, value: PyObject) -> PyResult<()> {
        let pv =
            TypeCollection::slint_value_from_py_value(py, &value, Some(&self.type_collection))?;
        self.data.set_field(key, pv);
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
            type_collection: slf.type_collection.clone(),
        }
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

#[gen_stub_pyclass]
#[pyclass(unsendable)]
struct PyStructFieldIterator {
    inner: std::collections::hash_map::IntoIter<String, slint_interpreter::Value>,
    type_collection: TypeCollection,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyStructFieldIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<(String, SlintToPyValue)> {
        slf.inner.next().map(|(name, val)| (name, slf.type_collection.to_py_value(val)))
    }
}

thread_local! {
    static ENUM_CLASS: OnceCell<PyObject> = OnceCell::new();
}

pub fn enum_class(py: Python) -> PyObject {
    ENUM_CLASS.with(|cls| {
        cls.get_or_init(|| -> PyObject {
            let enum_module = py.import("enum").unwrap();
            enum_module.getattr("Enum").unwrap().into()
        })
        .clone_ref(py)
    })
}

#[derive(Clone)]
/// Struct that knows about the enums (and maybe other types) exported by
/// a `.slint` file loaded with load_file. This is used to map enums
/// provided by Slint to the correct python enum classes.
pub struct TypeCollection {
    enum_classes: Rc<HashMap<String, PyObject>>,
}

impl TypeCollection {
    pub fn new(result: &slint_interpreter::CompilationResult, py: Python<'_>) -> Self {
        let mut enum_classes = HashMap::new();

        let enum_ctor = crate::value::enum_class(py);

        for struct_or_enum in result.structs_and_enums(i_slint_core::InternalToken {}) {
            match struct_or_enum {
                Type::Enumeration(en) => {
                    let enum_type = enum_ctor
                        .call(
                            py,
                            (
                                en.name.to_string(),
                                en.values
                                    .iter()
                                    .map(|val| {
                                        let val = val.to_string();
                                        (val.clone(), val)
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                            None,
                        )
                        .unwrap();

                    enum_classes.insert(en.name.to_string(), enum_type);
                }
                _ => {}
            }
        }

        let enum_classes = Rc::new(enum_classes);
        Self { enum_classes }
    }

    pub fn to_py_value(&self, value: slint_interpreter::Value) -> SlintToPyValue {
        SlintToPyValue { slint_value: value, type_collection: self.clone() }
    }

    pub fn struct_to_py(&self, s: slint_interpreter::Struct) -> PyStruct {
        PyStruct { data: s, type_collection: self.clone() }
    }

    pub fn enum_to_py(
        &self,
        enum_name: &str,
        enum_value: &str,
        py: Python<'_>,
    ) -> Result<PyObject, PyErr> {
        let enum_cls = self.enum_classes.get(enum_name).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
                "Slint provided enum {enum_name} is unknown"
            ))
        })?;
        enum_cls.getattr(py, enum_value)
    }

    pub fn model_to_py(
        &self,
        model: &ModelRc<slint_interpreter::Value>,
    ) -> crate::models::ReadOnlyRustModel {
        crate::models::ReadOnlyRustModel { model: model.clone(), type_collection: self.clone() }
    }

    pub fn enums(&self) -> impl Iterator<Item = (&String, &PyObject)> {
        self.enum_classes.iter()
    }

    pub fn slint_value_from_py_value(
        py: Python<'_>,
        ob: &PyObject,
        type_collection: Option<&Self>,
    ) -> PyResult<slint_interpreter::Value> {
        Self::slint_value_from_py_value_bound(&ob.bind(py), type_collection)
    }

    pub fn slint_value_from_py_value_bound(
        ob: &Bound<'_, PyAny>,
        type_collection: Option<&Self>,
    ) -> PyResult<slint_interpreter::Value> {
        if ob.is_none() {
            return Ok(slint_interpreter::Value::Void);
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
                ob.extract::<PyRef<'_, crate::models::PyModelBase>>().map(|pymodel| {
                    slint_interpreter::Value::Model(Self::apply(
                        type_collection,
                        pymodel.as_model(),
                    ))
                })
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::models::ReadOnlyRustModel>>().map(|rustmodel| {
                    slint_interpreter::Value::Model(Self::apply(
                        type_collection,
                        rustmodel.model.clone(),
                    ))
                })
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, PyStruct>>().and_then(|pystruct| {
                    Ok(slint_interpreter::Value::Struct(pystruct.data.clone()))
                })
            })
            .or_else(|_| {
                ob.is_instance(&enum_class(ob.py()).into_bound(ob.py())).and_then(|r| {
                    r.then(|| {
                        let enum_name =
                            ob.getattr("__class__").and_then(|cls| cls.getattr("__name__"))?;
                        let enum_value = ob.getattr("name")?;
                        Ok(slint_interpreter::Value::EnumerationValue(
                            enum_name.to_string(),
                            enum_value.to_string(),
                        ))
                    })
                    .unwrap_or_else(|| {
                        Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                            "Object to convert is not an enum",
                        ))
                    })
                })
            })
            .or_else(|_| {
                let dict = ob.downcast::<PyDict>()?;
                let dict_items: Result<Vec<(String, slint_interpreter::Value)>, PyErr> = dict
                    .iter()
                    .map(|(name, pyval)| {
                        let name = name.extract::<&str>()?.to_string();
                        let slintval =
                            Self::slint_value_from_py_value_bound(&pyval, type_collection)?;
                        Ok((name, slintval))
                    })
                    .collect::<Result<Vec<(_, _)>, PyErr>>();
                Ok::<_, PyErr>(slint_interpreter::Value::Struct(
                    slint_interpreter::Struct::from_iter(dict_items?.into_iter()),
                ))
            })?;

        Ok(interpreter_val)
    }

    fn apply(
        type_collection: Option<&Self>,
        model: ModelRc<slint_interpreter::Value>,
    ) -> ModelRc<slint_interpreter::Value> {
        let Some(type_collection) = type_collection else {
            return model;
        };
        if let Some(rust_model) = model.as_any().downcast_ref::<crate::models::PyModelShared>() {
            rust_model.apply_type_collection(type_collection);
        }
        model
    }
}
