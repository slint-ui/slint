// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use slint_interpreter::{ComponentHandle, Value};

use indexmap::IndexMap;
use pyo3::gc::PyVisit;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::PyTraverseError;

use crate::errors::{
    PyGetPropertyError, PyInvokeError, PyPlatformError, PySetCallbackError, PySetPropertyError,
};
use crate::value::PyValue;

#[pyclass(unsendable)]
pub struct ComponentCompiler {
    compiler: slint_interpreter::ComponentCompiler,
}

#[pymethods]
impl ComponentCompiler {
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

    #[getter]
    fn get_diagnostics(&self) -> Vec<PyDiagnostic> {
        self.compiler.diagnostics().iter().map(|diag| PyDiagnostic(diag.clone())).collect()
    }

    #[setter]
    fn set_translation_domain(&mut self, domain: String) {
        self.compiler.set_translation_domain(domain)
    }

    fn build_from_path(&mut self, path: PathBuf) -> Option<ComponentDefinition> {
        spin_on::spin_on(self.compiler.build_from_path(path))
            .map(|definition| ComponentDefinition { definition })
    }

    fn build_from_source(
        &mut self,
        source_code: String,
        path: PathBuf,
    ) -> Option<ComponentDefinition> {
        spin_on::spin_on(self.compiler.build_from_source(source_code, path))
            .map(|definition| ComponentDefinition { definition })
    }
}

#[derive(Debug, Clone)]
#[pyclass(unsendable)]
pub struct PyDiagnostic(slint_interpreter::Diagnostic);

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
        self.0.line_column().0
    }

    #[getter]
    fn line_number(&self) -> usize {
        self.0.line_column().1
    }

    #[getter]
    fn source_file(&self) -> Option<PathBuf> {
        self.0.source_file().map(|path| path.to_path_buf())
    }

    fn __str__(&self) -> String {
        self.0.to_string()
    }
}

#[pyclass(name = "DiagnosticLevel")]
pub enum PyDiagnosticLevel {
    Error,
    Warning,
}

#[pyclass(unsendable)]
struct ComponentDefinition {
    definition: slint_interpreter::ComponentDefinition,
}

#[pymethods]
impl ComponentDefinition {
    #[getter]
    fn name(&self) -> &str {
        self.definition.name()
    }

    #[getter]
    fn properties(&self) -> IndexMap<String, PyValueType> {
        self.definition.properties().map(|(name, ty)| (name, ty.into())).collect()
    }

    #[getter]
    fn callbacks(&self) -> Vec<String> {
        self.definition.callbacks().collect()
    }

    #[getter]
    fn globals(&self) -> Vec<String> {
        self.definition.globals().collect()
    }

    fn global_properties(&self, name: &str) -> Option<IndexMap<String, PyValueType>> {
        self.definition
            .global_properties(name)
            .map(|propiter| propiter.map(|(name, ty)| (name, ty.into())).collect())
    }

    fn global_callbacks(&self, name: &str) -> Option<Vec<String>> {
        self.definition.global_callbacks(name).map(|callbackiter| callbackiter.collect())
    }

    fn create(&self) -> Result<ComponentInstance, crate::errors::PyPlatformError> {
        Ok(ComponentInstance {
            instance: self.definition.create()?,
            callbacks: Default::default(),
            global_callbacks: Default::default(),
        })
    }
}

#[pyclass(name = "ValueType")]
pub enum PyValueType {
    Void,
    Number,
    String,
    Bool,
    Model,
    Struct,
    Brush,
    Image,
}

impl From<slint_interpreter::ValueType> for PyValueType {
    fn from(value: slint_interpreter::ValueType) -> Self {
        match value {
            slint_interpreter::ValueType::Bool => PyValueType::Bool,
            slint_interpreter::ValueType::Void => PyValueType::Void,
            slint_interpreter::ValueType::Number => PyValueType::Number,
            slint_interpreter::ValueType::String => PyValueType::String,
            slint_interpreter::ValueType::Model => PyValueType::Model,
            slint_interpreter::ValueType::Struct => PyValueType::Struct,
            slint_interpreter::ValueType::Brush => PyValueType::Brush,
            slint_interpreter::ValueType::Image => PyValueType::Image,
            _ => unimplemented!(),
        }
    }
}

#[pyclass(unsendable, weakref)]
struct ComponentInstance {
    instance: slint_interpreter::ComponentInstance,
    callbacks: GcVisibleCallbacks,
    global_callbacks: HashMap<String, GcVisibleCallbacks>,
}

#[pymethods]
impl ComponentInstance {
    #[getter]
    fn definition(&self) -> ComponentDefinition {
        ComponentDefinition { definition: self.instance.definition() }
    }

    fn get_property(&self, name: &str) -> Result<PyValue, PyGetPropertyError> {
        Ok(self.instance.get_property(name)?.into())
    }

    fn set_property(&self, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        let pv: PyValue = value.extract()?;
        Ok(self.instance.set_property(name, pv.0).map_err(|e| PySetPropertyError(e))?)
    }

    fn get_global_property(
        &self,
        global_name: &str,
        prop_name: &str,
    ) -> Result<PyValue, PyGetPropertyError> {
        Ok(self.instance.get_global_property(global_name, prop_name)?.into())
    }

    fn set_global_property(
        &self,
        global_name: &str,
        prop_name: &str,
        value: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let pv: PyValue = value.extract()?;
        Ok(self
            .instance
            .set_global_property(global_name, prop_name, pv.0)
            .map_err(|e| PySetPropertyError(e))?)
    }

    #[pyo3(signature = (callback_name, *args))]
    fn invoke(&self, callback_name: &str, args: Bound<'_, PyTuple>) -> PyResult<PyValue> {
        let mut rust_args = vec![];
        for arg in args.iter() {
            let pv: PyValue = arg.extract()?;
            rust_args.push(pv.0)
        }
        Ok(self.instance.invoke(callback_name, &rust_args).map_err(|e| PyInvokeError(e))?.into())
    }

    #[pyo3(signature = (global_name, callback_name, *args))]
    fn invoke_global(
        &self,
        global_name: &str,
        callback_name: &str,
        args: Bound<'_, PyTuple>,
    ) -> PyResult<PyValue> {
        let mut rust_args = vec![];
        for arg in args.iter() {
            let pv: PyValue = arg.extract()?;
            rust_args.push(pv.0)
        }
        Ok(self
            .instance
            .invoke_global(global_name, callback_name, &rust_args)
            .map_err(|e| PyInvokeError(e))?
            .into())
    }

    fn set_callback(&self, name: &str, callable: PyObject) -> Result<(), PySetCallbackError> {
        let rust_cb = self.callbacks.register(name.to_string(), callable);
        Ok(self.instance.set_callback(name, rust_cb)?.into())
    }

    fn set_global_callback(
        &mut self,
        global_name: &str,
        callback_name: &str,
        callable: PyObject,
    ) -> Result<(), PySetCallbackError> {
        let rust_cb = self
            .global_callbacks
            .entry(global_name.to_string())
            .or_default()
            .register(callback_name.to_string(), callable);
        Ok(self.instance.set_global_callback(global_name, callback_name, rust_cb)?.into())
    }

    fn show(&self) -> Result<(), PyPlatformError> {
        Ok(self.instance.show()?)
    }

    fn hide(&self) -> Result<(), PyPlatformError> {
        Ok(self.instance.hide()?)
    }

    fn run(&self) -> Result<(), PyPlatformError> {
        Ok(self.instance.run()?)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        self.callbacks.__traverse__(&visit)?;
        for global_callbacks in self.global_callbacks.values() {
            global_callbacks.__traverse__(&visit)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.callbacks.__clear__();
        self.global_callbacks.clear();
    }
}

#[derive(Default)]
struct GcVisibleCallbacks {
    callables: Rc<RefCell<HashMap<String, PyObject>>>,
}

impl GcVisibleCallbacks {
    fn register(&self, name: String, callable: PyObject) -> impl Fn(&[Value]) -> Value + 'static {
        self.callables.borrow_mut().insert(name.clone(), callable);

        let callables = self.callables.clone();

        move |args| {
            let callables = callables.borrow();
            let callable = callables.get(&name).unwrap();
            Python::with_gil(|py| {
                let py_args = PyTuple::new_bound(py, args.iter().map(|v| PyValue(v.clone())));
                let result = match callable.call_bound(py, py_args, None) {
                    Ok(result) => result,
                    Err(err) => {
                        eprintln!(
                            "Python: Invoking python callback for {name} threw an exception: {err}"
                        );
                        return Value::Void;
                    }
                };
                let pv: PyValue = match result.extract(py) {
                    Ok(value) => value,
                    Err(err) => {
                        eprintln!("Python: Unable to convert return value of Python callback for {name} to Slint value: {err}");
                        return Value::Void;
                    }
                };
                pv.0
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
