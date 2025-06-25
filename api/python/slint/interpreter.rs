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

use indexmap::IndexMap;
use pyo3::gc::PyVisit;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use pyo3::PyTraverseError;

use crate::errors::{
    PyGetPropertyError, PyInvokeError, PyPlatformError, PySetCallbackError, PySetPropertyError,
};
use crate::value::{PyStruct, PyValue};

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

    fn build_from_path(&mut self, path: PathBuf) -> CompilationResult {
        CompilationResult { result: spin_on::spin_on(self.compiler.build_from_path(path)) }
    }

    fn build_from_source(&mut self, source_code: String, path: PathBuf) -> CompilationResult {
        CompilationResult {
            result: spin_on::spin_on(self.compiler.build_from_source(source_code, path)),
        }
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
}

#[gen_stub_pymethods]
#[pymethods]
impl CompilationResult {
    #[getter]
    fn component_names(&self) -> Vec<String> {
        self.result.component_names().map(ToString::to_string).collect()
    }

    fn component(&self, name: &str) -> Option<ComponentDefinition> {
        self.result.component(name).map(|definition| ComponentDefinition { definition })
    }

    #[getter]
    fn get_diagnostics(&self) -> Vec<PyDiagnostic> {
        self.result.diagnostics().map(|diag| PyDiagnostic(diag.clone())).collect()
    }

    #[getter]
    fn structs_and_enums<'py>(&self, py: Python<'py>) -> HashMap<String, Bound<'py, PyAny>> {
        let structs_and_enums =
            self.result.structs_and_enums(i_slint_core::InternalToken {}).collect::<Vec<_>>();

        fn convert_type<'py>(py: Python<'py>, ty: &Type) -> Option<(String, Bound<'py, PyAny>)> {
            match ty {
                Type::Struct(s) if s.name.is_some() && s.node.is_some() => {
                    let struct_instance = PyStruct::from(slint_interpreter::Struct::from_iter(
                        s.fields.iter().map(|(name, field_type)| {
                            (
                                name.to_string(),
                                slint_interpreter::default_value_for_type(field_type),
                            )
                        }),
                    ));

                    return Some((
                        s.name.as_ref().unwrap().to_string(),
                        struct_instance.into_bound_py_any(py).unwrap(),
                    ));
                }
                Type::Enumeration(_en) => {
                    // TODO
                }
                _ => {}
            }
            None
        }

        structs_and_enums
            .iter()
            .filter_map(|ty| convert_type(py, ty))
            .into_iter()
            .collect::<HashMap<String, Bound<'py, PyAny>>>()
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
    fn functions(&self) -> Vec<String> {
        self.definition.functions().collect()
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

    fn global_functions(&self, name: &str) -> Option<Vec<String>> {
        self.definition.global_functions(name).map(|functioniter| functioniter.collect())
    }

    fn create(&self) -> Result<ComponentInstance, crate::errors::PyPlatformError> {
        Ok(ComponentInstance {
            instance: self.definition.create()?,
            callbacks: Default::default(),
            global_callbacks: Default::default(),
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

#[gen_stub_pyclass]
#[pyclass(unsendable, weakref)]
pub struct ComponentInstance {
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
                let py_args = PyTuple::new(py, args.iter().map(|v| PyValue(v.clone()))).unwrap();
                let result = match callable.call(py, py_args, None) {
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
