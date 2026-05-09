// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::generator::python::ident;
use pyo3::types::PyDict;
use pyo3::{IntoPyObjectExt, PyTraverseError};
use pyo3::{PyVisit, prelude::*};
use pyo3_stub_gen::{derive::gen_stub_pyclass, derive::gen_stub_pymethods};

use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;

use i_slint_compiler::langtype::Type;

use i_slint_core::model::{Model, ModelRc};

use crate::keys::PyKeys;

#[gen_stub_pyclass]
pub struct SlintToPyValue {
    pub slint_value: slint_interpreter::Value,
    pub type_collection: TypeCollection,
    /// The Slint type this value was declared with, when known. Used to
    /// preserve distinctions the interpreter erases — most notably int vs
    /// float, since both share `Value::Number(f64)`.
    pub expected_type: Option<Type>,
}

impl<'py> IntoPyObject<'py> for SlintToPyValue {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let type_collection = self.type_collection;
        let expected_type = self.expected_type;
        use slint_interpreter::Value;
        match self.slint_value {
            Value::Void => ().into_bound_py_any(py),
            Value::Number(num) => match expected_type {
                Some(Type::Int32) => (num as i64).into_bound_py_any(py),
                _ => num.into_bound_py_any(py),
            },
            Value::String(str) => str.into_bound_py_any(py),
            Value::Bool(b) => b.into_bound_py_any(py),
            Value::Image(image) => crate::image::PyImage::from(image).into_bound_py_any(py),
            Value::Model(model) => {
                let element_type = expected_type.as_ref().and_then(|t| match t {
                    Type::Array(elem) => Some((**elem).clone()),
                    _ => None,
                });
                crate::models::PyModelShared::rust_into_py_model(&model, py).map_or_else(
                    || type_collection.model_to_py(&model, element_type).into_bound_py_any(py),
                    |m| Ok(m),
                )
            }
            Value::Struct(structval) => {
                let struct_type = expected_type.filter(|t| matches!(t, Type::Struct(_)));
                type_collection.struct_to_py(structval, struct_type).into_bound_py_any(py)
            }
            Value::Brush(brush) => crate::brush::PyBrush::from(brush).into_bound_py_any(py),
            Value::EnumerationValue(enum_name, enum_value) => {
                type_collection.enum_to_py(&enum_name, &enum_value, py)?.into_bound_py_any(py)
            }
            Value::Keys(keys) => crate::keys::PyKeys::from(keys).into_bound_py_any(py),
            v @ _ => {
                eprintln!(
                    "Python: conversion from slint to python needed for {v:#?} and not implemented yet"
                );
                ().into_bound_py_any(py)
            }
        }
    }
}

pub fn traverse_value(
    value: &slint_interpreter::Value,
    visit: &PyVisit<'_>,
) -> Result<(), PyTraverseError> {
    match value {
        slint_interpreter::Value::Model(model) => {
            if let Some(rust_model) = model.as_any().downcast_ref::<crate::models::PyModelShared>()
            {
                rust_model.__traverse__(&visit)?
            }
        }
        slint_interpreter::Value::Struct(structval) => traverse_struct(&structval, visit)?,
        _ => {}
    }

    Ok(())
}

fn traverse_struct(
    structval: &slint_interpreter::Struct,
    visit: &PyVisit<'_>,
) -> Result<(), PyTraverseError> {
    for (_, value) in structval.iter() {
        traverse_value(value, visit)?;
    }
    Ok(())
}

pub fn clear_strongrefs_in_value(value: &slint_interpreter::Value) {
    match value {
        slint_interpreter::Value::Model(model) => {
            if let Some(rust_model) = model.as_any().downcast_ref::<crate::models::PyModelShared>()
            {
                rust_model.__clear__();
            }
        }
        slint_interpreter::Value::Struct(structval) => clear_strongrefs_in_struct(&structval),
        _ => {}
    }
}

fn clear_strongrefs_in_struct(structval: &slint_interpreter::Struct) {
    for (_, value) in structval.iter() {
        clear_strongrefs_in_value(value);
    }
}

#[gen_stub_pyclass]
#[pyclass(subclass, unsendable, skip_from_py_object)]
#[derive(Clone)]
pub struct PyStruct {
    pub data: slint_interpreter::Struct,
    pub type_collection: TypeCollection,
    /// The declared `Type::Struct` for `data`, when known. Used so field
    /// access maps each field to the right Python type (e.g. int vs float).
    pub expected_type: Option<Type>,
}

impl PyStruct {
    fn field_type(&self, key: &str) -> Option<Type> {
        self.expected_type.as_ref().and_then(|t| match t {
            Type::Struct(s) => s.fields.get(key).cloned(),
            _ => None,
        })
    }
}

#[pymethods]
impl PyStruct {
    fn __getattr__(&self, key: &str) -> PyResult<SlintToPyValue> {
        self.data.get_field(key).map_or_else(
            || {
                Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                    "Python: No such field {key} on PyStruct"
                )))
            },
            |value| Ok(self.type_collection.to_py_value(value.clone(), self.field_type(key))),
        )
    }
    fn __setattr__(&mut self, py: Python<'_>, key: String, value: Py<PyAny>) -> PyResult<()> {
        let field_type = self.field_type(&key);
        let pv = TypeCollection::slint_value_from_py_value(
            py,
            &value,
            Some(&self.type_collection),
            field_type.as_ref(),
        )?;
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
            expected_type: slf.expected_type.clone(),
        }
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        traverse_struct(&self.data, &visit)
    }

    fn __clear__(&mut self) {
        for (_, value) in self.data.iter() {
            clear_strongrefs_in_value(&value);
        }
    }
}

#[gen_stub_pyclass]
#[pyclass(unsendable)]
struct PyStructFieldIterator {
    inner: std::collections::hash_map::IntoIter<String, slint_interpreter::Value>,
    type_collection: TypeCollection,
    expected_type: Option<Type>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyStructFieldIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<(String, SlintToPyValue)> {
        slf.inner.next().map(|(name, val)| {
            let field_type = slf.expected_type.as_ref().and_then(|t| match t {
                Type::Struct(s) => s.fields.get(name.as_str()).cloned(),
                _ => None,
            });
            let py_value = slf.type_collection.to_py_value(val, field_type);
            (name, py_value)
        })
    }
}

thread_local! {
    static ENUM_CLASS: OnceCell<Py<PyAny>> = OnceCell::new();
}

pub fn enum_class(py: Python) -> Py<PyAny> {
    ENUM_CLASS.with(|cls| {
        cls.get_or_init(|| -> Py<PyAny> {
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
    enum_classes: Rc<HashMap<String, Py<PyAny>>>,
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
                                    .map(|val| (ident(&val).to_string(), val.to_string()))
                                    .collect::<Vec<_>>(),
                            ),
                            None,
                        )
                        .unwrap();

                    enum_classes.insert(ident(&en.name).into(), enum_type);
                }
                _ => {}
            }
        }

        let enum_classes = Rc::new(enum_classes);
        Self { enum_classes }
    }

    pub fn to_py_value(
        &self,
        value: slint_interpreter::Value,
        expected_type: Option<Type>,
    ) -> SlintToPyValue {
        SlintToPyValue { slint_value: value, type_collection: self.clone(), expected_type }
    }

    pub fn struct_to_py(
        &self,
        s: slint_interpreter::Struct,
        expected_type: Option<Type>,
    ) -> PyStruct {
        PyStruct { data: s, type_collection: self.clone(), expected_type }
    }

    pub fn enum_to_py(
        &self,
        enum_name: &str,
        enum_value: &str,
        py: Python<'_>,
    ) -> Result<Py<PyAny>, PyErr> {
        let enum_cls = self.enum_classes.get(ident(enum_name).as_str()).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
                "Slint provided enum {enum_name} is unknown"
            ))
        })?;
        enum_cls.getattr(py, enum_value)
    }

    pub fn model_to_py(
        &self,
        model: &ModelRc<slint_interpreter::Value>,
        element_type: Option<Type>,
    ) -> crate::models::ReadOnlyRustModel {
        crate::models::ReadOnlyRustModel {
            model: model.clone(),
            type_collection: self.clone(),
            element_type,
        }
    }

    pub fn enums(&self) -> impl Iterator<Item = (&String, &Py<PyAny>)> {
        self.enum_classes.iter()
    }

    pub fn slint_value_from_py_value(
        py: Python<'_>,
        ob: &Py<PyAny>,
        type_collection: Option<&Self>,
        expected_type: Option<&Type>,
    ) -> PyResult<slint_interpreter::Value> {
        Self::slint_value_from_py_value_bound(&ob.bind(py), type_collection, expected_type)
    }

    pub fn slint_value_from_py_value_bound(
        ob: &Bound<'_, PyAny>,
        type_collection: Option<&Self>,
        expected_type: Option<&Type>,
    ) -> PyResult<slint_interpreter::Value> {
        if ob.is_none() {
            return Ok(slint_interpreter::Value::Void);
        }

        let interpreter_val = ob
            .extract::<bool>()
            .map(slint_interpreter::Value::Bool)
            .or_else(|_| {
                ob.extract::<&'_ str>().map(|s| slint_interpreter::Value::String(s.into()))
            })
            .or_else(|_| ob.extract::<f64>().map(slint_interpreter::Value::Number))
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::image::PyImage>>()
                    .map(|pyimg| slint_interpreter::Value::Image(pyimg.image.clone()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::brush::PyBrush>>()
                    .map(|pybrush| slint_interpreter::Value::Brush(pybrush.brush.clone()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, PyKeys>>()
                    .map(|keys| slint_interpreter::Value::Keys(keys.keys.clone()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::brush::PyColor>>()
                    .map(|pycolor| slint_interpreter::Value::Brush(pycolor.color.into()))
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::models::PyModelBase>>().map(|pymodel| {
                    slint_interpreter::Value::Model(Self::apply(
                        type_collection,
                        expected_type,
                        pymodel.as_model(),
                    ))
                })
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, crate::models::ReadOnlyRustModel>>().map(|rustmodel| {
                    slint_interpreter::Value::Model(Self::apply(
                        type_collection,
                        expected_type,
                        rustmodel.model.clone(),
                    ))
                })
            })
            .or_else(|_| {
                ob.extract::<PyRef<'_, PyStruct>>()
                    .map(|pystruct| slint_interpreter::Value::Struct(pystruct.data.clone()))
            })
            .or_else(|_| {
                ob.is_instance(&enum_class(ob.py()).into_bound(ob.py())).and_then(|is_enum| {
                    if is_enum {
                        {
                            let enum_name =
                                ob.getattr("__class__").and_then(|cls| cls.getattr("__name__"))?;
                            let enum_value = ob.getattr("name")?;
                            Ok(slint_interpreter::Value::EnumerationValue(
                                enum_name.to_string(),
                                enum_value.to_string(),
                            ))
                        }
                    } else {
                        {
                            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                                "Object to convert is not an enum",
                            ))
                        }
                    }
                })
            })
            .or_else(|_| {
                // Try NamedTuple conversion first, then fall back to direct PyDict cast.
                // NamedTuples (e.g. StandardListViewItem) are tuple subclasses registered
                // as `typing.NamedTuple` in language.rs. We guard with an isinstance(ob, tuple)
                // check to avoid false positives from unrelated types that also have `_asdict`.
                let dict = if ob.is_instance_of::<pyo3::types::PyTuple>()
                    && ob.hasattr(pyo3::intern!(ob.py(), "_fields")).unwrap_or(false)
                {
                    let asdict = ob.call_method0(pyo3::intern!(ob.py(), "_asdict"))?;
                    asdict.cast::<PyDict>().cloned().map_err(|e| -> PyErr { e.into() })
                } else {
                    ob.cast::<PyDict>().cloned().map_err(|_| {
                        pyo3::exceptions::PyTypeError::new_err("Object is not a dict or NamedTuple")
                    })
                }?;
                let struct_fields = expected_type.and_then(|t| match t {
                    Type::Struct(s) => Some(&s.fields),
                    _ => None,
                });
                let dict_items: Result<Vec<(String, slint_interpreter::Value)>, PyErr> = dict
                    .iter()
                    .map(|(name, pyval)| {
                        let name = name.extract::<&str>()?.to_string();
                        let field_type = struct_fields.and_then(|fields| fields.get(name.as_str()));
                        let slintval = Self::slint_value_from_py_value_bound(
                            &pyval,
                            type_collection,
                            field_type,
                        )?;
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
        expected_type: Option<&Type>,
        model: ModelRc<slint_interpreter::Value>,
    ) -> ModelRc<slint_interpreter::Value> {
        let Some(type_collection) = type_collection else {
            return model;
        };
        if let Some(rust_model) = model.as_any().downcast_ref::<crate::models::PyModelShared>() {
            let element_type = match expected_type {
                Some(Type::Array(element)) => Some((**element).clone()),
                _ => None,
            };
            rust_model.apply_type_collection(type_collection, element_type);
        }
        model
    }
}
