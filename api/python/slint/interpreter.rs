// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use pyo3::IntoPyObjectExt;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyclass_enum, gen_stub_pymethods};
use slint_interpreter::{ComponentHandle, Value};

use i_slint_compiler::langtype::Type;
use i_slint_compiler::parser::normalize_identifier;

use indexmap::IndexMap;
use pyo3::gc::PyVisit;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::PyTraverseError;

use crate::errors::{
    PyGetPropertyError, PyInvokeError, PyPlatformError, PySetCallbackError, PySetPropertyError,
};
use crate::value::{SlintToPyValue, TypeCollection};

#[gen_stub_pyclass]
#[pyclass(unsendable)]
pub struct Compiler {
    compiler: slint_interpreter::Compiler,
}

#[gen_stub_pymethods]
#[pymethods]
impl Compiler {
    #[new]
    fn py_new() -> PyResult<Self> {
        Ok(Self { compiler: Default::default() })
    }

    #[getter]
    fn get_include_paths(&self) -> PyResult<Vec<PathBuf>> {
        Ok(self.compiler.include_paths().clone())
    }

    #[setter]
    fn set_include_paths(&mut self, paths: Vec<PathBuf>) {
        self.compiler.set_include_paths(paths)
    }

    #[getter]
    fn get_style(&self) -> PyResult<Option<String>> {
        Ok(self.compiler.style().cloned())
    }

    #[setter]
    fn set_style(&mut self, style: String) {
        self.compiler.set_style(style)
    }

    #[getter]
    fn get_library_paths(&self) -> PyResult<HashMap<String, PathBuf>> {
        Ok(self.compiler.library_paths().clone())
    }

    #[setter]
    fn set_library_paths(&mut self, libraries: HashMap<String, PathBuf>) {
        self.compiler.set_library_paths(libraries)
    }

    #[setter]
    fn set_translation_domain(&mut self, domain: String) {
        self.compiler.set_translation_domain(domain)
    }

    fn build_from_path(&mut self, py: Python<'_>, path: PathBuf) -> CompilationResult {
        CompilationResult::new(spin_on::spin_on(self.compiler.build_from_path(path)), py)
    }

    fn build_from_source(
        &mut self,
        py: Python<'_>,
        source_code: String,
        path: PathBuf,
    ) -> CompilationResult {
        CompilationResult::new(
            spin_on::spin_on(self.compiler.build_from_source(source_code, path)),
            py,
        )
    }
}

#[derive(Debug, Clone)]
#[gen_stub_pyclass]
#[pyclass(unsendable)]
pub struct PyDiagnostic(slint_interpreter::Diagnostic);

#[gen_stub_pymethods]
#[pymethods]
impl PyDiagnostic {
    #[getter]
    fn level(&self) -> PyDiagnosticLevel {
        match self.0.level() {
            slint_interpreter::DiagnosticLevel::Error => PyDiagnosticLevel::Error,
            slint_interpreter::DiagnosticLevel::Warning => PyDiagnosticLevel::Warning,
            _ => unimplemented!(),
        }
    }

    #[getter]
    fn message(&self) -> &str {
        self.0.message()
    }

    #[getter]
    fn column_number(&self) -> usize {
        self.0.line_column().1
    }

    #[getter]
    fn line_number(&self) -> usize {
        self.0.line_column().0
    }

    #[getter]
    fn source_file(&self) -> Option<PathBuf> {
        self.0.source_file().map(|path| path.to_path_buf())
    }

    fn __str__(&self) -> String {
        self.0.to_string()
    }
}

#[gen_stub_pyclass_enum]
#[pyclass(name = "DiagnosticLevel", eq, eq_int)]
#[derive(PartialEq)]
pub enum PyDiagnosticLevel {
    Error,
    Warning,
}

#[gen_stub_pyclass]
#[pyclass(unsendable)]
pub struct CompilationResult {
    result: slint_interpreter::CompilationResult,
    type_collection: TypeCollection,
}

impl CompilationResult {
    fn new(result: slint_interpreter::CompilationResult, py: Python<'_>) -> Self {
        let type_collection = TypeCollection::new(&result, py);
        Self { result, type_collection }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl CompilationResult {
    #[getter]
    fn component_names(&self) -> Vec<String> {
        self.result.component_names().map(ToString::to_string).collect()
    }

    fn component(&self, name: &str) -> Option<ComponentDefinition> {
        self.result.component(name).map(|definition| ComponentDefinition {
            definition,
            type_collection: self.type_collection.clone(),
        })
    }

    #[getter]
    fn get_diagnostics(&self) -> Vec<PyDiagnostic> {
        self.result.diagnostics().map(|diag| PyDiagnostic(diag.clone())).collect()
    }

    #[getter]
    fn structs_and_enums<'py>(
        &self,
        py: Python<'py>,
    ) -> (HashMap<String, Bound<'py, PyAny>>, HashMap<String, Bound<'py, PyAny>>) {
        let mut structs = HashMap::new();

        for struct_or_enum in self.result.structs_and_enums(i_slint_core::InternalToken {}) {
            match struct_or_enum {
                Type::Struct(s) if s.name.is_some() && s.node.is_some() => {
                    let struct_instance =
                        self.type_collection.struct_to_py(slint_interpreter::Struct::from_iter(
                            s.fields.iter().map(|(name, field_type)| {
                                (
                                    name.to_string(),
                                    slint_interpreter::default_value_for_type(field_type),
                                )
                            }),
                        ));

                    structs.insert(
                        s.name.as_ref().unwrap().to_string(),
                        struct_instance.into_bound_py_any(py).unwrap(),
                    );
                }
                _ => {}
            }
        }

        (
            structs,
            self.type_collection
                .enums()
                .map(|(name, enum_cls)| (name.clone(), enum_cls.into_bound_py_any(py).unwrap()))
                .collect(),
        )
    }

    #[getter]
    fn named_exports(&self) -> Vec<(String, String)> {
        self.result.named_exports(i_slint_core::InternalToken {}).cloned().collect::<Vec<_>>()
    }
}

#[gen_stub_pyclass]
#[pyclass(unsendable)]
pub struct ComponentDefinition {
    definition: slint_interpreter::ComponentDefinition,
    type_collection: TypeCollection,
}

#[pymethods]
impl ComponentDefinition {
    #[getter]
    fn name(&self) -> &str {
        self.definition.name()
    }

    #[getter]
    fn properties(&self) -> IndexMap<String, PyValueType> {
        self.definition
            .properties_and_callbacks()
            .filter_map(|(name, (ty, _))| ty.is_property_type().then(|| (name, ty.into())))
            .collect()
    }

    #[getter]
    fn callbacks(&self) -> Vec<String> {
        self.definition.callbacks().collect()
    }

    #[getter]
    fn functions(&self) -> Vec<String> {
        self.definition.functions().collect()
    }

    #[getter]
    fn globals(&self) -> Vec<String> {
        self.definition.globals().collect()
    }

    fn global_properties(&self, name: &str) -> Option<IndexMap<String, PyValueType>> {
        self.definition.global_properties_and_callbacks(name).map(|propiter| {
            propiter
                .filter_map(|(name, (ty, _))| ty.is_property_type().then(|| (name, ty.into())))
                .collect()
        })
    }

    fn global_callbacks(&self, name: &str) -> Option<Vec<String>> {
        self.definition.global_callbacks(name).map(|callbackiter| callbackiter.collect())
    }

    fn global_functions(&self, name: &str) -> Option<Vec<String>> {
        self.definition.global_functions(name).map(|functioniter| functioniter.collect())
    }

    fn callback_returns_void(&self, callback_name: &str) -> bool {
        let callback_name = normalize_identifier(callback_name);
        self.definition
            .properties_and_callbacks()
            .find_map(|(name, (ty, _))| {
                if name == callback_name {
                    if let Type::Callback(signature) = ty {
                        return Some(signature.return_type == Type::Void);
                    }
                }
                None
            })
            .unwrap_or_default()
    }

    fn global_callback_returns_void(&self, global_name: &str, callback_name: &str) -> bool {
        let global_name = normalize_identifier(global_name);
        let callback_name = normalize_identifier(callback_name);
        self.definition
            .global_properties_and_callbacks(&global_name)
            .and_then(|mut props| {
                props.find_map(|(name, (ty, _))| {
                    if name == callback_name {
                        if let Type::Callback(signature) = ty {
                            return Some(signature.return_type == Type::Void);
                        }
                    }
                    None
                })
            })
            .unwrap_or_default()
    }

    fn create(&self) -> Result<ComponentInstance, crate::errors::PyPlatformError> {
        Ok(ComponentInstance {
            instance: self.definition.create()?,
            callbacks: GcVisibleCallbacks {
                callables: Default::default(),
                type_collection: self.type_collection.clone(),
            },
            global_callbacks: Default::default(),
            type_collection: self.type_collection.clone(),
        })
    }
}

#[gen_stub_pyclass_enum]
#[pyclass(name = "ValueType", eq, eq_int)]
#[derive(PartialEq)]
pub enum PyValueType {
    Void,
    Number,
    String,
    Bool,
    Model,
    Struct,
    Brush,
    Image,
    Enumeration,
}

impl From<i_slint_compiler::langtype::Type> for PyValueType {
    fn from(ty: i_slint_compiler::langtype::Type) -> Self {
        match ty {
            i_slint_compiler::langtype::Type::Bool => PyValueType::Bool,
            i_slint_compiler::langtype::Type::Void => PyValueType::Void,
            i_slint_compiler::langtype::Type::Float32
            | i_slint_compiler::langtype::Type::Int32
            | i_slint_compiler::langtype::Type::Duration
            | i_slint_compiler::langtype::Type::Angle
            | i_slint_compiler::langtype::Type::PhysicalLength
            | i_slint_compiler::langtype::Type::LogicalLength
            | i_slint_compiler::langtype::Type::Percent
            | i_slint_compiler::langtype::Type::UnitProduct(_) => PyValueType::Number,
            i_slint_compiler::langtype::Type::String => PyValueType::String,
            i_slint_compiler::langtype::Type::Array(..) => PyValueType::Model,
            i_slint_compiler::langtype::Type::Struct { .. } => PyValueType::Struct,
            i_slint_compiler::langtype::Type::Brush => PyValueType::Brush,
            i_slint_compiler::langtype::Type::Color => PyValueType::Brush,
            i_slint_compiler::langtype::Type::Image => PyValueType::Image,
            i_slint_compiler::langtype::Type::Enumeration(..) => PyValueType::Enumeration,
            _ => unimplemented!(),
        }
    }
}

#[gen_stub_pyclass]
#[pyclass(unsendable, weakref)]
pub struct ComponentInstance {
    instance: slint_interpreter::ComponentInstance,
    callbacks: GcVisibleCallbacks,
    global_callbacks: HashMap<String, GcVisibleCallbacks>,
    type_collection: TypeCollection,
}

#[pymethods]
impl ComponentInstance {
    #[getter]
    fn definition(&self) -> ComponentDefinition {
        ComponentDefinition {
            definition: self.instance.definition(),
            type_collection: self.type_collection.clone(),
        }
    }

    fn get_property(&self, name: &str) -> Result<SlintToPyValue, PyGetPropertyError> {
        Ok(self.type_collection.to_py_value(self.instance.get_property(name)?))
    }

    fn set_property(&self, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        let pv =
            TypeCollection::slint_value_from_py_value_bound(&value, Some(&self.type_collection))?;
        Ok(self.instance.set_property(name, pv).map_err(|e| PySetPropertyError(e))?)
    }

    fn get_global_property(
        &self,
        global_name: &str,
        prop_name: &str,
    ) -> Result<SlintToPyValue, PyGetPropertyError> {
        Ok(self
            .type_collection
            .to_py_value(self.instance.get_global_property(global_name, prop_name)?))
    }

    fn set_global_property(
        &self,
        global_name: &str,
        prop_name: &str,
        value: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let pv =
            TypeCollection::slint_value_from_py_value_bound(&value, Some(&self.type_collection))?;
        Ok(self
            .instance
            .set_global_property(global_name, prop_name, pv)
            .map_err(|e| PySetPropertyError(e))?)
    }

    #[pyo3(signature = (callback_name, *args))]
    fn invoke(&self, callback_name: &str, args: Bound<'_, PyTuple>) -> PyResult<SlintToPyValue> {
        let mut rust_args = vec![];
        for arg in args.iter() {
            let pv =
                TypeCollection::slint_value_from_py_value_bound(&arg, Some(&self.type_collection))?;
            rust_args.push(pv)
        }
        Ok(self.type_collection.to_py_value(
            self.instance.invoke(callback_name, &rust_args).map_err(|e| PyInvokeError(e))?,
        ))
    }

    #[pyo3(signature = (global_name, callback_name, *args))]
    fn invoke_global(
        &self,
        global_name: &str,
        callback_name: &str,
        args: Bound<'_, PyTuple>,
    ) -> PyResult<SlintToPyValue> {
        let mut rust_args = vec![];
        for arg in args.iter() {
            let pv =
                TypeCollection::slint_value_from_py_value_bound(&arg, Some(&self.type_collection))?;
            rust_args.push(pv)
        }
        Ok(self.type_collection.to_py_value(
            self.instance
                .invoke_global(global_name, callback_name, &rust_args)
                .map_err(|e| PyInvokeError(e))?,
        ))
    }

    fn set_callback(&self, name: &str, callable: Py<PyAny>) -> Result<(), PySetCallbackError> {
        let rust_cb = self.callbacks.register(name.to_string(), callable);
        Ok(self.instance.set_callback(name, rust_cb)?.into())
    }

    fn set_global_callback(
        &mut self,
        global_name: &str,
        callback_name: &str,
        callable: Py<PyAny>,
    ) -> Result<(), PySetCallbackError> {
        let rust_cb = self
            .global_callbacks
            .entry(global_name.to_string())
            .or_insert_with(|| GcVisibleCallbacks {
                callables: Default::default(),
                type_collection: self.type_collection.clone(),
            })
            .register(callback_name.to_string(), callable);
        Ok(self.instance.set_global_callback(global_name, callback_name, rust_cb)?.into())
    }

    fn show(&self) -> Result<(), PyPlatformError> {
        Ok(self.instance.show()?)
    }

    fn hide(&self) -> Result<(), PyPlatformError> {
        Ok(self.instance.hide()?)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        self.callbacks.__traverse__(&visit)?;
        for global_callbacks in self.global_callbacks.values() {
            global_callbacks.__traverse__(&visit)?;
        }

        for value in self.properties_for_gc() {
            crate::value::traverse_value(&value, &visit)?;
        }

        Ok(())
    }

    fn __clear__(&mut self) {
        self.callbacks.__clear__();
        self.global_callbacks.clear();

        for value in self.properties_for_gc() {
            crate::value::clear_strongrefs_in_value(&value)
        }
    }
}

impl ComponentInstance {
    fn properties_for_gc(&self) -> Vec<slint_interpreter::Value> {
        let mut props = Vec::new();

        props.extend(
            self.instance
                .definition()
                .properties_and_callbacks()
                .filter_map(|(name, (ty, _))| ty.is_property_type().then(|| name))
                .filter_map(|prop_name| self.instance.get_property(&prop_name).ok()),
        );

        for global_name in self.instance.definition().globals() {
            if let Some(prop_iter) =
                self.instance.definition().global_properties_and_callbacks(&global_name)
            {
                props.extend(
                    prop_iter
                        .filter_map(|(name, (ty, _))| ty.is_property_type().then(|| name))
                        .filter_map(|prop_name| {
                            self.instance.get_global_property(&global_name, &prop_name).ok()
                        }),
                );
            }
        }

        props
    }
}

struct GcVisibleCallbacks {
    callables: Rc<RefCell<HashMap<String, Py<PyAny>>>>,
    type_collection: TypeCollection,
}

impl GcVisibleCallbacks {
    fn register(&self, name: String, callable: Py<PyAny>) -> impl Fn(&[Value]) -> Value + 'static {
        self.callables.borrow_mut().insert(name.clone(), callable);

        let callables = self.callables.clone();
        let type_collection = self.type_collection.clone();

        move |args| {
            let callables = callables.borrow();
            let callable = callables.get(&name).unwrap();
            Python::attach(|py| {
                let py_args =
                    PyTuple::new(py, args.iter().map(|v| type_collection.to_py_value(v.clone())))
                        .unwrap();
                let result = match callable.call(py, py_args, None) {
                    Ok(result) => result,
                    Err(err) => {
                        crate::handle_unraisable(
                            py,
                            format!(
                                "Python: Invoking python callback for {name} threw an exception"
                            ),
                            err,
                        );
                        return Value::Void;
                    }
                };

                let pv = match TypeCollection::slint_value_from_py_value(
                    py,
                    &result,
                    Some(&type_collection),
                ) {
                    Ok(value) => value,
                    Err(err) => {
                        crate::handle_unraisable(
                            py,
                            format!(
                                "Python: Unable to convert return value of Python callback for {name} to Slint value"
                            ),
                            err,
                        );
                        return Value::Void;
                    }
                };
                pv
            })
        }
    }

    fn __traverse__(&self, visit: &PyVisit<'_>) -> Result<(), PyTraverseError> {
        for callback in self.callables.borrow().values() {
            visit.call(callback)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.callables.borrow_mut().clear();
    }
}
